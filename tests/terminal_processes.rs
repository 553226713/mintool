use mintool::services::{terminal::TerminalSession, terminal_processes::TerminalProcesses};
use std::{
    fs,
    net::{TcpListener, TcpStream},
    path::Path,
    process::{Child, Command},
    thread,
    time::{Duration, Instant},
};

struct ControlProcess(Child);
impl Drop for ControlProcess {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn wait_port(path: &Path) -> u16 {
    let start = Instant::now();
    loop {
        if let Ok(text) = fs::read_to_string(path) {
            if let Ok(port) = text.parse() {
                return port;
            }
        }
        assert!(
            start.elapsed() < Duration::from_secs(8),
            "service did not start: {}",
            path.display()
        );
        thread::sleep(Duration::from_millis(20));
    }
}
fn assert_port_released(port: u16) {
    let start = Instant::now();
    loop {
        if TcpListener::bind(("127.0.0.1", port)).is_ok() {
            return;
        }
        assert!(
            start.elapsed() < Duration::from_secs(4),
            "port {port} is still occupied"
        );
        thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn closing_one_terminal_and_exiting_the_app_release_only_their_owned_service_ports() {
    let root = tempfile::tempdir().unwrap();
    let server = root.path().join("server.py");
    fs::write(&server, "import signal,socket,sys,time\nfrom pathlib import Path\nsignal.signal(signal.SIGTERM,signal.SIG_IGN)\nsignal.signal(signal.SIGHUP,signal.SIG_IGN)\ns=socket.socket();s.bind(('127.0.0.1',0));s.listen()\nPath(sys.argv[1]).write_text(str(s.getsockname()[1]))\nwhile True: time.sleep(1)\n").unwrap();
    // This listener does not belong to any terminal and must survive both cleanups.
    let control_port = root.path().join("control-port");
    let control = Command::new("python3")
        .arg(&server)
        .arg(&control_port)
        .spawn()
        .unwrap();
    let mut control = ControlProcess(control);
    let control_port = wait_port(&control_port);
    let mut first = TerminalSession::spawn(root.path(), "/bin/bash").unwrap();
    let mut second = TerminalSession::spawn(root.path(), "/bin/bash").unwrap();
    let first_port = root.path().join("first-port");
    let second_port = root.path().join("second-port");
    // Include a separate session and a background shell job; both ignore graceful signals.
    first
        .send(
            format!(
                "setsid python3 '{}' '{}' >/dev/null 2>&1 &\r",
                server.display(),
                first_port.display()
            )
            .as_bytes(),
        )
        .unwrap();
    second
        .send(
            format!(
                "python3 '{}' '{}' >/dev/null 2>&1 &\r",
                server.display(),
                second_port.display()
            )
            .as_bytes(),
        )
        .unwrap();
    let first_port = wait_port(&first_port);
    let second_port = wait_port(&second_port);
    first.shutdown();
    assert_port_released(first_port);
    assert!(TcpStream::connect(("127.0.0.1", second_port)).is_ok());
    assert!(TcpStream::connect(("127.0.0.1", control_port)).is_ok());
    assert!(control.0.try_wait().unwrap().is_none());
    TerminalProcesses::shutdown_all();
    assert_port_released(second_port);
    assert!(TcpStream::connect(("127.0.0.1", control_port)).is_ok());
    assert!(control.0.try_wait().unwrap().is_none());
}
