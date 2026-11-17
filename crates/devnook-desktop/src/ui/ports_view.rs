use crate::app::{DevNookApp, PortFamilyFilter, PortOwnershipFilter};
use devnook_core::models::AddressFamily;
use crate::ui::{badge, palette};
use gpui_kit::component::{
    button::{Button, ButtonVariants as _},
    input::Input,
};
use gpui_kit::{prelude::*, *};

pub fn render_ports_view(app: &mut DevNookApp, cx: &mut Context<DevNookApp>) -> impl IntoElement {
    let zh = app.is_zh();
    let pal = palette(app.data.dark_mode);
    let query = app.port_search_input.read(cx).value().to_lowercase();
    let own_filter = app.port_ownership_filter;
    let fam_filter = app.port_family_filter;
    let confirm_kill = app.confirm_kill_pid;
    let expanded = app.expanded_port_cmds.clone();

    let entries: Vec<_> = app
        .port_entries
        .iter()
        .filter(|e| {
            match own_filter {
                PortOwnershipFilter::All => {}
                PortOwnershipFilter::Managed if !e.is_managed => return false,
                PortOwnershipFilter::External if e.is_managed => return false,
                _ => {}
            }
            match fam_filter {
                PortFamilyFilter::All => {}
                PortFamilyFilter::IPv4 if e.family != AddressFamily::IPv4 => return false,
                PortFamilyFilter::IPv6 if e.family != AddressFamily::IPv6 => return false,
                _ => {}
            }
            if !query.is_empty() {
                let hay = format!(
                    "{} {} {} {} {} {}",
                    e.port,
                    e.pid,
                    e.process_name.to_lowercase(),
                    e.command.to_lowercase(),
                    e.cwd
                        .as_ref()
                        .map(|p| p.to_string_lossy().to_lowercase())
                        .unwrap_or_default(),
                    e.matched_project.as_deref().unwrap_or("").to_lowercase(),
                );
                if !hay.contains(&query) {
                    return false;
                }
            }
            true
        })
        .cloned()
        .collect();

    div()
        .flex()
        .flex_col()
        .size_full()
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
                                .flex()
                                .items_center()
                                .gap_2()
                                .child(
                                    div()
                                        .w(px(320.))
                                        .child(Input::new(&app.port_search_input)),
                                )
                                .child(
                                    Button::new("refresh-ports")
                                        .primary()
                                        .label(if zh { "刷新端口" } else { "Refresh" })
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.refresh_ports(cx);
                                        })),
                                ),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(rgb(pal.muted))
                                .child(if zh {
                                    format!("共 {} 个监听端口（未检测到监听不代表项目已停止）", entries.len())
                                } else {
                                    format!("{} listening TCP ports", entries.len())
                                }),
                        ),
                )
                // Ownership & Address family filters
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_3()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_1()
                                .children(
                                    [
                                        (PortOwnershipFilter::All, if zh { "全部来源" } else { "All" }),
                                        (PortOwnershipFilter::Managed, if zh { "DevNook 托管" } else { "Managed" }),
                                        (PortOwnershipFilter::External, if zh { "外部进程" } else { "External" }),
                                    ]
                                    .into_iter()
                                    .enumerate()
                                    .map(|(ix, (f, lbl))| {
                                        Button::new(("own-f", ix))
                                            .when(own_filter == f, |b| b.primary())
                                            .when(own_filter != f, |b| b.ghost())
                                            .label(lbl)
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                this.port_ownership_filter = f;
                                                cx.notify();
                                            }))
                                    }),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_1()
                                .children(
                                    [
                                        (PortFamilyFilter::All, "IPv4 + IPv6"),
                                        (PortFamilyFilter::IPv4, "IPv4"),
                                        (PortFamilyFilter::IPv6, "IPv6"),
                                    ]
                                    .into_iter()
                                    .enumerate()
                                    .map(|(ix, (f, lbl))| {
                                        Button::new(("fam-f", ix))
                                            .when(fam_filter == f, |b| b.primary())
                                            .when(fam_filter != f, |b| b.ghost())
                                            .label(lbl)
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                this.port_family_filter = f;
                                                cx.notify();
                                            }))
                                    }),
                                ),
                        ),
                ),
        )
        .child(
            div()
                .id("ports-scroll")
                .flex_1()
                .overflow_y_scroll()
                .flex()
                .flex_col()
                .gap_2()
                .when(entries.is_empty(), |d| {
                    d.child(
                        div()
                            .p_6()
                            .rounded_lg()
                            .bg(rgb(pal.surface))
                            .border_1()
                            .border_color(rgb(pal.border))
                            .text_color(rgb(pal.muted))
                            .child(if zh { "未找到匹配的监听端口。" } else { "No matching listening TCP ports found." }),
                    )
                })
                .children(entries.into_iter().enumerate().map(|(ix, entry)| {
                    let pid = entry.pid;
                    let port = entry.port;
                    let is_confirming = confirm_kill == Some((pid, port));
                    let is_expanded = expanded.contains(&(pid, port));
                    let addrs = entry.bind_addrs.join(", ");
                    let cwd_text = entry
                        .cwd
                        .as_ref()
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_else(|| if zh { "未知工作目录".to_string() } else { "Unknown cwd".to_string() });

                    let cmd_display = if !is_expanded && entry.command.len() > 120 {
                        format!("{}...", &entry.command[..120])
                    } else {
                        entry.command.clone()
                    };

                    div()
                        .id(("port-row", ix))
                        .flex()
                        .items_center()
                        .justify_between()
                        .p_3()
                        .rounded_lg()
                        .bg(rgb(pal.surface))
                        .border_1()
                        .border_color(rgb(pal.border))
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .flex_1()
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .child(badge(format!(":{}", entry.port), 0x1e3a8a, 0x93c5fd))
                                        .child(
                                            div()
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .child(entry.process_name.clone()),
                                        )
                                        .child(badge(format!("PID {}", entry.pid), 0x1f2937, 0xd1d5db))
                                        .child(badge(addrs, 0x1f2937, 0x9ca3af))
                                        .child(badge(
                                            if entry.is_managed {
                                                if zh { "DevNook 托管" } else { "Managed" }
                                            } else if zh {
                                                "外部进程"
                                            } else {
                                                "External"
                                            },
                                            if entry.is_managed { 0x312e81 } else { 0x374151 },
                                            if entry.is_managed { 0xc7d2fe } else { 0xd1d5db },
                                        ))
                                        .when_some(entry.matched_project.clone(), |d, proj| {
                                            d.child(badge(
                                                if zh { format!("项目: {proj}") } else { format!("Project: {proj}") },
                                                0x064e3b,
                                                0x6ee7b7,
                                            ))
                                        }),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(rgb(pal.text))
                                                .child(format!("Cmd: {cmd_display}")),
                                        )
                                        .when(entry.command.len() > 120, |d| {
                                            d.child(
                                                Button::new(("exp-cmd", ix))
                                                    .ghost()
                                                    .label(if is_expanded {
                                                        if zh { "收起" } else { "Collapse" }
                                                    } else if zh {
                                                        "展开"
                                                    } else {
                                                        "Expand"
                                                    })
                                                    .on_click(cx.listener(move |this, _, _, cx| {
                                                        if !this.expanded_port_cmds.remove(&(pid, port)) {
                                                            this.expanded_port_cmds.insert((pid, port));
                                                        }
                                                        cx.notify();
                                                    })),
                                            )
                                        }),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(rgb(pal.muted))
                                        .child(format!("Dir: {cwd_text}")),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .when(!is_confirming, |d| {
                                    d.child(
                                        Button::new(("kill-ask", ix))
                                            .ghost()
                                            .label(if zh { "停止进程" } else { "Stop PID" })
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                this.confirm_kill_pid = Some((pid, port));
                                                cx.notify();
                                            })),
                                    )
                                })
                                .when(is_confirming, |d| {
                                    d.child(
                                        Button::new(("kill-confirm-term", ix))
                                            .primary()
                                            .label(if zh {
                                                format!("确认结束 PID {pid}")
                                            } else {
                                                format!("Confirm TERM {pid}")
                                            })
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                this.confirm_kill_pid = None;
                                                this.kill_port_pid(pid, false, cx);
                                            })),
                                    )
                                    .when(entry.is_managed, |x| {
                                        x.child(
                                            Button::new(("kill-confirm-force", ix))
                                                .ghost()
                                                .label("KILL")
                                                .on_click(cx.listener(move |this, _, _, cx| {
                                                    this.confirm_kill_pid = None;
                                                    this.kill_port_pid(pid, true, cx);
                                                })),
                                        )
                                    })
                                    .child(
                                        Button::new(("kill-cancel", ix))
                                            .ghost()
                                            .label(if zh { "取消" } else { "Cancel" })
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.confirm_kill_pid = None;
                                                cx.notify();
                                            })),
                                    )
                                }),
                        )
                })),
        )
}
