#[derive(Default, Clone, Copy)]
pub struct KeyModifiers {
    pub control: bool,
    pub shift: bool,
    pub alt: bool,
}

pub fn paste(text: &str, bracketed: bool) -> Vec<u8> {
    if bracketed {
        format!("\x1b[200~{text}\x1b[201~").into_bytes()
    } else {
        text.as_bytes().to_vec()
    }
}
pub fn key(key: &str, modifiers: KeyModifiers, application_cursor: bool) -> Option<Vec<u8>> {
    if modifiers.control && !modifiers.shift && !modifiers.alt {
        if key.len() == 1 && key.as_bytes()[0].is_ascii_alphabetic() {
            return Some(vec![key.as_bytes()[0].to_ascii_uppercase() - b'A' + 1]);
        }
        if key == "space" {
            return Some(vec![0]);
        }
    }
    let modifier = 1
        + u8::from(modifiers.shift)
        + 2 * u8::from(modifiers.alt)
        + 4 * u8::from(modifiers.control);
    let cursor = match key {
        "up" => Some("A"),
        "down" => Some("B"),
        "right" => Some("C"),
        "left" => Some("D"),
        "home" => Some("H"),
        "end" => Some("F"),
        _ => None,
    };
    if let Some(suffix) = cursor {
        return Some(if modifier > 1 {
            format!("\x1b[1;{modifier}{suffix}").into_bytes()
        } else if application_cursor && matches!(key, "up" | "down" | "left" | "right") {
            format!("\x1bO{suffix}").into_bytes()
        } else {
            format!("\x1b[{suffix}").into_bytes()
        });
    }
    let bytes = match key {
        "enter" => "\r",
        "backspace" => "\x7f",
        "escape" => "\x1b",
        "tab" if modifiers.shift => "\x1b[Z",
        "tab" => "\t",
        "delete" => "\x1b[3~",
        "insert" => "\x1b[2~",
        "pageup" => "\x1b[5~",
        "pagedown" => "\x1b[6~",
        "f1" => "\x1bOP",
        "f2" => "\x1bOQ",
        "f3" => "\x1bOR",
        "f4" => "\x1bOS",
        "f5" => "\x1b[15~",
        "f6" => "\x1b[17~",
        "f7" => "\x1b[18~",
        "f8" => "\x1b[19~",
        "f9" => "\x1b[20~",
        "f10" => "\x1b[21~",
        "f11" => "\x1b[23~",
        "f12" => "\x1b[24~",
        _ => return None,
    };
    Some(bytes.as_bytes().to_vec())
}
