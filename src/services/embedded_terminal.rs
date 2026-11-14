use super::system_terminal::SystemTerminal;
use anyhow::{Context, Result};
use std::{
    path::Path,
    process::{Child, Stdio},
    sync::{Arc, Mutex},
};
use x11rb::{
    connection::Connection,
    protocol::xproto::{ConfigureWindowAux, ConnectionExt, CreateWindowAux, WindowClass},
    rust_connection::RustConnection,
    COPY_DEPTH_FROM_PARENT,
};

pub struct EmbeddedTerminal {
    connection: RustConnection,
    host: u32,
    process: Child,
    processes: Arc<Mutex<super::terminal_processes::TerminalProcesses>>,
    bounds: Option<(i16, i16, u16, u16)>,
    visible: bool,
    children: Vec<u32>,
}

impl EmbeddedTerminal {
    pub fn spawn(
        parent: u32,
        terminal: &SystemTerminal,
        directory: &Path,
        shell: &str,
    ) -> Result<Self> {
        let (connection, _) = x11rb::connect(None).context("无法连接 X11 桌面")?;
        let host = connection.generate_id()?;
        connection
            .create_window(
                COPY_DEPTH_FROM_PARENT,
                host,
                parent,
                0,
                0,
                1,
                1,
                0,
                WindowClass::INPUT_OUTPUT,
                0,
                &CreateWindowAux::new(),
            )?
            .check()?;
        connection.flush()?;
        let owner = uuid::Uuid::new_v4();
        let result = terminal
            .command(host, directory, shell)?
            .env("MINTOOL_TERMINAL_ID", owner.to_string())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
        let process = match result {
            Ok(process) => process,
            Err(error) => {
                let _ = connection.destroy_window(host);
                let _ = connection.flush();
                return Err(error).context("无法启动嵌入式系统终端");
            }
        };
        let processes = super::terminal_processes::TerminalProcesses::track(process.id(), owner);
        Ok(Self {
            connection,
            host,
            process,
            processes,
            bounds: None,
            visible: false,
            children: Vec::new(),
        })
    }
    pub fn position(&mut self, x: i16, y: i16, width: u16, height: u16) -> Result<()> {
        let bounds = (x, y, width.max(1), height.max(1));
        if self.bounds != Some(bounds) {
            self.connection
                .configure_window(
                    self.host,
                    &ConfigureWindowAux::new()
                        .x(x as i32)
                        .y(y as i32)
                        .width(bounds.2 as u32)
                        .height(bounds.3 as u32),
                )?
                .check()?;
            self.children.clear();
            self.bounds = Some(bounds);
        }
        self.refresh_children()?;
        self.show(true)
    }
    pub fn refresh_children(&mut self) -> Result<()> {
        let Some((_, _, width, height)) = self.bounds else {
            return Ok(());
        };
        let children = self.connection.query_tree(self.host)?.reply()?.children;
        if children != self.children {
            for child in &children {
                self.connection
                    .configure_window(
                        *child,
                        &ConfigureWindowAux::new()
                            .x(0)
                            .y(0)
                            .width(width as u32)
                            .height(height as u32),
                    )?
                    .check()?;
            }
            self.children = children;
            self.connection.flush()?;
        }
        Ok(())
    }
    pub fn show(&mut self, visible: bool) -> Result<()> {
        if self.visible != visible {
            if visible {
                self.connection.map_window(self.host)?.check()?;
            } else {
                self.connection.unmap_window(self.host)?.check()?;
            }
            self.visible = visible;
        }
        self.connection.flush()?;
        Ok(())
    }
    pub fn exited(&mut self) -> bool {
        self.processes.lock().unwrap().poll();
        let exited = self.process.try_wait().ok().flatten().is_some();
        if exited {
            self.processes.lock().unwrap().shutdown();
        }
        exited
    }
}
impl Drop for EmbeddedTerminal {
    fn drop(&mut self) {
        self.processes.lock().unwrap().shutdown();
        let _ = self.process.kill();
        let _ = self.process.wait();
        let _ = self.connection.destroy_window(self.host);
        let _ = self.connection.flush();
    }
}
