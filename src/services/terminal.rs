use super::mutex::lock_unpoisoned;
use anyhow::{Context, Result};
use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use std::{
    io::{Read, Write},
    num::NonZeroU16,
    panic::{catch_unwind, AssertUnwindSafe},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
};

const SCROLLBACK_LINES: usize = 2000;

/// Everything needed to start a terminal process, independent of the module
/// that requested it (local project shell, SSH connection, future Docker).
#[derive(Clone, Debug, Default)]
pub struct TerminalSpec {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub envs: Vec<(String, String)>,
}
impl TerminalSpec {
    pub fn local_shell(path: PathBuf, shell: &str) -> Self {
        Self {
            program: shell.into(),
            args: Vec::new(),
            cwd: path,
            envs: Vec::new(),
        }
    }
}

fn process_parser_byte(parser: &mut vt100::Parser, byte: u8) {
    if catch_unwind(AssertUnwindSafe(|| {
        parser.process(std::slice::from_ref(&byte));
    }))
    .is_err()
    {
        let (rows, cols) = parser.screen().size();
        *parser = vt100::Parser::new(rows, cols, SCROLLBACK_LINES);
    }
}

pub struct TerminalSession {
    pub parser: Arc<Mutex<vt100::Parser>>,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    master: Box<dyn MasterPty + Send>,
    child: Box<dyn Child + Send + Sync>,
    processes: Arc<Mutex<super::terminal_processes::TerminalProcesses>>,
    pub size: (u16, u16),
    exited: Arc<AtomicBool>,
    pub dirty: Arc<AtomicBool>,
}
impl TerminalSession {
    pub fn spawn(path: &Path, shell: &str) -> Result<Self> {
        Self::spawn_spec(&TerminalSpec::local_shell(path.to_path_buf(), shell))
    }
    pub fn spawn_spec(spec: &TerminalSpec) -> Result<Self> {
        let size = PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        };
        let pair = native_pty_system().openpty(size)?;
        let mut command = CommandBuilder::new(&spec.program);
        for argument in &spec.args {
            command.arg(argument);
        }
        let owner = uuid::Uuid::new_v4();
        command.env("MINTOOL_TERMINAL_ID", owner.to_string());
        command.cwd(&spec.cwd);
        command.env("TERM", "xterm-256color");
        command.env("COLORTERM", "truecolor");
        for (name, value) in &spec.envs {
            command.env(name, value);
        }
        let child = pair
            .slave
            .spawn_command(command)
            .context("无法启动终端进程")?;
        let processes = super::terminal_processes::TerminalProcesses::track(
            child.process_id().context("无法取得终端进程 ID")?,
            owner,
        );
        drop(pair.slave);
        let mut reader = pair.master.try_clone_reader()?;
        let writer = Arc::new(Mutex::new(pair.master.take_writer()?));
        let thread_writer = writer.clone();
        let rows = NonZeroU16::new(size.rows).context("终端行数为零")?;
        let cols = NonZeroU16::new(size.cols).context("终端列数为零")?;
        let parser = Arc::new(Mutex::new(vt100::Parser::new(rows, cols, SCROLLBACK_LINES)));
        let thread_parser = parser.clone();
        let exited = Arc::new(AtomicBool::new(false));
        let thread_exited = exited.clone();
        let dirty = Arc::new(AtomicBool::new(true));
        let thread_dirty = dirty.clone();
        thread::spawn(move || {
            let mut buffer = [0u8; 16384];
            let mut query_parser = vte::Parser::new();
            let mut queries = super::terminal_query::TerminalQueries {
                cursor: (0, 0),
                replies: Vec::new(),
            };
            loop {
                match reader.read(&mut buffer) {
                    Ok(0) | Err(_) => break,
                    Ok(count) => {
                        let mut parser = lock_unpoisoned(&thread_parser);
                        for byte in &buffer[..count] {
                            process_parser_byte(&mut parser, *byte);
                            queries.cursor = parser.screen().cursor_position();
                            query_parser.advance(&mut queries, *byte);
                        }
                        drop(parser);
                        if !queries.replies.is_empty() {
                            let mut writer = lock_unpoisoned(&thread_writer);
                            let _ = writer.write_all(&queries.replies);
                            let _ = writer.flush();
                            queries.replies.clear();
                        }
                        thread_dirty.store(true, Ordering::Relaxed);
                    }
                }
            }
            thread_exited.store(true, Ordering::Relaxed);
            thread_dirty.store(true, Ordering::Relaxed);
        });
        Ok(Self {
            parser,
            writer,
            master: pair.master,
            child,
            processes,
            size: (24, 80),
            exited,
            dirty,
        })
    }
    pub fn send(&mut self, data: &[u8]) -> Result<()> {
        lock_unpoisoned(&self.parser).screen_mut().set_scrollback(0);
        let mut writer = lock_unpoisoned(&self.writer);
        writer.write_all(data)?;
        writer.flush()?;
        Ok(())
    }
    pub fn poll_processes(&mut self) {
        lock_unpoisoned(&self.processes).poll();
        if self.child.try_wait().ok().flatten().is_some() {
            lock_unpoisoned(&self.processes).shutdown();
            self.exited.store(true, Ordering::Relaxed);
        }
    }
    pub fn shutdown(&mut self) {
        lock_unpoisoned(&self.processes).shutdown();
        let _ = self.child.kill();
        let _ = self.child.wait();
        self.exited.store(true, Ordering::Relaxed);
    }
    pub fn has_exited(&self) -> bool {
        self.exited.load(Ordering::Relaxed)
    }
    pub fn bracketed_paste(&self) -> bool {
        lock_unpoisoned(&self.parser).screen().bracketed_paste()
    }
    pub fn application_cursor(&self) -> bool {
        lock_unpoisoned(&self.parser).screen().application_cursor()
    }
    pub fn scrollback_position(&self) -> (usize, usize) {
        let parser = lock_unpoisoned(&self.parser);
        (
            parser.screen().scrollback(),
            parser.screen().scrollback_len(),
        )
    }
    pub fn set_scrollback_position(&mut self, position: usize) {
        lock_unpoisoned(&self.parser)
            .screen_mut()
            .set_scrollback(position);
    }
    pub fn resize(&mut self, rows: u16, cols: u16) -> Result<()> {
        let rows = NonZeroU16::new(rows.max(1)).context("终端行数为零")?;
        let cols = NonZeroU16::new(cols.max(2)).context("终端列数为零")?;
        if self.size != (rows.get(), cols.get()) {
            self.master.resize(PtySize {
                rows: rows.get(),
                cols: cols.get(),
                pixel_width: 0,
                pixel_height: 0,
            })?;
            lock_unpoisoned(&self.parser).set_size(rows, cols);
            self.size = (rows.get(), cols.get());
        }
        Ok(())
    }
    pub fn scroll(&mut self, lines: i32) {
        let (position, _) = self.scrollback_position();
        self.set_scrollback_position((position as i32 + lines).max(0) as usize);
    }
}
impl Drop for TerminalSession {
    fn drop(&mut self) {
        self.shutdown();
    }
}
