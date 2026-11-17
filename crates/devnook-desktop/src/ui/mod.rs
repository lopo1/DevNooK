pub mod pipelines_view;
pub mod ports_view;
pub mod projects_view;
pub mod services_view;

use gpui_kit::{prelude::*, *};

pub struct Palette {
    pub bg: u32,
    pub surface: u32,
    pub border: u32,
    pub text: u32,
    pub muted: u32,
    pub accent: u32,
}

pub fn palette(dark: bool) -> Palette {
    if dark {
        Palette {
            bg: 0x0f172a,
            surface: 0x1e293b,
            border: 0x334155,
            text: 0xf1f5f9,
            muted: 0x94a3b8,
            accent: 0x3b82f6,
        }
    } else {
        Palette {
            bg: 0xf8fafc,
            surface: 0xffffff,
            border: 0xe2e8f0,
            text: 0x0f172a,
            muted: 0x64748b,
            accent: 0x2563eb,
        }
    }
}

pub fn badge(label: impl Into<SharedString>, bg: u32, fg: u32) -> Div {
    div()
        .px_2()
        .py_0p5()
        .rounded_md()
        .text_xs()
        .bg(rgb(bg))
        .text_color(rgb(fg))
        .child(label.into())
}
