use crate::model::appearance::Appearance;
use adabraka_ui::theme::{install_theme, Theme};
use gpui::*;
use std::sync::atomic::{AtomicBool, Ordering};

// Match the component library's application-wide theme without locking and
// cloning its full token set for every terminal cell.
static LIGHT: AtomicBool = AtomicBool::new(false);
pub fn is_light() -> bool {
    LIGHT.load(Ordering::Relaxed)
}
fn color(dark: u32, light: u32) -> Hsla {
    rgb(if is_light() { light } else { dark }).into()
}
pub fn background() -> Hsla {
    color(0x0c1015, 0xf5f7fa)
}
pub fn panel() -> Hsla {
    color(0x131920, 0xffffff)
}
pub fn elevated() -> Hsla {
    color(0x1c2530, 0xe9eef4)
}
pub fn border() -> Hsla {
    color(0x27313d, 0xd4dce5)
}
pub fn muted() -> Hsla {
    color(0x8c9aaa, 0x59687a)
}
pub fn text() -> Hsla {
    color(0xe4eaf2, 0x202c3b)
}
pub fn accent() -> Hsla {
    color(0x6ee7b7, 0x14734e)
}
pub fn selected() -> Hsla {
    color(0x1b322a, 0xdeeee6)
}
pub fn selected_hover() -> Hsla {
    color(0x203c31, 0xd0e6da)
}
pub fn selected_border() -> Hsla {
    color(0x315946, 0x8bb6a1)
}
pub fn selection() -> Hsla {
    color(0x1c493c, 0xc9e6d7)
}
pub fn terminal_selection() -> Hsla {
    color(0x39476b, 0xc9d9f0)
}
pub fn changed_background() -> Hsla {
    color(0x25342e, 0xedf2df)
}
pub fn danger() -> Hsla {
    color(0xee8598, 0xb52b43)
}
pub fn error_background() -> Hsla {
    color(0x422a34, 0xfbe7ec)
}
pub fn success() -> Hsla {
    color(0x8ac9a5, 0x28794b)
}
pub fn warning() -> Hsla {
    color(0xe1bd85, 0x8a5b0f)
}
pub fn install(cx: &mut App, appearance: Appearance) {
    LIGHT.store(appearance == Appearance::Light, Ordering::Relaxed);
    let mut theme = if is_light() {
        Theme::light()
    } else {
        Theme::dark()
    };
    theme.tokens.background = background();
    theme.tokens.foreground = text();
    theme.tokens.card = panel();
    theme.tokens.card_foreground = text();
    theme.tokens.popover = elevated();
    theme.tokens.popover_foreground = text();
    theme.tokens.muted = elevated();
    theme.tokens.muted_foreground = muted();
    theme.tokens.border = border();
    theme.tokens.input = border();
    theme.tokens.primary = accent();
    theme.tokens.primary_foreground = background();
    theme.tokens.ring = accent();
    theme.tokens.accent = selected();
    theme.tokens.accent_foreground = accent();
    theme.tokens.destructive = danger();
    theme.tokens.font_family = "Inter".into();
    theme.tokens.font_mono = "JetBrains Mono".into();
    install_theme(cx, theme);
    cx.refresh_windows();
}
pub fn git_color(status: Option<&str>) -> Hsla {
    match status {
        Some(s) if s.contains('U') || s == "AA" || s == "DD" => danger(),
        Some(s) if s.contains('D') => danger(),
        Some("??") => success(),
        Some(s) if s.contains('A') => success(),
        Some(_) => warning(),
        None => text(),
    }
}
pub fn terminal_color(value: vt100::Color, default: Hsla) -> Hsla {
    match value {
        vt100::Color::Default => default,
        vt100::Color::Rgb(r, g, b) => rgb(((r as u32) << 16) | ((g as u32) << 8) | b as u32).into(),
        vt100::Color::Idx(i) => {
            const COLORS: [u32; 16] = [
                0x242933, 0xe27b86, 0x91c9a1, 0xe6c38c, 0x8aaff0, 0xc49de1, 0x89c9cf, 0xe4eaf2,
                0x737f94, 0xf099a3, 0xade2bc, 0xf2d8af, 0xaac9ff, 0xddbbf5, 0xa7e0e4, 0xf4f6fa,
            ];
            if i < 16 {
                const LIGHT_COLORS: [u32; 16] = [
                    0x202c3b, 0xb72c42, 0x28794b, 0x876214, 0x2a60ae, 0x8144a5, 0x21747b, 0x647080,
                    0x59687a, 0xc33750, 0x277b46, 0x886011, 0x316cbb, 0x9250b6, 0x247a84, 0x202c3b,
                ];
                rgb(if is_light() {
                    LIGHT_COLORS[i as usize]
                } else {
                    COLORS[i as usize]
                })
                .into()
            } else if i >= 232 {
                let n = (8 + (i - 232) * 10) as u32;
                rgb(n * 0x010101).into()
            } else {
                let n = i - 16;
                let v = |c: u8| -> u32 {
                    if c == 0 {
                        0
                    } else {
                        (55 + c * 40) as u32
                    }
                };
                rgb((v(n / 36) << 16) | (v(n / 6 % 6) << 8) | v(n % 6)).into()
            }
        }
    }
}
