use crate::app::{ActiveTab, DevNookApp};
use devnook_core::models::StepRunState;
use crate::ui::{badge, palette};
use gpui_kit::component::{
    button::{Button, ButtonVariants as _},
    input::Input,
};
use gpui_kit::{prelude::*, *};

fn step_colors(state: &StepRunState) -> (u32, u32) {
    match state {
        StepRunState::Pending => (0x1f2937, 0x9ca3af),
        StepRunState::Reused => (0x1e3a8a, 0x93c5fd),
        StepRunState::Starting | StepRunState::WaitingReady => (0x78350f, 0xfcd34d),
        StepRunState::Done => (0x064e3b, 0x6ee7b7),
        StepRunState::Failed(_) => (0x7f1d1d, 0xfca5a5),
    }
}

pub fn render_pipelines_view(app: &mut DevNookApp, cx: &mut Context<DevNookApp>) -> impl IntoElement {
    let zh = app.is_zh();
    let pal = palette(app.data.dark_mode);
    let selected_plan_id = app.selected_plan_id.clone();
    let active_plan = selected_plan_id
        .as_deref()
        .and_then(|id| app.data.plans.iter().find(|p| p.id == id))
        .cloned();
    let plan_run = app.plan_run_state.clone();

    div()
        .flex()
        .size_full()
        .gap_3()
        .child(
            div()
                .w(px(280.))
                .h_full()
                .flex()
                .flex_col()
                .gap_2()
                .p_3()
                .rounded_lg()
                .bg(rgb(pal.surface))
                .border_1()
                .border_color(rgb(pal.border))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(if zh { "启动方案" } else { "Startup Plans" }),
                        )
                        .child(
                            Button::new("new-plan")
                                .primary()
                                .label(if zh { "+ 新建" } else { "+ New" })
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.create_new_plan(window, cx);
                                })),
                        ),
                )
                .child(
                    div()
                        .id("plans-scroll")
                        .flex_1()
                        .overflow_y_scroll()
                        .flex()
                        .flex_col()
                        .gap_1p5()
                        .children(app.data.plans.iter().enumerate().map(|(ix, plan)| {
                            let is_sel = selected_plan_id.as_deref() == Some(&plan.id);
                            let pid = plan.id.clone();
                            div()
                                .id(("plan-item", ix))
                                .p_2()
                                .rounded_md()
                                .border_1()
                                .border_color(if is_sel {
                                    rgb(pal.accent)
                                } else {
                                    rgb(pal.border)
                                })
                                .cursor_pointer()
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.select_plan(&pid, window, cx);
                                }))
                                .child(div().font_weight(FontWeight::MEDIUM).child(plan.name.clone()))
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(rgb(pal.muted))
                                        .child(if zh {
                                            format!("{} 个步骤", plan.steps.len())
                                        } else {
                                            format!("{} steps", plan.steps.len())
                                        }),
                                )
                        })),
                ),
        )
        .child(
            div()
                .flex_1()
                .h_full()
                .flex()
                .flex_col()
                .gap_3()
                .p_3()
                .rounded_lg()
                .bg(rgb(pal.surface))
                .border_1()
                .border_color(rgb(pal.border))
                .when(active_plan.is_none(), |d| {
                    d.child(
                        div()
                            .p_6()
                            .text_color(rgb(pal.muted))
                            .child(if zh {
                                "选择或新建一个启动方案，按依赖顺序启动多个服务。"
                            } else {
                                "Select or create a Startup Plan to orchestrate dependent services in order."
                            }),
                    )
                })
                .when_some(active_plan, |d, plan| {
                    let plan_id = plan.id.clone();
                    let plan_id_run = plan.id.clone();
                    let plan_id_stop = plan.id.clone();
                    let plan_id_del = plan.id.clone();
                    let is_this_running =
                        plan_run.running && plan_run.active_plan_id.as_deref() == Some(&plan.id);

                    d.child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap_2()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .flex_1()
                                    .child(div().w(px(220.)).child(Input::new(&app.plan_name_input)))
                                    .child(div().flex_1().child(Input::new(&app.plan_desc_input))),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        Button::new("save-plan")
                                            .ghost()
                                            .label(if zh { "保存" } else { "Save" })
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                this.save_current_plan(&plan_id, cx);
                                            })),
                                    )
                                    .child(
                                        Button::new("run-plan")
                                            .primary()
                                            .label(if is_this_running {
                                                if zh { "执行中..." } else { "Running..." }
                                            } else if zh {
                                                "按顺序启动"
                                            } else {
                                                "Run Plan"
                                            })
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                this.run_plan(&plan_id_run, cx);
                                            })),
                                    )
                                    .child(
                                        Button::new("stop-plan")
                                            .ghost()
                                            .label(if zh { "停止本次启动" } else { "Stop Started" })
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                this.stop_plan_services(&plan_id_stop, cx);
                                            })),
                                    )
                                    .child(
                                        Button::new("del-plan")
                                            .ghost()
                                            .label(if zh { "删除" } else { "Delete" })
                                            .on_click(cx.listener(move |this, _, window, cx| {
                                                this.delete_plan(&plan_id_del, window, cx);
                                            })),
                                    ),
                            ),
                    )
                    .when_some(plan_run.summary.clone(), |d, summary| {
                        d.child(
                            div()
                                .px_3()
                                .py_2()
                                .rounded_md()
                                .bg(rgb(0x1e293b))
                                .text_xs()
                                .text_color(rgb(0x93c5fd))
                                .child(summary),
                        )
                    })
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_sm()
                            .child(if zh {
                                "执行顺序（已就绪服务自动复用，等待上游就绪后再启动下游；停止方案仅停止本轮启动的实例）"
                            } else {
                                "Execution Order (reuses already-ready services; waits for upstream readiness)"
                            }),
                    )
                    .child(
                        div()
                            .id("steps-scroll")
                            .flex_1()
                            .overflow_y_scroll()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .children(plan.steps.iter().enumerate().map(|(ix, step)| {
                                let svc_name = app
                                    .data
                                    .services
                                    .iter()
                                    .find(|s| s.id == step.service_id)
                                    .map(|s| s.name.clone())
                                    .unwrap_or_else(|| "Unknown Service".to_string());
                                let step_state = if plan_run.active_plan_id.as_deref() == Some(&plan.id) {
                                    plan_run
                                        .step_states
                                        .get(ix)
                                        .map(|(_, st)| st.clone())
                                        .unwrap_or(StepRunState::Pending)
                                } else {
                                    StepRunState::Pending
                                };
                                let (bg, fg) = step_colors(&step_state);
                                let pid_up = plan.id.clone();
                                let pid_down = plan.id.clone();
                                let pid_rem = plan.id.clone();
                                let pid_wait = plan.id.clone();
                                let sid_jump = step.service_id.clone();

                                div()
                                    .id(("step-row", ix))
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .p_2()
                                    .rounded_md()
                                    .border_1()
                                    .border_color(rgb(pal.border))
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap_2()
                                            .child(badge(format!("#{}", ix + 1), 0x1e293b, 0xcbd5e1))
                                            .child(div().font_weight(FontWeight::MEDIUM).child(svc_name))
                                            .child(
                                                Button::new(("step-wait", ix))
                                                    .ghost()
                                                    .label(if step.wait_for_ready {
                                                        if zh {
                                                            format!("等待就绪 ({}s)", step.timeout_secs)
                                                        } else {
                                                            format!("Wait Ready ({}s)", step.timeout_secs)
                                                        }
                                                    } else if zh {
                                                        "直接下一步".to_string()
                                                    } else {
                                                        "Fire & Continue".to_string()
                                                    })
                                                    .on_click(cx.listener(move |this, _, _, cx| {
                                                        this.toggle_step_wait(&pid_wait, ix, cx);
                                                    })),
                                            )
                                            .child(badge(step_state.label(zh).to_string(), bg, fg)),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap_1()
                                            .child(
                                                Button::new(("step-logs", ix))
                                                    .ghost()
                                                    .label(if zh { "日志" } else { "Logs" })
                                                    .on_click(cx.listener(move |this, _, window, cx| {
                                                        this.select_service(&sid_jump, window, cx);
                                                        this.active_tab = ActiveTab::Services;
                                                        cx.notify();
                                                    })),
                                            )
                                            .child(
                                                Button::new(("step-up", ix))
                                                    .ghost()
                                                    .label("↑")
                                                    .on_click(cx.listener(move |this, _, _, cx| {
                                                        this.move_plan_step(&pid_up, ix, -1, cx);
                                                    })),
                                            )
                                            .child(
                                                Button::new(("step-down", ix))
                                                    .ghost()
                                                    .label("↓")
                                                    .on_click(cx.listener(move |this, _, _, cx| {
                                                        this.move_plan_step(&pid_down, ix, 1, cx);
                                                    })),
                                            )
                                            .child(
                                                Button::new(("step-rem", ix))
                                                    .ghost()
                                                    .label(if zh { "移除" } else { "Remove" })
                                                    .on_click(cx.listener(move |this, _, _, cx| {
                                                        this.remove_plan_step(&pid_rem, ix, cx);
                                                    })),
                                            ),
                                    )
                            })),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1p5()
                            .pt_2()
                            .border_t_1()
                            .border_color(rgb(pal.border))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(pal.muted))
                                    .child(if zh {
                                        "添加已登记的服务到此方案："
                                    } else {
                                        "Add registered service to this plan:"
                                    }),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_wrap()
                                    .gap_1p5()
                                    .children(app.data.services.iter().enumerate().map(|(ix, svc)| {
                                        let pid_add = plan.id.clone();
                                        let sid_add = svc.id.clone();
                                        Button::new(("add-svc-step", ix))
                                            .ghost()
                                            .label(format!("+ {}", svc.name))
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                this.add_step_to_plan(&pid_add, &sid_add, cx);
                                            }))
                                    })),
                            ),
                    )
                }),
        )
}
