use crate::app::{DevNookApp, LogStreamFilter};
use devnook_core::models::{LogStream, ServiceStatus};
use crate::ui::{badge, palette};
use gpui_kit::component::{
    button::{Button, ButtonVariants as _},
    input::Input,
};
use gpui_kit::{prelude::*, *};

fn status_colors(status: &ServiceStatus) -> (u32, u32) {
    match status {
        ServiceStatus::Ready => (0x064e3b, 0x6ee7b7),
        ServiceStatus::Starting => (0x78350f, 0xfcd34d),
        ServiceStatus::Unhealthy(_) | ServiceStatus::Failed(_) => (0x7f1d1d, 0xfca5a5),
        ServiceStatus::Stopped => (0x1f2937, 0x9ca3af),
    }
}

pub fn render_services_view(app: &mut DevNookApp, cx: &mut Context<DevNookApp>) -> impl IntoElement {
    let zh = app.is_zh();
    let pal = palette(app.data.dark_mode);
    let runtimes = app.runtimes.lock().map(|m| m.clone()).unwrap_or_default();
    let selected_id = app.selected_service_id.clone();
    let log_query = app.log_search_input.read(cx).value().to_lowercase();
    let stream_filter = app.log_stream_filter;

    div()
        .flex()
        .size_full()
        .gap_3()
        // Left sidebar
        .child(
            div()
                .w(px(300.))
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
                                .child(if zh { "服务列表" } else { "Services" }),
                        )
                        .child(
                            Button::new("new-svc")
                                .primary()
                                .label(if zh { "+ 新建" } else { "+ New" })
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.reset_service_form(window, cx);
                                })),
                        ),
                )
                .child(
                    div()
                        .id("svc-list-scroll")
                        .flex_1()
                        .overflow_y_scroll()
                        .flex()
                        .flex_col()
                        .gap_1p5()
                        .children(app.data.services.iter().enumerate().map(|(ix, svc)| {
                            let rt = runtimes.get(&svc.id).cloned().unwrap_or_default();
                            let (bg, fg) = status_colors(&rt.status);
                            let is_sel = selected_id.as_deref() == Some(&svc.id);
                            let sid = svc.id.clone();
                            let sid_toggle = svc.id.clone();
                            let active = rt.status.is_active();
                            let ports_label = if rt.listening_ports.is_empty() {
                                svc.expected_port
                                    .map(|p| format!("exp :{p}"))
                                    .unwrap_or_else(|| "no port".to_string())
                            } else {
                                rt.listening_ports
                                    .iter()
                                    .map(|p| format!(":{p}"))
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            };

                            div()
                                .id(("svc-item", ix))
                                .flex()
                                .items_center()
                                .justify_between()
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
                                    this.select_service(&sid, window, cx);
                                }))
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap_1()
                                        .child(div().font_weight(FontWeight::MEDIUM).child(svc.name.clone()))
                                        .child(
                                            div()
                                                .flex()
                                                .items_center()
                                                .gap_1()
                                                .child(badge(rt.status.label(zh).to_string(), bg, fg))
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .text_color(rgb(pal.muted))
                                                        .child(ports_label),
                                                ),
                                        ),
                                )
                                .child(
                                    Button::new(("svc-quick-toggle", ix))
                                        .when(!active, |b| b.primary())
                                        .when(active, |b| b.ghost())
                                        .label(if active {
                                            if zh { "停止" } else { "Stop" }
                                        } else if zh {
                                            "启动"
                                        } else {
                                            "Start"
                                        })
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            if active {
                                                this.stop_service(&sid_toggle, cx);
                                            } else {
                                                this.start_service(&sid_toggle, cx);
                                            }
                                        })),
                                )
                        })),
                ),
        )
        // Right pane
        .child(
            div()
                .flex_1()
                .h_full()
                .flex()
                .flex_col()
                .gap_3()
                .child(
                    div()
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
                                        .child(if selected_id.is_some() {
                                            if zh { "服务配置与控制" } else { "Service Configuration & Controls" }
                                        } else if zh {
                                            "新建服务配置"
                                        } else {
                                            "Create New Service Configuration"
                                        }),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .child(
                                            Button::new("save-svc-cfg")
                                                .primary()
                                                .label(if zh { "保存配置" } else { "Save Config" })
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.save_service_from_form(cx);
                                                })),
                                        )
                                        .when_some(selected_id.clone(), |d, sid| {
                                            let rt = runtimes.get(&sid).cloned().unwrap_or_default();
                                            let active = rt.status.is_active();
                                            let sid_run = sid.clone();
                                            let sid_restart = sid.clone();
                                            let sid_del = sid.clone();
                                            d.child(
                                                Button::new("confirm-exec-svc")
                                                    .when(!active, |b| b.primary())
                                                    .when(active, |b| b.ghost())
                                                    .label(if active {
                                                        if zh { "停止服务" } else { "Stop" }
                                                    } else if zh {
                                                        "确认执行"
                                                    } else {
                                                        "Confirm & Start"
                                                    })
                                                    .on_click(cx.listener(move |this, _, _, cx| {
                                                        if active {
                                                            this.stop_service(&sid_run, cx);
                                                        } else {
                                                            this.start_service(&sid_run, cx);
                                                        }
                                                    })),
                                            )
                                            .when(active, |b| {
                                                b.child(
                                                    Button::new("restart-svc")
                                                        .ghost()
                                                        .label(if zh { "重启" } else { "Restart" })
                                                        .on_click(cx.listener(move |this, _, _, cx| {
                                                            this.restart_service(&sid_restart, cx);
                                                        })),
                                                )
                                            })
                                            .child(
                                                Button::new("del-svc")
                                                    .ghost()
                                                    .label(if zh { "删除" } else { "Delete" })
                                                    .on_click(cx.listener(move |this, _, window, cx| {
                                                        this.delete_service(&sid_del, window, cx);
                                                    })),
                                            )
                                        }),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .gap_2()
                                .child(div().flex_1().child(Input::new(&app.svc_name_input)))
                                .child(div().w(px(140.)).child(Input::new(&app.svc_port_input))),
                        )
                        .child(
                            div()
                                .flex()
                                .gap_2()
                                .child(div().flex_1().child(Input::new(&app.svc_dir_input)))
                                .child(div().flex_1().child(Input::new(&app.svc_cmd_input))),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(rgb(pal.muted))
                                        .child(if zh {
                                            "就绪探针 (tcp:3000 | http:localhost:8080/health | log:Ready | alive):"
                                        } else {
                                            "Readiness Probe (tcp:3000 | http:localhost:8080/health | log:Ready | alive):"
                                        }),
                                )
                                .child(div().flex_1().child(Input::new(&app.svc_health_input))),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(rgb(pal.muted))
                                        .child(if zh {
                                            "环境变量 (KEY=VAL; KEY2=VAL2，日志自动脱敏):"
                                        } else {
                                            "Env Vars (KEY=VAL; KEY2=VAL2, auto-redacted in logs):"
                                        }),
                                )
                                .child(div().flex_1().child(Input::new(&app.svc_env_input))),
                        ),
                )
                .child({
                    let rt = selected_id
                        .as_deref()
                        .and_then(|id| runtimes.get(id))
                        .cloned()
                        .unwrap_or_default();
                    let svc_cfg = selected_id
                        .as_deref()
                        .and_then(|id| app.data.services.iter().find(|s| s.id == id))
                        .cloned();
                    let (bg, fg) = status_colors(&rt.status);
                    let ports_text = if rt.listening_ports.is_empty() {
                        if zh { "未检测到".to_string() } else { "None".to_string() }
                    } else {
                        rt.listening_ports
                            .iter()
                            .map(|p| format!(":{p}"))
                            .collect::<Vec<_>>()
                            .join(", ")
                    };
                    let web_port = rt
                        .listening_ports
                        .first()
                        .copied()
                        .or_else(|| svc_cfg.as_ref().and_then(|s| s.expected_port));

                    let selected_hist = app.selected_run_history_idx;
                    let active_logs: Vec<_> = match selected_hist.and_then(|idx| rt.history.get(idx)) {
                        Some(rec) => rec.logs.clone(),
                        None => rt.logs.iter().cloned().collect(),
                    };

                    let filtered_logs: Vec<_> = active_logs
                        .into_iter()
                        .filter(|line| {
                            let stream_ok = match stream_filter {
                                LogStreamFilter::All => true,
                                LogStreamFilter::Stdout => line.stream == LogStream::Stdout,
                                LogStreamFilter::Stderr => line.stream == LogStream::Stderr,
                                LogStreamFilter::System => line.stream == LogStream::System,
                            };
                            if !stream_ok {
                                return false;
                            }
                            if !log_query.is_empty()
                                && !line.text.to_lowercase().contains(&log_query)
                            {
                                return false;
                            }
                            true
                        })
                        .collect();

                    div()
                        .flex_1()
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
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .child(
                                            div()
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .child(if zh { "服务详情与日志" } else { "Service Detail & Logs" }),
                                        )
                                        .child(badge(rt.status.label(zh).to_string(), bg, fg))
                                        .when_some(rt.pid, |d, pid| {
                                            d.child(badge(format!("PID {pid}"), 0x1e293b, 0xcbd5e1))
                                        })
                                        .child(badge(
                                            if zh {
                                                format!("实际端口: {ports_text}")
                                            } else {
                                                format!("Actual Ports: {ports_text}")
                                            },
                                            0x1e293b,
                                            0x93c5fd,
                                        )),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .when_some(web_port, |d, port| {
                                            let url = format!("http://localhost:{port}");
                                            d.child(
                                                Button::new("open-browser")
                                                    .primary()
                                                    .label(if zh {
                                                        format!("打开浏览器 :{port}")
                                                    } else {
                                                        format!("Open Browser :{port}")
                                                    })
                                                    .on_click(cx.listener(move |this, _, _, cx| {
                                                        this.open_browser_url(&url, cx);
                                                    })),
                                            )
                                        })
                                        .when_some(selected_id.clone(), |d, sid| {
                                            d.child(
                                                Button::new("clear-logs")
                                                    .ghost()
                                                    .label(if zh { "清空日志" } else { "Clear" })
                                                    .on_click(cx.listener(move |this, _, _, cx| {
                                                        this.clear_service_logs(&sid, cx);
                                                    })),
                                            )
                                        }),
                                ),
                        )
                        // Log search, stream filter & run history bar
                        .child(
                            div()
                                .flex()
                                .flex_wrap()
                                .items_center()
                                .gap_2()
                                .child(div().w(px(240.)).child(Input::new(&app.log_search_input)))
                                .children(
                                    [
                                        (LogStreamFilter::All, if zh { "全部" } else { "All" }),
                                        (LogStreamFilter::Stdout, "Stdout"),
                                        (LogStreamFilter::Stderr, "Stderr"),
                                        (LogStreamFilter::System, "System"),
                                    ]
                                    .into_iter()
                                    .enumerate()
                                    .map(|(ix, (f, lbl))| {
                                        Button::new(("log-f", ix))
                                            .when(stream_filter == f, |b| b.primary())
                                            .when(stream_filter != f, |b| b.ghost())
                                            .label(lbl)
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                this.log_stream_filter = f;
                                                cx.notify();
                                            }))
                                    }),
                                )
                                .when(!rt.history.is_empty(), |d| {
                                    d.child(
                                        Button::new("hist-live")
                                            .when(selected_hist.is_none(), |b| b.primary())
                                            .when(selected_hist.is_some(), |b| b.ghost())
                                            .label(if zh { "当前日志" } else { "Live" })
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.selected_run_history_idx = None;
                                                cx.notify();
                                            })),
                                    )
                                    .children(rt.history.iter().enumerate().map(|(hix, rec)| {
                                        Button::new(("hist-run", hix))
                                            .when(selected_hist == Some(hix), |b| b.primary())
                                            .when(selected_hist != Some(hix), |b| b.ghost())
                                            .label(format!("#{} ({})", hix + 1, rec.started_at))
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                this.selected_run_history_idx = Some(hix);
                                                cx.notify();
                                            }))
                                    }))
                                }),
                        )
                        .child(
                            div()
                                .id("svc-logs-scroll")
                                .flex_1()
                                .overflow_y_scroll()
                                .p_2()
                                .rounded_md()
                                .bg(rgb(0x0b1120))
                                .flex()
                                .flex_col()
                                .gap_0p5()
                                .when(filtered_logs.is_empty(), |d| {
                                    d.child(
                                        div()
                                            .text_xs()
                                            .text_color(rgb(0x64748b))
                                            .child(if zh {
                                                "暂无日志。保存配置并点击「确认执行」即可实时查看输出。"
                                            } else {
                                                "No logs yet. Save config and click 'Confirm & Start' to stream output."
                                            }),
                                    )
                                })
                                .children(filtered_logs.into_iter().map(|line| {
                                    let color = match line.stream {
                                        LogStream::Stdout => rgb(0xe2e8f0),
                                        LogStream::Stderr => rgb(0xfca5a5),
                                        LogStream::System => rgb(0x7dd3fc),
                                    };
                                    div()
                                        .text_xs()
                                        .text_color(color)
                                        .child(format!("[{}] {}", line.timestamp, line.text))
                                })),
                        )
                }),
        )
}
