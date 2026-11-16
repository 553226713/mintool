use super::{local::LocalEntry, sftp::RemoteEntry};
use crate::{app::MintoolApp, ui::theme};
use adabraka_ui::components::button::{Button, ButtonSize, ButtonVariant};
use gpui::*;
use std::cmp::Ordering;

fn compare<T: Ord>(a: T, b: T, descending: bool) -> Ordering {
    if descending {
        b.cmp(&a)
    } else {
        a.cmp(&b)
    }
}

impl MintoolApp {
    pub(crate) fn ssh_local_entries(&self) -> Vec<LocalEntry> {
        let panel = self.modules.ssh.panel();
        let mut entries = panel
            .local
            .as_ref()
            .map(|browser| browser.entries.clone())
            .unwrap_or_default();
        entries.retain(|entry| panel.preferences.show_hidden || !entry.name.starts_with('.'));
        entries.sort_by(|a, b| {
            b.directory
                .cmp(&a.directory)
                .then_with(|| match panel.preferences.sort.as_str() {
                    "size" => compare(a.size, b.size, panel.preferences.descending),
                    "modified" => compare(a.modified, b.modified, panel.preferences.descending),
                    _ => compare(
                        a.name.to_lowercase(),
                        b.name.to_lowercase(),
                        panel.preferences.descending,
                    ),
                })
                .then_with(|| a.name.cmp(&b.name))
        });
        entries
    }
    pub(crate) fn ssh_remote_entries(&self) -> Vec<RemoteEntry> {
        let panel = self.modules.ssh.panel();
        let entries = panel
            .remote
            .as_ref()
            .and_then(|browser| browser.entries.get(&browser.current))
            .cloned()
            .unwrap_or_default();
        sorted_remote(
            entries,
            &panel.preferences.sort,
            panel.preferences.descending,
            panel.preferences.show_hidden,
        )
    }
    pub(crate) fn ssh_listing_options(&self, cx: &mut Context<Self>) -> AnyElement {
        let preferences = &self.modules.ssh.panel().preferences;
        let mut row = div()
            .flex()
            .flex_wrap()
            .gap_1()
            .px_2()
            .py_1()
            .flex_none()
            .text_size(px(10.))
            .text_color(theme::muted());
        for (key, label) in [("name", "名称"), ("size", "大小"), ("modified", "修改时间")] {
            let active = preferences.sort == key || (preferences.sort.is_empty() && key == "name");
            row = row.child(
                Button::new(
                    SharedString::from(format!("sort-{key}")),
                    format!(
                        "{label}{}",
                        if active {
                            if preferences.descending {
                                " ↓"
                            } else {
                                " ↑"
                            }
                        } else {
                            ""
                        }
                    ),
                )
                .variant(ButtonVariant::Ghost)
                .size(ButtonSize::Sm)
                .selected(active)
                .on_click(cx.listener(move |app, _, _, cx| {
                    let preferences = &mut app.modules.ssh.panel_mut().preferences;
                    if preferences.sort == key {
                        preferences.descending = !preferences.descending;
                    } else {
                        preferences.sort = key.into();
                        preferences.descending = false;
                    }
                    app.changed(cx);
                })),
            );
        }
        row.child(
            Button::new("show-hidden", "隐藏文件")
                .selected(preferences.show_hidden)
                .variant(ButtonVariant::Ghost)
                .size(ButtonSize::Sm)
                .on_click(cx.listener(|app, _, _, cx| {
                    let p = &mut app.modules.ssh.panel_mut().preferences;
                    p.show_hidden = !p.show_hidden;
                    app.changed(cx);
                })),
        )
        .into_any_element()
    }
}

pub fn sorted_remote(
    mut entries: Vec<RemoteEntry>,
    sort: &str,
    descending: bool,
    show_hidden: bool,
) -> Vec<RemoteEntry> {
    entries.retain(|entry| show_hidden || !entry.name.starts_with('.'));
    match sort {
        "size" => entries.sort_by_key(|entry| entry.size),
        "modified" => entries.sort_by_cached_key(|entry| remote_date_key(&entry.modified)),
        _ => entries.sort_by_cached_key(|entry| entry.name.to_lowercase()),
    }
    if descending {
        entries.reverse();
    }
    entries.sort_by_key(|entry| !entry.directory);
    entries
}

fn remote_date_key(value: &str) -> (u64, usize, u64, u64, u64) {
    let fields = value.split_whitespace().collect::<Vec<_>>();
    if fields.len() < 3 {
        return (0, 0, 0, 0, 0);
    }
    let month = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ]
    .iter()
    .position(|name| *name == fields[0])
    .map_or(0, |i| i + 1);
    let mut year: u64 = 1970;
    let mut days = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        / 86400;
    loop {
        let length = if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) {
            366
        } else {
            365
        };
        if days < length {
            break;
        }
        days -= length;
        year += 1;
    }
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let mut current_month = 1u64;
    for length in [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ] {
        if days < length {
            break;
        }
        days -= length;
        current_month += 1;
    }
    let day = fields[1].parse().unwrap_or(0);
    if let Some((hour, minute)) = fields[2].split_once(':') {
        if month as u64 > current_month + 1 {
            year = year.saturating_sub(1);
        }
        (
            year,
            month,
            day,
            hour.parse().unwrap_or(0),
            minute.parse().unwrap_or(0),
        )
    } else {
        (fields[2].parse().unwrap_or(0), month, day, 0, 0)
    }
}

#[cfg(test)]
mod tests {
    use super::sorted_remote;
    use crate::modules::ssh::sftp::parse_listing;
    #[test]
    fn list_sorting_keeps_directories_first_and_hidden_files_optional() {
        let entries=parse_listing("-rw-r--r-- 1 user group 200 Jan 1 2025 z.txt\n-rw-r--r-- 1 user group 1 Feb 1 2025 a.txt\ndrwxr-xr-x 1 user group 4096 Jan 1 2024 dir\n-rw-r--r-- 1 user group 3 Jan 1 2026 .hidden\n","/");
        let sorted = sorted_remote(entries.clone(), "size", true, false);
        assert_eq!(
            sorted
                .iter()
                .map(|entry| entry.name.as_str())
                .collect::<Vec<_>>(),
            vec!["dir", "z.txt", "a.txt"]
        );
        let sorted = sorted_remote(entries, "modified", true, true);
        assert_eq!(
            sorted
                .iter()
                .map(|entry| entry.name.as_str())
                .collect::<Vec<_>>(),
            vec!["dir", ".hidden", "a.txt", "z.txt"]
        );
    }
}
