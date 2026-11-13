use crate::{DecodeError, UTF8_END, UTF8_START};
use alloc::string::String;

#[derive(Clone, Copy)]
enum Charset {
    Ascii,
    Latin1,
    DoubleByte(&'static encoding_rs::Encoding),
}

fn multibyte(final_byte: u8) -> Result<Charset, DecodeError> {
    match final_byte {
        b'A' => Ok(Charset::DoubleByte(encoding_rs::GBK)),
        b'B' => Ok(Charset::DoubleByte(encoding_rs::EUC_JP)),
        b'C' => Ok(Charset::DoubleByte(encoding_rs::EUC_KR)),
        _ => Err(DecodeError::UnsupportedEncoding),
    }
}

/// Decode the GL/GR character sets used by X11 input methods. GB2312, JIS X0208
/// and KS C5601 may be interleaved with ASCII and UTF-8 within one message.
pub fn compound_text_to_utf8(bytes: &[u8]) -> Result<String, DecodeError> {
    // Preserve the original API's support for unescaped UTF-8 strings.
    if !bytes.contains(&0x1b) {
        return Ok(String::from_utf8(bytes.to_vec())?);
    }
    let mut out = String::new();
    let mut gl = Charset::Ascii;
    let mut gr = Charset::Latin1;
    let mut offset = 0;
    while offset < bytes.len() {
        let remaining = &bytes[offset..];
        if remaining.starts_with(UTF8_START) {
            let text = &remaining[UTF8_START.len()..];
            let end = text
                .windows(UTF8_END.len())
                .position(|window| window == UTF8_END)
                .ok_or(DecodeError::InvalidEncoding)?;
            out.push_str(&String::from_utf8(text[..end].to_vec())?);
            offset += UTF8_START.len() + end + UTF8_END.len();
            continue;
        }
        if remaining[0] == 0x1b {
            match remaining {
                [0x1b, b'$', b'(', charset, ..] => {
                    gl = multibyte(*charset)?;
                    offset += 4;
                }
                [0x1b, b'$', b')', charset, ..] => {
                    gr = multibyte(*charset)?;
                    offset += 4;
                }
                [0x1b, b'(', b'B', ..] => {
                    gl = Charset::Ascii;
                    offset += 3;
                }
                [0x1b, b'-', b'A', ..] => {
                    gr = Charset::Latin1;
                    offset += 3;
                }
                _ => return Err(DecodeError::UnsupportedEncoding),
            }
            continue;
        }
        let first = remaining[0];
        if matches!(first, b'\t' | b'\n' | b' ') {
            out.push(first as char);
            offset += 1;
            continue;
        }
        let high = first >= 0x80;
        match if high { gr } else { gl } {
            Charset::Ascii if (0x21..=0x7e).contains(&first) => {
                out.push(first as char);
                offset += 1;
            }
            Charset::Latin1 if first >= 0xa0 => {
                out.push(first as char);
                offset += 1;
            }
            Charset::DoubleByte(encoding) => {
                let second = *remaining.get(1).ok_or(DecodeError::InvalidEncoding)?;
                let range = if high { 0xa1..=0xfe } else { 0x21..=0x7e };
                if !range.contains(&first) || !range.contains(&second) {
                    return Err(DecodeError::InvalidEncoding);
                }
                // EUC-CN/JP/KR use the same character positions with the high bit set.
                let pair = [first | 0x80, second | 0x80];
                let decoded = encoding
                    .decode_without_bom_handling_and_without_replacement(&pair)
                    .ok_or(DecodeError::InvalidEncoding)?;
                out.push_str(&decoded);
                offset += 2;
            }
            _ => return Err(DecodeError::InvalidEncoding),
        }
    }
    Ok(out)
}
