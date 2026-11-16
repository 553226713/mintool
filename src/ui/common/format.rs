/// Human-readable byte size shared by previews and transfer panels.
pub fn format_size(size: u64) -> String {
    if size >= 1024 * 1024 {
        format!("{:.1} MiB", size as f64 / 1024. / 1024.)
    } else if size >= 1024 {
        format!("{:.1} KiB", size as f64 / 1024.)
    } else {
        format!("{size} B")
    }
}

/// Local time for file metadata, independent of locale-dependent ls formatting.
pub fn format_timestamp(seconds: u64) -> String {
    let seconds = seconds.min(i64::MAX as u64) as libc::time_t;
    let mut value = std::mem::MaybeUninit::<libc::tm>::uninit();
    let result = unsafe { libc::localtime_r(&seconds, value.as_mut_ptr()) };
    if result.is_null() {
        return String::new();
    }
    let value = unsafe { value.assume_init() };
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}",
        value.tm_year + 1900,
        value.tm_mon + 1,
        value.tm_mday,
        value.tm_hour,
        value.tm_min
    )
}
