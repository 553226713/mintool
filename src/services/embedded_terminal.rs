use super::{mutex::lock_unpoisoned, system_terminal::SystemTerminal};
use anyhow::{Context, Result};
use std::{
    path::Path,
    process::{Child, Stdio},
    sync::{Arc, Mutex},
};
use x11rb::{
    connection::Connection,
    protocol::shape::{ConnectionExt as _, SK, SO},
    protocol::xproto::{
        ClipOrdering, ConfigureWindowAux, ConnectionExt, CreateWindowAux, Gravity, InputFocus,
        KeyButMask, Rectangle, WindowClass,
    },
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
    button_down: bool,
    menu: Option<(i16, i16, u16, u16, u16, u16)>,
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
                &CreateWindowAux::new().bit_gravity(Gravity::NORTH_WEST),
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
            button_down: false,
            menu: None,
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
    pub fn set_menu_bounds(&mut self, menu: Option<Rectangle>) -> Result<()> {
        let (Some((x, y, width, height)), Some(menu)) = (self.bounds, menu) else {
            if self.menu.is_some() {
                for kind in [SK::BOUNDING, SK::INPUT] {
                    self.connection
                        .shape_mask(SO::SET, kind, self.host, 0, 0, x11rb::NONE)?;
                }
                self.connection.flush()?;
                self.menu = None;
            }
            return Ok(());
        };
        let menu = Rectangle {
            x: menu.x.saturating_sub(x),
            y: menu.y.saturating_sub(y),
            ..menu
        };
        let shape = (menu.x, menu.y, menu.width, menu.height, width, height);
        if self.menu != Some(shape) {
            self.connection
                .shape_mask(SO::SET, SK::BOUNDING, self.host, 0, 0, x11rb::NONE)?;
            self.connection.shape_rectangles(
                SO::SUBTRACT,
                SK::BOUNDING,
                ClipOrdering::UNSORTED,
                self.host,
                0,
                0,
                &[menu],
            )?;
            self.connection.shape_rectangles(
                SO::SET,
                SK::INPUT,
                ClipOrdering::UNSORTED,
                self.host,
                0,
                0,
                &[],
            )?;
            self.connection.flush()?;
            self.menu = Some(shape);
        }
        Ok(())
    }
    pub fn poll_focus(&mut self) -> Result<bool> {
        if !self.visible || self.menu.is_some() {
            self.button_down = false;
            return Ok(false);
        }
        let pointer = self.connection.query_pointer(self.host)?.reply()?;
        let button_down = pointer.mask.contains(KeyButMask::BUTTON1);
        let clicked = button_down && !self.button_down && pointer.child != x11rb::NONE;
        if clicked {
            let target = deepest_child(&self.connection, pointer.child)?;
            self.connection
                .set_input_focus(InputFocus::POINTER_ROOT, target, x11rb::CURRENT_TIME)?
                .check()?;
            self.connection.flush()?;
        }
        self.button_down = button_down;
        Ok(clicked)
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
        lock_unpoisoned(&self.processes).poll();
        let exited = self.process.try_wait().ok().flatten().is_some();
        if exited {
            lock_unpoisoned(&self.processes).shutdown();
        }
        exited
    }
}

fn deepest_child(connection: &RustConnection, mut window: u32) -> Result<u32> {
    loop {
        let pointer = connection.query_pointer(window)?.reply()?;
        if pointer.child == x11rb::NONE {
            return Ok(window);
        }
        window = pointer.child;
    }
}

impl Drop for EmbeddedTerminal {
    fn drop(&mut self) {
        lock_unpoisoned(&self.processes).shutdown();
        let _ = self.process.kill();
        let _ = self.process.wait();
        let _ = self.connection.destroy_window(self.host);
        let _ = self.connection.flush();
    }
}
