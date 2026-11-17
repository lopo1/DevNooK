use crate::app::{ActiveTab, DevNookApp};
use devnook_core::models::{EditorKind, ProjectCategory, ViewMode};
use crate::ui::{badge, palette};
use gpui_kit::component::{
    button::{Button, ButtonVariants as _},
    checkbox::Checkbox,
    input::Input,
};
use gpui_kit::{prelude::*, *};
use std::collections::HashSet;

pub fn render_projects_view(app: &mut DevNookApp, cx: &mut Context<DevNookApp>) -> impl IntoElement {
    let zh = app.is_zh();
    let pal = palette(app.data.dark_mode);
    let query = app.project_search_input.read(cx).value().to_lowercase();
    let only_fav = app.filter_favorites_only;
    let only_listening = app.filter_listening_only;
    let selected_cat = app.filter_category;
    let preferred_editor = app.data.preferred_editor;
    let view_mode = app.data.view_mode;

    let listening_project_ids: HashSet<String> = app
        .port_entries
        .iter()
        .filter_map(|p| p.matched_project_id.clone())
        .collect();

    let filtered: Vec<_> = app
        .data
        .projects
        .iter()
        .filter(|p| {
            if only_fav && !p.favorite {
                return false;
            }
            if only_listening && !listening_project_ids.contains(&p.id) {
                return false;
            }
            if let Some(cat) = selected_cat {
                if p.category != cat {
                    return false;
                }
            }
            if !query.is_empty() {
                let hay = format!(
                    "{} {} {}",
                    p.name.to_lowercase(),
                    p.path.to_string_lossy().to_lowercase(),
                    p.tags.join(" ").to_lowercase()
                );
                if !hay.contains(&query) {
                    return false;
                }
            }
            true
        })
        .cloned()
        .collect();

    let candidates_opt = app.scan_candidates.clone();

    div()
        .flex()
        .flex_col()
        .size_full()
        .gap_3()
        // Top toolbar
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
                        .gap_2()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .flex_1()
                                .child(
                                    div()
                                        .w(px(260.))
                                        .child(Input::new(&app.project_search_input)),
                                )
                                .child(
                                    Button::new("scan-preview")
                                        .primary()
                                        .label(if zh { "扫描预览" } else { "Scan & Preview" })
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.open_scan_preview(cx);
                                        })),
                                )
                                .child(
                                    Button::new("view-mode-toggle")
                                        .ghost()
                                        .label(match (view_mode, zh) {
                                            (ViewMode::List, true) => "切换卡片",
                                            (ViewMode::List, false) => "Card View",
                                            (ViewMode::Card, true) => "切换列表",
                                            (ViewMode::Card, false) => "List View",
                                        })
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.toggle_view_mode(cx);
                                        })),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .child(
                                    div()
                                        .w(px(260.))
                                        .child(Input::new(&app.import_path_input)),
                                )
                                .child(
                                    Button::new("import-project")
                                        .label(if zh { "导入路径" } else { "Import Path" })
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.import_project_or_root(window, cx);
                                        })),
                                ),
                        ),
                )
                // Filters & Preferred Editor
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_2()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_1()
                                .child(
                                    Button::new("cat-all")
                                        .when(
                                            selected_cat.is_none() && !only_fav && !only_listening,
                                            |b| b.primary(),
                                        )
                                        .when(
                                            selected_cat.is_some() || only_fav || only_listening,
                                            |b| b.ghost(),
                                        )
                                        .label(if zh { "全部" } else { "All" })
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.filter_category = None;
                                            this.filter_favorites_only = false;
                                            this.filter_listening_only = false;
                                            cx.notify();
                                        })),
                                )
                                .child(
                                    Button::new("cat-fav")
                                        .when(only_fav, |b| b.primary())
                                        .when(!only_fav, |b| b.ghost())
                                        .label(if zh { "★ 收藏" } else { "★ Favorites" })
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.filter_favorites_only = !this.filter_favorites_only;
                                            cx.notify();
                                        })),
                                )
                                .child(
                                    Button::new("cat-listen")
                                        .when(only_listening, |b| b.primary())
                                        .when(!only_listening, |b| b.ghost())
                                        .label(if zh { "● 监听中" } else { "● Listening" })
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.filter_listening_only = !this.filter_listening_only;
                                            cx.notify();
                                        })),
                                )
                                .children(ProjectCategory::all().iter().enumerate().map(
                                    |(ix, &cat)| {
                                        let active = selected_cat == Some(cat);
                                        Button::new(("cat", ix))
                                            .when(active, |b| b.primary())
                                            .when(!active, |b| b.ghost())
                                            .label(cat.label())
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                this.filter_category =
                                                    if this.filter_category == Some(cat) {
                                                        None
                                                    } else {
                                                        Some(cat)
                                                    };
                                                cx.notify();
                                            }))
                                    },
                                )),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_1()
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(rgb(pal.muted))
                                        .child(if zh { "默认编辑器:" } else { "Editor:" }),
                                )
                                .children(EditorKind::all().iter().take(5).enumerate().map(
                                    |(ix, &ed)| {
                                        let active = preferred_editor == ed;
                                        Button::new(("ed", ix))
                                            .when(active, |b| b.primary())
                                            .when(!active, |b| b.ghost())
                                            .label(ed.label())
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                this.set_preferred_editor(ed, cx);
                                            }))
                                    },
                                )),
                        ),
                )
                // Scan roots chips
                .when(!app.data.scan_roots.is_empty(), |d| {
                    d.child(
                        div()
                            .flex()
                            .flex_wrap()
                            .items_center()
                            .gap_1p5()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(pal.muted))
                                    .child(if zh { "扫描目录:" } else { "Scan Roots:" }),
                            )
                            .children(app.data.scan_roots.iter().enumerate().map(|(ix, root)| {
                                Button::new(("rm-root", ix))
                                    .ghost()
                                    .label(format!("{} ×", root.display()))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.remove_scan_root(ix, cx);
                                    }))
                            })),
                    )
                }),
        )
        // Scan Preview Panel (when open)
        .when_some(candidates_opt, |d, candidates| {
            let selected_count = candidates.iter().filter(|c| c.selected).count();
            d.child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .p_3()
                    .rounded_lg()
                    .bg(rgb(pal.surface))
                    .border_1()
                    .border_color(rgb(pal.accent))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(if zh {
                                        format!("扫描候选预览（共 {} 个，已选 {} 个，只读扫描不执行命令）", candidates.len(), selected_count)
                                    } else {
                                        format!("Scan Preview ({} found, {} selected, read-only)", candidates.len(), selected_count)
                                    }),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        Button::new("import-selected-cands")
                                            .primary()
                                            .label(if zh { "确认导入选中" } else { "Import Selected" })
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.confirm_scan_import(cx);
                                            })),
                                    )
                                    .child(
                                        Button::new("cancel-scan-preview")
                                            .ghost()
                                            .label(if zh { "关闭" } else { "Close" })
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.scan_candidates = None;
                                                cx.notify();
                                            })),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .id("scan-cands-scroll")
                            .max_h(px(200.))
                            .overflow_y_scroll()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .children(candidates.into_iter().enumerate().map(|(ix, cand)| {
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .px_2()
                                    .py_1()
                                    .rounded_md()
                                    .border_1()
                                    .border_color(rgb(pal.border))
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap_2()
                                            .child(
                                                Checkbox::new(("cand-chk", ix))
                                                    .label(cand.project.name.clone())
                                                    .checked(cand.selected)
                                                    .on_change(cx.listener(
                                                        move |this, checked: &bool, _, cx| {
                                                            if let Some(ref mut list) =
                                                                this.scan_candidates
                                                            {
                                                                if let Some(item) = list.get_mut(ix)
                                                                {
                                                                    item.selected = *checked;
                                                                }
                                                            }
                                                            cx.notify();
                                                        },
                                                    )),
                                            )
                                            .child(badge(
                                                cand.project.category.label(),
                                                0x1e293b,
                                                0x93c5fd,
                                            ))
                                            .when(cand.already_imported, |x| {
                                                x.child(badge(
                                                    if zh { "已导入" } else { "Imported" },
                                                    0x064e3b,
                                                    0x6ee7b7,
                                                ))
                                            }),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(rgb(pal.muted))
                                            .child(cand.project.path.to_string_lossy().to_string()),
                                    )
                            })),
                    ),
            )
        })
        // Project list / grid
        .child(
            div()
                .id("projects-scroll")
                .flex_1()
                .overflow_y_scroll()
                .flex()
                .when(view_mode == ViewMode::List, |d| d.flex_col().gap_2())
                .when(view_mode == ViewMode::Card, |d| d.flex_wrap().gap_3())
                .when(filtered.is_empty(), |this| {
                    this.child(
                        div()
                            .p_6()
                            .rounded_lg()
                            .bg(rgb(pal.surface))
                            .border_1()
                            .border_color(rgb(pal.border))
                            .text_color(rgb(pal.muted))
                            .child(if zh {
                                "暂无匹配项目。点击上方「扫描预览」或粘贴目录路径导入。"
                            } else {
                                "No projects found. Click 'Scan & Preview' or paste a directory path above."
                            }),
                    )
                })
                .children(filtered.into_iter().enumerate().map(|(ix, proj)| {
                    let pid = proj.id.clone();
                    let pid_fav = proj.id.clone();
                    let pid_svc = proj.id.clone();
                    let pid_finder = proj.id.clone();
                    let pid_term = proj.id.clone();
                    let pid_rm = proj.id.clone();
                    let is_listening = listening_project_ids.contains(&proj.id);
                    let path_str = proj.path.to_string_lossy().to_string();
                    let editor_label = proj
                        .preferred_editor
                        .unwrap_or(preferred_editor)
                        .label();

                    div()
                        .id(("proj-card", ix))
                        .flex()
                        .when(view_mode == ViewMode::List, |d| {
                            d.items_center().justify_between()
                        })
                        .when(view_mode == ViewMode::Card, |d| {
                            d.w(px(380.)).flex_col().gap_2()
                        })
                        .p_3()
                        .rounded_lg()
                        .bg(rgb(pal.surface))
                        .border_1()
                        .border_color(rgb(pal.border))
                        .hover(|s| s.border_color(rgb(pal.accent)))
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .child(
                                            Button::new(("fav-btn", ix))
                                                .ghost()
                                                .label(if proj.favorite { "★" } else { "☆" })
                                                .on_click(cx.listener(move |this, _, _, cx| {
                                                    this.toggle_favorite(&pid_fav, cx);
                                                })),
                                        )
                                        .child(
                                            div()
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .child(proj.name.clone()),
                                        )
                                        .child(badge(proj.category.label(), 0x1e293b, 0x93c5fd))
                                        .when(is_listening, |d| {
                                            d.child(badge(
                                                if zh { "监听中" } else { "Listening" },
                                                0x064e3b,
                                                0x6ee7b7,
                                            ))
                                        })
                                        .when_some(proj.git_branch.clone(), |d, branch| {
                                            d.child(badge(
                                                format!("git:{branch}"),
                                                0x1f2937,
                                                0x86efac,
                                            ))
                                        }),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(rgb(pal.muted))
                                        .child(path_str),
                                )
                                .when(!proj.service_drafts.is_empty(), |d| {
                                    d.child(
                                        div()
                                            .flex()
                                            .flex_wrap()
                                            .items_center()
                                            .gap_1()
                                            .children(proj.service_drafts.iter().enumerate().map(
                                                |(dix, draft)| {
                                                    let pid_d = proj.id.clone();
                                                    let draft_clone = draft.clone();
                                                    let lbl = if let Some(p) = draft.expected_port {
                                                        format!("+ {} (:{p})", draft.name)
                                                    } else {
                                                        format!("+ {}", draft.name)
                                                    };
                                                    Button::new(("draft", ix * 100 + dix))
                                                        .ghost()
                                                        .label(lbl)
                                                        .on_click(cx.listener(
                                                            move |this, _, window, cx| {
                                                                this.create_service_from_draft(
                                                                    &pid_d,
                                                                    &draft_clone,
                                                                    window,
                                                                    cx,
                                                                );
                                                                this.active_tab = ActiveTab::Services;
                                                                cx.notify();
                                                            },
                                                        ))
                                                },
                                            )),
                                    )
                                }),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_1p5()
                                .child(
                                    Button::new(("finder-btn", ix))
                                        .ghost()
                                        .label("Finder")
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.open_project_with(&pid_finder, EditorKind::Finder, cx);
                                        })),
                                )
                                .child(
                                    Button::new(("term-btn", ix))
                                        .ghost()
                                        .label("Terminal")
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.open_project_with(&pid_term, EditorKind::Terminal, cx);
                                        })),
                                )
                                .child(
                                    Button::new(("reg-svc", ix))
                                        .ghost()
                                        .label(if zh { "+ 服务" } else { "+ Service" })
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            this.prepare_service_from_project(&pid_svc, window, cx);
                                            this.active_tab = ActiveTab::Services;
                                            cx.notify();
                                        })),
                                )
                                .child(
                                    Button::new(("rm-proj", ix))
                                        .ghost()
                                        .label(if zh { "移除" } else { "Remove" })
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.remove_project_from_library(&pid_rm, cx);
                                        })),
                                )
                                .child(
                                    Button::new(("open-ed", ix))
                                        .primary()
                                        .label(if zh {
                                            format!("用 {editor_label} 打开")
                                        } else {
                                            format!("Open in {editor_label}")
                                        })
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.open_project_in_editor(&pid, cx);
                                        })),
                                ),
                        )
                })),
        )
}
