use anyhow::{Context, Result};
use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use std::{
    io::{Read, Write},
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
};

pub struct TerminalSession {
    pub parser: Arc<Mutex<vt100::Parser>>,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    master: Box<dyn MasterPty + Send>,
    child: Box<dyn Child + Send + Sync>,
    processes: Arc<Mutex<super::terminal_processes::TerminalProcesses>>,
    pub size: (u16, u16),
    pub exited: Arc<Mutex<bool>>,
    pub dirty: Arc<AtomicBool>,
}
impl TerminalSession {
    pub fn spawn(path: &Path, shell: &str) -> Result<Self> {
        let size = PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        };
        let pair = native_pty_system().openpty(size)?;
        let mut command = CommandBuilder::new(shell);
        let owner = uuid::Uuid::new_v4();
        command.env("MINTOOL_TERMINAL_ID", owner.to_string());
        command.cwd(path);
        command.env("TERM", "xterm-256color");
        command.env("COLORTERM", "truecolor");
        let child = pair
            .slave
            .spawn_command(command)
            .context("无法启动 Shell")?;
        let processes = super::terminal_processes::TerminalProcesses::track(
            child.process_id().context("无法取得终端进程 ID")?,
            owner,
        );
        drop(pair.slave);
        let mut reader = pair.master.try_clone_reader()?;
        let writer = Arc::new(Mutex::new(pair.master.take_writer()?));
        let thread_writer = writer.clone();
        let parser = Arc::new(Mutex::new(vt100::Parser::new(24, 80, 5000)));
        let thread_parser = parser.clone();
        let exited = Arc::new(Mutex::new(false));
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
                        if let Ok(mut parser) = thread_parser.lock() {
                            for byte in &buffer[..count] {
                                parser.process(std::slice::from_ref(byte));
                                queries.cursor = parser.screen().cursor_position();
                                query_parser.advance(&mut queries, *byte);
                            }
                        }
                        if !queries.replies.is_empty() {
                            if let Ok(mut writer) = thread_writer.lock() {
                                let _ = writer.write_all(&queries.replies);
                                let _ = writer.flush();
                            }
                            queries.replies.clear();
                        }
                        thread_dirty.store(true, Ordering::Relaxed);
                    }
                }
            }
            if let Ok(mut exited) = thread_exited.lock() {
                *exited = true;
            }
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
        self.parser.lock().unwrap().set_scrollback(0);
        let mut writer = self.writer.lock().unwrap();
        writer.write_all(data)?;
        writer.flush()?;
        Ok(())
    }
    pub fn poll_processes(&mut self) {
        self.processes.lock().unwrap().poll();
        if self.child.try_wait().ok().flatten().is_some() {
            self.processes.lock().unwrap().shutdown();
            *self.exited.lock().unwrap() = true;
        }
    }
    pub fn shutdown(&mut self) {
        self.processes.lock().unwrap().shutdown();
        let _ = self.child.kill();
        let _ = self.child.wait();
        *self.exited.lock().unwrap() = true;
    }
    pub fn resize(&mut self, rows: u16, cols: u16) -> Result<()> {
        let rows = rows.max(1);
        let cols = cols.max(2);
        if self.size != (rows, cols) {
            self.master.resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })?;
            self.parser.lock().unwrap().set_size(rows, cols);
            self.size = (rows, cols);
        }
        Ok(())
    }
    pub fn scroll(&mut self, lines: i32) {
        let mut parser = self.parser.lock().unwrap();
        let offset = parser.screen().scrollback() as i32;
        parser.set_scrollback((offset + lines).max(0) as usize);
    }
}
impl Drop for TerminalSession {
    fn drop(&mut self) {
        self.shutdown();
    }
}
