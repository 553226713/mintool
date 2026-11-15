/// Preset label colors shared by project and SSH metadata.
pub const SWATCHES: [(&str, &str); 6] = [
    ("#6ee7b7", "绿"),
    ("#8aaff0", "蓝"),
    ("#c49de1", "紫"),
    ("#e1bd85", "橙"),
    ("#ee8598", "红"),
    ("#8c9aaa", "灰"),
];

pub fn color_value(hex: &str) -> u32 {
    u32::from_str_radix(hex.trim_start_matches('#'), 16).unwrap_or(0x8c9aaa)
}
