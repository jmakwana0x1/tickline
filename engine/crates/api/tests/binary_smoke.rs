//! Drives the real `tickline-engine` binary end to end (issue #23).
//!
//! The nightly mutation run found three mutants nothing could kill: `router` replaced with an
//! empty router, `main` replaced with `Ok(())`, and `shutdown` replaced with `()`. Each one
//! breaks a step of this test. The server binds loopback only, on a port the OS picks, so the
//! test needs no fixed port and never leaves the machine.

use std::{
    error::Error,
    io::{BufRead, BufReader, Read, Write},
    net::TcpStream,
    process::{Child, Command, Stdio},
    sync::mpsc::{self, Receiver},
    thread,
    time::{Duration, Instant},
};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

/// Generous: a cold CI runner can be slow to exec a debug binary.
const TIMEOUT: Duration = Duration::from_secs(20);

/// The log line the binary prints once its listener is bound.
const LISTENING: &str = "tickline engine listening";

/// Kills the engine if the test fails before shutting it down cleanly.
struct Engine(Child);

impl Drop for Engine {
    fn drop(&mut self) {
        if let Ok(None) = self.0.try_wait() {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}

#[test]
fn engine_binary_serves_health_until_interrupted() -> TestResult {
    let mut child = Command::new(env!("CARGO_BIN_EXE_tickline-engine"))
        .env("BIND_ADDR", "127.0.0.1:0")
        .env_remove("RUST_LOG")
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()?;
    let stdout = child
        .stdout
        .take()
        .ok_or("engine stdout was not captured")?;
    let mut engine = Engine(child);

    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if tx.send(line).is_err() {
                break;
            }
        }
    });

    // Kills `main -> Ok(())`: nothing is ever logged.
    let addr = listening_addr(&rx)?;

    // Kills `router -> Default::default()`: an empty router answers 404.
    let response = http_get(&addr, "/health")?;
    assert!(
        response.starts_with("HTTP/1.1 200"),
        "expected 200 from /health, got: {response}"
    );
    assert!(
        response.contains(r#""status":"ok""#),
        "expected an ok body from /health, got: {response}"
    );

    // Kills `shutdown -> ()`: the server would begin shutting down as soon as it started.
    assert!(
        engine.0.try_wait()?.is_none(),
        "engine exited on its own; it must run until told to stop"
    );

    let signalled = Command::new("kill")
        .args(["-INT", &engine.0.id().to_string()])
        .status()?;
    assert!(signalled.success(), "could not send SIGINT to the engine");

    let deadline = Instant::now()
        .checked_add(TIMEOUT)
        .ok_or("deadline overflow")?;
    loop {
        if let Some(status) = engine.0.try_wait()? {
            assert!(
                status.success(),
                "engine exited uncleanly after SIGINT: {status}"
            );
            return Ok(());
        }
        if Instant::now() > deadline {
            return Err("engine did not exit within the timeout after SIGINT".into());
        }
        thread::sleep(Duration::from_millis(50));
    }
}

/// Waits for the listening log line and returns the address it reports.
fn listening_addr(lines: &Receiver<String>) -> TestResult<String> {
    loop {
        let line = lines
            .recv_timeout(TIMEOUT)
            .map_err(|e| format!("no '{LISTENING}' log line from the engine: {e}"))?;
        let Ok(record) = serde_json::from_str::<serde_json::Value>(&line) else {
            continue;
        };
        let fields = record.get("fields").unwrap_or(&record);
        if fields.get("message").and_then(|m| m.as_str()) != Some(LISTENING) {
            continue;
        }
        let addr = fields
            .get("addr")
            .and_then(|a| a.as_str())
            .ok_or("listening log line carries no addr")?;
        return Ok(addr.to_owned());
    }
}

/// A minimal HTTP/1.1 GET over loopback. No client dependency needed for one request.
fn http_get(addr: &str, path: &str) -> TestResult<String> {
    let mut stream = TcpStream::connect(addr)?;
    stream.set_read_timeout(Some(TIMEOUT))?;
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n"
    )?;
    let mut response = String::new();
    stream.read_to_string(&mut response)?;
    Ok(response)
}
