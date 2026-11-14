use super::mutex::lock_unpoisoned;
use std::{
    collections::{HashMap, HashSet},
    fs,
    sync::{Arc, Mutex, OnceLock, Weak},
    thread,
    time::{Duration, Instant},
};

#[derive(Clone, Copy)]
struct Process {
    pid: u32,
    parent: u32,
    group: u32,
    session: u32,
    started: u64,
    zombie: bool,
    owner: Option<uuid::Uuid>,
}

/// Tracks identities as well as PIDs so a reused PID cannot target another app.
pub struct TerminalProcesses {
    owner: uuid::Uuid,
    owned: HashMap<u32, u64>,
    sessions: HashMap<u32, u64>,
    groups: HashMap<u32, u64>,
    last_scan: Instant,
    closed: bool,
}

type Shared = Arc<Mutex<TerminalProcesses>>;
type SnapshotCache = Mutex<Option<(Instant, HashMap<u32, Process>)>>;
static REGISTRY: OnceLock<Mutex<Vec<Weak<Mutex<TerminalProcesses>>>>> = OnceLock::new();
static SNAPSHOT: OnceLock<SnapshotCache> = OnceLock::new();

impl TerminalProcesses {
    pub fn track(pid: u32, owner: uuid::Uuid) -> Shared {
        let mut processes = Self {
            owner,
            owned: HashMap::new(),
            sessions: HashMap::new(),
            groups: HashMap::new(),
            last_scan: Instant::now(),
            closed: false,
        };
        if let Some(process) = read_process(pid) {
            processes.add(process);
        }
        let processes = Arc::new(Mutex::new(processes));
        let mut registry = lock_unpoisoned(REGISTRY.get_or_init(Default::default));
        registry.retain(|processes| processes.strong_count() > 0);
        registry.push(Arc::downgrade(&processes));
        processes
    }

    fn add(&mut self, process: Process) {
        self.owned.insert(process.pid, process.started);
        // Only claim process groups/sessions whose leader belongs to this terminal.
        if process.pid == process.session {
            self.sessions.insert(process.pid, process.started);
        }
        if process.pid == process.group {
            self.groups.insert(process.pid, process.started);
        }
    }

    pub fn poll(&mut self) {
        if !self.closed && self.last_scan.elapsed() >= Duration::from_millis(250) {
            self.refresh(false);
        }
    }

    fn refresh(&mut self, fresh: bool) {
        let processes = snapshot(fresh);
        self.last_scan = Instant::now();
        self.sessions.retain(|pid, started| {
            processes
                .get(pid)
                .is_none_or(|process| process.started == *started)
        });
        self.groups.retain(|pid, started| {
            processes
                .get(pid)
                .is_none_or(|process| process.started == *started)
        });
        let mut owned: HashSet<_> = self
            .owned
            .iter()
            .filter(|(pid, started)| {
                processes
                    .get(pid)
                    .is_some_and(|process| process.started == **started)
            })
            .map(|(pid, _)| *pid)
            .collect();
        loop {
            let before = owned.len();
            for process in processes.values() {
                if process.owner == Some(self.owner)
                    || owned.contains(&process.pid)
                    || owned.contains(&process.parent)
                    || self.sessions.contains_key(&process.session)
                    || self.groups.contains_key(&process.group)
                {
                    owned.insert(process.pid);
                }
            }
            if before == owned.len() {
                break;
            }
        }
        for pid in owned {
            if let Some(&process) = processes.get(&pid) {
                self.add(process);
            }
        }
    }

    fn signal(&mut self, signal: i32) {
        self.refresh(true);
        for (&pid, &started) in &self.owned {
            if pid > 1
                && read_process(pid)
                    .is_some_and(|process| process.started == started && !process.zombie)
            {
                // Pin the process before checking its identity again, closing
                // the PID-reuse race between reading /proc and signalling.
                let descriptor = unsafe { libc::syscall(libc::SYS_pidfd_open, pid, 0) };
                if descriptor >= 0 {
                    if read_process(pid).is_some_and(|process| process.started == started) {
                        unsafe {
                            libc::syscall(
                                libc::SYS_pidfd_send_signal,
                                descriptor,
                                signal,
                                std::ptr::null::<libc::siginfo_t>(),
                                0,
                            );
                        }
                    }
                    unsafe {
                        libc::close(descriptor as i32);
                    }
                }
            }
        }
    }

    pub fn shutdown(&mut self) {
        if self.closed {
            return;
        }
        self.signal(libc::SIGTERM);
        thread::sleep(Duration::from_millis(150));
        self.signal(libc::SIGKILL);
        self.closed = true;
    }

    /// Called before the application entity store is destroyed, in one grace period.
    pub fn shutdown_all() {
        let registry = REGISTRY.get_or_init(Default::default);
        let processes: Vec<_> = lock_unpoisoned(registry)
            .iter()
            .filter_map(Weak::upgrade)
            .collect();
        let mut active = Vec::new();
        for processes in processes {
            let mut guard = lock_unpoisoned(&processes);
            if !guard.closed {
                guard.signal(libc::SIGTERM);
                active.push(processes.clone());
            }
        }
        if !active.is_empty() {
            thread::sleep(Duration::from_millis(150));
        }
        for processes in active {
            let mut guard = lock_unpoisoned(&processes);
            guard.signal(libc::SIGKILL);
            guard.closed = true;
        }
    }
}

fn snapshot(fresh: bool) -> HashMap<u32, Process> {
    let mut cache = lock_unpoisoned(SNAPSHOT.get_or_init(Default::default));
    if !fresh {
        if let Some((time, processes)) = cache.as_ref() {
            if time.elapsed() < Duration::from_millis(250) {
                return processes.clone();
            }
        }
    }
    let processes: HashMap<_, _> = fs::read_dir("/proc")
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter_map(|entry| entry.file_name().to_str()?.parse().ok())
        .filter_map(read_process)
        .map(|process| (process.pid, process))
        .collect();
    *cache = Some((Instant::now(), processes.clone()));
    processes
}
fn read_process(pid: u32) -> Option<Process> {
    let stat = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let (_, fields) = stat.rsplit_once(") ")?;
    let fields: Vec<_> = fields.split_whitespace().collect();
    Some(Process {
        pid,
        parent: fields.get(1)?.parse().ok()?,
        group: fields.get(2)?.parse().ok()?,
        session: fields.get(3)?.parse().ok()?,
        started: fields.get(19)?.parse().ok()?,
        zombie: fields.first()? == &"Z",
        owner: fs::read(format!("/proc/{pid}/environ"))
            .ok()
            .and_then(|environment| {
                environment.split(|byte| *byte == 0).find_map(|field| {
                    let value = field.strip_prefix(b"MINTOOL_TERMINAL_ID=")?;
                    uuid::Uuid::parse_str(std::str::from_utf8(value).ok()?).ok()
                })
            }),
    })
}
