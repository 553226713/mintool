use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::{
    collections::BTreeSet,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver},
        Arc,
    },
    time::{Duration, Instant},
};

#[derive(Default)]
pub struct FileEvents {
    pub paths: BTreeSet<PathBuf>,
    pub rescan: bool,
    pub error: Option<String>,
}

/// Native, non-recursive watches follow the visible tree and open documents.
/// The callback never reads files or touches UI state. Bursts are coalesced.
pub struct FileMonitor {
    watcher: Option<RecommendedWatcher>,
    receiver: Receiver<notify::Result<Event>>,
    overflow: Arc<AtomicBool>,
    watched: BTreeSet<PathBuf>,
    requested: Option<BTreeSet<PathBuf>>,
    pending: FileEvents,
    first_event: Option<Instant>,
    last_event: Option<Instant>,
}
impl Default for FileMonitor {
    fn default() -> Self {
        let (sender, receiver) = mpsc::sync_channel(1024);
        let overflow = Arc::new(AtomicBool::new(false));
        let callback_overflow = overflow.clone();
        let result = notify::recommended_watcher(move |event: notify::Result<Event>| {
            // Reading a preview must not cause a feedback loop.
            if event
                .as_ref()
                .is_ok_and(|event| matches!(event.kind, EventKind::Access(_)))
            {
                return;
            }
            if sender.try_send(event).is_err() {
                callback_overflow.store(true, Ordering::Relaxed);
            }
        });
        let (watcher, error) = match result {
            Ok(watcher) => (Some(watcher), None),
            Err(error) => (None, Some(format!("文件监听不可用：{error}"))),
        };
        Self {
            watcher,
            receiver,
            overflow,
            watched: BTreeSet::new(),
            requested: None,
            pending: FileEvents {
                error,
                ..FileEvents::default()
            },
            first_event: None,
            last_event: None,
        }
    }
}
impl FileMonitor {
    pub fn synchronize(&mut self, paths: BTreeSet<PathBuf>) -> BTreeSet<PathBuf> {
        let mut added = BTreeSet::new();
        if self.requested.as_ref() == Some(&paths) {
            return added;
        }
        self.requested = Some(paths.clone());
        let Some(watcher) = &mut self.watcher else {
            return added;
        };
        let obsolete: Vec<_> = self
            .watched
            .iter()
            .filter(|path| !paths.contains(*path) || !path.is_dir())
            .cloned()
            .collect();
        for path in obsolete {
            let _ = watcher.unwatch(&path);
            self.watched.remove(&path);
        }
        for path in paths.difference(&self.watched).cloned().collect::<Vec<_>>() {
            if !path.is_dir() {
                continue;
            }
            match watcher.watch(&path, RecursiveMode::NonRecursive) {
                Ok(()) => {
                    added.insert(path.clone());
                    self.watched.insert(path);
                }
                Err(error) => {
                    self.pending.error = Some(format!("无法监听 {}：{error}", path.display()))
                }
            }
        }
        added
    }
    pub fn retry(&mut self) {
        self.requested = None;
    }
    pub fn poll(&mut self) -> Option<FileEvents> {
        let mut received = false;
        // Bound each UI tick even while a build generates many files.
        for event in self.receiver.try_iter().take(1024) {
            received = true;
            match event {
                Ok(event) => {
                    self.pending.rescan |= event.need_rescan();
                    if matches!(
                        event.kind,
                        EventKind::Remove(_)
                            | EventKind::Modify(notify::event::ModifyKind::Name(_))
                    ) {
                        let obsolete: Vec<_> = self
                            .watched
                            .iter()
                            .filter(|watched| {
                                event.paths.iter().any(|path| watched.starts_with(path))
                            })
                            .cloned()
                            .collect();
                        for path in obsolete {
                            if let Some(watcher) = &mut self.watcher {
                                let _ = watcher.unwatch(&path);
                            }
                            self.watched.remove(&path);
                        }
                    }
                    self.pending.paths.extend(event.paths);
                }
                Err(error) => {
                    self.pending.rescan = true;
                    self.pending.error = Some(format!("文件监听出错：{error}"));
                }
            }
        }
        if self.overflow.swap(false, Ordering::Relaxed) {
            self.pending.rescan = true;
            received = true;
        }
        if received {
            self.first_event.get_or_insert_with(Instant::now);
            self.last_event = Some(Instant::now());
        }
        let ready = self.pending.error.is_some()
            || self
                .last_event
                .is_some_and(|time| time.elapsed() >= Duration::from_millis(80))
            || self
                .first_event
                .is_some_and(|time| time.elapsed() >= Duration::from_millis(250));
        if !ready {
            return None;
        }
        self.first_event = None;
        self.last_event = None;
        // Reconcile watches after deleted directories have been recreated or renamed.
        if self.pending.rescan {
            if let Some(watcher) = &mut self.watcher {
                for path in &self.watched {
                    let _ = watcher.unwatch(path);
                }
            }
            self.watched.clear();
        }
        if self.pending.rescan || !self.pending.paths.is_empty() {
            self.requested = None;
        }
        Some(std::mem::take(&mut self.pending))
    }
}
