use xim_ctext::{compound_text_to_utf8, utf8_to_compound_text};

#[test]
fn chinese_ime_compound_text_can_switch_character_sets_within_one_message() {
    // Real Xlib Xutf8TextListToTextProperty output: even this Chinese phrase
    // switches between GB2312 and JIS X0208 for different characters.
    let phrase = b"\x1b$(A2bJT\x1b$(BJ,\x1b$(AWi";
    assert_eq!(compound_text_to_utf8(phrase).unwrap(), "测试分组");
    assert_eq!(
        compound_text_to_utf8(b"\x1b$(A2bJT\x1b(Babc\x1b$(BJ,\x1b$(AWi").unwrap(),
        "测试abc分组"
    );
    assert_eq!(
        compound_text_to_utf8(b"\x1b$(A2bJT\x1b(B'fen").unwrap(),
        "测试'fen"
    );
}

#[test]
fn chinese_preedit_updates_and_deletion_decode_without_panicking() {
    for (wire, expected) in [
        (b"ce".as_slice(), "ce"),
        (b"\x1b$(A2b".as_slice(), "测"),
        (b"\x1b$(A2bJT".as_slice(), "测试"),
        (b"\x1b$(A2bJT\x1b(B'fen".as_slice(), "测试'fen"),
        (b"".as_slice(), ""),
    ] {
        assert_eq!(compound_text_to_utf8(wire).unwrap(), expected);
    }
    assert_eq!(
        compound_text_to_utf8(b"\x1b$)A\xb2\xe2\xca\xd4 abc").unwrap(),
        "测试 abc"
    );
}

#[test]
fn ime_utf8_and_japanese_text_still_decode() {
    let text = "中文😀e\u{301}";
    assert_eq!(
        compound_text_to_utf8(&utf8_to_compound_text(text)).unwrap(),
        text
    );
    assert_eq!(
        compound_text_to_utf8(b"\x1b$(BCfF|4Z\x1b%G\xf0\x9f\x98\x80\x1b%@").unwrap(),
        "中日韓😀"
    );
    assert_eq!(compound_text_to_utf8(b"\x1b$(BEl5~").unwrap(), "東京");
}

#[test]
fn invalid_ime_payloads_return_errors_instead_of_panicking() {
    for bytes in [
        b"\x1b%G".as_slice(),
        b"\x1b%G\xff\x1b%@".as_slice(),
        b"\x1b$(A2".as_slice(),
        b"\x1b$(A2\x1b".as_slice(),
        b"\x1b$(Z".as_slice(),
        b"\x1b".as_slice(),
    ] {
        assert!(compound_text_to_utf8(bytes).is_err(), "{bytes:?}");
    }
}
