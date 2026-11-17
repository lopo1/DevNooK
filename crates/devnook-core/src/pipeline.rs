use crate::models::{ServiceConfig, ServiceRuntime, ServiceStatus, StartupPlan, StepRunState};
use crate::service::ServiceManager;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use tokio::time::{Duration, Instant, sleep};

#[derive(Debug, Clone)]
pub enum PipelineEvent {
    StepUpdated {
        plan_id: String,
        step_index: usize,
        state: StepRunState,
    },
    ServiceStartedByPlan {
        plan_id: String,
        service_id: String,
    },
    Finished {
        plan_id: String,
        success: bool,
        message: String,
    },
}

pub fn run_startup_plan(
    plan: StartupPlan,
    services: Vec<ServiceConfig>,
    runtimes_snapshot: Arc<Mutex<HashMap<String, ServiceRuntime>>>,
    manager: Arc<ServiceManager>,
    tx: mpsc::UnboundedSender<PipelineEvent>,
) {
    let rt = manager.rt_handle();
    let svc_map: HashMap<String, ServiceConfig> =
        services.into_iter().map(|s| (s.id.clone(), s)).collect();

    rt.spawn(async move {
        let plan_id = plan.id.clone();

        for (idx, step) in plan.steps.iter().enumerate() {
            let Some(cfg) = svc_map.get(&step.service_id).cloned() else {
                let msg = format!("Service {} not found", step.service_id);
                let _ = tx.send(PipelineEvent::StepUpdated {
                    plan_id: plan_id.clone(),
                    step_index: idx,
                    state: StepRunState::Failed(msg.clone()),
                });
                let _ = tx.send(PipelineEvent::Finished {
                    plan_id,
                    success: false,
                    message: msg,
                });
                return;
            };

            let already_ready = runtimes_snapshot
                .lock()
                .ok()
                .and_then(|m| m.get(&cfg.id).map(|r| r.status == ServiceStatus::Ready))
                .unwrap_or(false);

            if already_ready {
                let _ = tx.send(PipelineEvent::StepUpdated {
                    plan_id: plan_id.clone(),
                    step_index: idx,
                    state: StepRunState::Reused,
                });
                continue;
            }

            let _ = tx.send(PipelineEvent::StepUpdated {
                plan_id: plan_id.clone(),
                step_index: idx,
                state: StepRunState::Starting,
            });

            let _ = tx.send(PipelineEvent::ServiceStartedByPlan {
                plan_id: plan_id.clone(),
                service_id: cfg.id.clone(),
            });
            manager.start_service(cfg.clone());

            if step.wait_for_ready {
                let _ = tx.send(PipelineEvent::StepUpdated {
                    plan_id: plan_id.clone(),
                    step_index: idx,
                    state: StepRunState::WaitingReady,
                });

                let deadline = Instant::now() + Duration::from_secs(step.timeout_secs.max(5));
                loop {
                    let status = runtimes_snapshot
                        .lock()
                        .ok()
                        .and_then(|m| m.get(&cfg.id).map(|r| r.status.clone()))
                        .unwrap_or(ServiceStatus::Stopped);

                    match status {
                        ServiceStatus::Ready => break,
                        ServiceStatus::Failed(err) => {
                            let msg = format!("{} failed: {err}", cfg.name);
                            let _ = tx.send(PipelineEvent::StepUpdated {
                                plan_id: plan_id.clone(),
                                step_index: idx,
                                state: StepRunState::Failed(msg.clone()),
                            });
                            let _ = tx.send(PipelineEvent::Finished {
                                plan_id,
                                success: false,
                                message: msg,
                            });
                            return;
                        }
                        _ => {}
                    }

                    if Instant::now() >= deadline {
                        let msg = format!("{} timed out after {}s", cfg.name, step.timeout_secs);
                        let _ = tx.send(PipelineEvent::StepUpdated {
                            plan_id: plan_id.clone(),
                            step_index: idx,
                            state: StepRunState::Failed(msg.clone()),
                        });
                        let _ = tx.send(PipelineEvent::Finished {
                            plan_id,
                            success: false,
                            message: msg,
                        });
                        return;
                    }

                    sleep(Duration::from_millis(350)).await;
                }
            }

            if step.delay_after_ms > 0 {
                sleep(Duration::from_millis(step.delay_after_ms)).await;
            }

            let _ = tx.send(PipelineEvent::StepUpdated {
                plan_id: plan_id.clone(),
                step_index: idx,
                state: StepRunState::Done,
            });
        }

        let _ = tx.send(PipelineEvent::Finished {
            plan_id,
            success: true,
            message: format!("All {} services ready.", plan.steps.len()),
        });
    });
}
