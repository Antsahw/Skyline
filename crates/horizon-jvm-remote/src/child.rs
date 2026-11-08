//! A Java runtime in a process of its own: Horizon listens on a socket only it and the child
//! can reach, starts `java ... dev.horizonlab.bridge.remote.Child`, and accepts one connection
//! that proves itself with a token passed in the child's environment (never on its command
//! line, which other users can read). Then the two talk through a [`Peer`].
//!
//! Unix sockets where the platform has them in Rust's std (a file in a private folder);
//! loopback TCP elsewhere (Windows), with [`Config::tcp`] or when `HORIZON_REMOTE_TCP=1`. Either way the child
//! must send the token in its hello before anything else is read.
//!
//! The runtime's arguments travel in the welcome frame, in UTF-8.
//!
//! The child's stdout and stderr (log4j, mods' prints, crashes) become log lines prefixed
//! with the runtime's name; the last lines are kept for the error when it dies early.

use crate::peer::{Error, Handler, Peer};
use crate::wire::Value;
use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// The protocol both sides speak (`Wire.java`'s `PROTOCOL`).
pub const PROTOCOL: i32 = 1;

/// The child's entry point in the bridge jar.
pub const CHILD_MAIN: &str = "dev.horizonlab.bridge.remote.Child";

/// Where log lines go.
pub type Log = Arc<dyn Fn(&str) + Send + Sync>;

/// What [`spawn`] starts.
pub struct Config {
    /// A short name for log lines and files ("fabric-1.20.1").
    pub name: String,
    /// The `java` executable.
    pub java: PathBuf,
    /// Options before the class path (`-Xmx`, `-D...`).
    pub jvm_args: Vec<String>,
    pub class_path: Vec<PathBuf>,
    /// The class the child runs once connected (`dev.horizonlab.bridge.r120.Boot120`).
    pub runtime: String,
    /// Its arguments.
    pub runtime_args: Vec<String>,
    /// The child's working folder, and where the socket file goes (`<dir>/remote/`).
    pub dir: PathBuf,
    /// Loopback TCP instead of a Unix socket (always so where std has no Unix sockets;
    /// `HORIZON_REMOTE_TCP=1` forces it too).
    pub tcp: bool,
    /// How long the child may take to connect (a cold JVM: about a second).
    pub connect_timeout: Duration,
    pub log: Log,
}

/// What the child said in its hello.
#[derive(Debug, Clone)]
pub struct Hello {
    pub runtime: String,
    pub java_version: String,
    pub pid: i64,
}

/// A running child.
pub struct Child {
    pub peer: Peer,
    pub hello: Hello,
    process: std::process::Child,
    tail: Arc<Mutex<VecDeque<String>>>,
}

const TAIL: usize = 40;

/// A token nobody else can guess: the OS's random bytes where there are some.
fn token() -> String {
    let mut bytes = [0u8; 16];
    let read = std::fs::File::open("/dev/urandom").and_then(|mut f| f.read_exact(&mut bytes));
    if read.is_err() {
        // No /dev/urandom (Windows): std's per-process random hash keys, mixed with the clock.
        use std::hash::{BuildHasher, Hasher};
        for chunk in bytes.chunks_mut(8) {
            let mut h = std::collections::hash_map::RandomState::new().build_hasher();
            h.write_u128(Instant::now().elapsed().as_nanos() ^ u128::from(std::process::id()));
            chunk.copy_from_slice(&h.finish().to_le_bytes()[..chunk.len()]);
        }
    }
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

enum Listener {
    #[cfg(unix)]
    Unix(std::os::unix::net::UnixListener, PathBuf),
    Tcp(std::net::TcpListener),
}

type Stream = (Box<dyn Read + Send>, Box<dyn std::io::Write + Send>);

impl Listener {
    fn bind(dir: &Path, name: &str, tcp: bool) -> Result<Listener, String> {
        let tcp = tcp || std::env::var("HORIZON_REMOTE_TCP").is_ok_and(|v| v == "1");
        #[cfg(unix)]
        if !tcp {
            use std::os::unix::fs::PermissionsExt;
            let folder = dir.join("remote");
            std::fs::create_dir_all(&folder).map_err(|e| format!("{}: {e}", folder.display()))?;
            // Only this user may connect: the folder is private.
            let _ = std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o700));
            let path = folder.join(format!("{name}-{}.sock", std::process::id()));
            let _ = std::fs::remove_file(&path);
            // Socket paths are limited to ~100 bytes; a deep game folder falls back to TCP.
            if path.as_os_str().len() < 100 {
                let listener = std::os::unix::net::UnixListener::bind(&path)
                    .map_err(|e| format!("{}: {e}", path.display()))?;
                listener
                    .set_nonblocking(true)
                    .map_err(|e| format!("socket: {e}"))?;
                return Ok(Listener::Unix(listener, path));
            }
        }
        let _ = (dir, name, tcp);
        let listener =
            std::net::TcpListener::bind(("127.0.0.1", 0)).map_err(|e| format!("loopback: {e}"))?;
        listener
            .set_nonblocking(true)
            .map_err(|e| format!("socket: {e}"))?;
        Ok(Listener::Tcp(listener))
    }

    fn address(&self) -> String {
        match self {
            #[cfg(unix)]
            Listener::Unix(_, path) => format!("unix:{}", path.display()),
            Listener::Tcp(l) => format!(
                "tcp:{}",
                l.local_addr().map(|a| a.to_string()).unwrap_or_default()
            ),
        }
    }

    /// One connection, or `None` while nobody has connected yet.
    fn accept(&self) -> std::io::Result<Option<Stream>> {
        let result = match self {
            #[cfg(unix)]
            Listener::Unix(l, _) => l.accept().map(|(s, _)| {
                s.set_nonblocking(false)?;
                Ok::<Stream, std::io::Error>((Box::new(s.try_clone()?), Box::new(s)))
            }),
            Listener::Tcp(l) => l.accept().map(|(s, _)| {
                s.set_nonblocking(false)?;
                // Small frames go out at once: Nagle's delay would add ~40 ms to a call.
                s.set_nodelay(true)?;
                Ok::<Stream, std::io::Error>((Box::new(s.try_clone()?), Box::new(s)))
            }),
        };
        match result {
            Ok(stream) => stream.map(Some),
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => Ok(None),
            Err(e) => Err(e),
        }
    }
}

impl Drop for Listener {
    fn drop(&mut self) {
        #[cfg(unix)]
        if let Listener::Unix(_, path) = self {
            let _ = std::fs::remove_file(path);
        }
    }
}

/// Starts the child and waits for its hello.
pub fn spawn(config: &Config) -> Result<Child, String> {
    std::fs::create_dir_all(&config.dir).map_err(|e| format!("{}: {e}", config.dir.display()))?;
    let listener = Listener::bind(&config.dir, &config.name, config.tcp)?;
    let token = token();
    let class_path =
        std::env::join_paths(&config.class_path).map_err(|e| format!("class path: {e}"))?;
    let mut command = Command::new(&config.java);
    command
        .args(&config.jvm_args)
        .arg("-cp")
        .arg(&class_path)
        .arg(CHILD_MAIN)
        .arg("--connect")
        .arg(listener.address())
        .arg("--runtime")
        .arg(&config.runtime)
        .current_dir(&config.dir)
        .env("HORIZON_REMOTE_TOKEN", &token)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut process = command
        .spawn()
        .map_err(|e| format!("{}: {e}", config.java.display()))?;
    let tail = Arc::new(Mutex::new(VecDeque::with_capacity(TAIL)));
    for stream in [
        process
            .stdout
            .take()
            .map(|s| Box::new(s) as Box<dyn Read + Send>),
        process
            .stderr
            .take()
            .map(|s| Box::new(s) as Box<dyn Read + Send>),
    ]
    .into_iter()
    .flatten()
    {
        let log = config.log.clone();
        let tail = tail.clone();
        let prefix = format!("[{}] ", config.name);
        let _ = std::thread::Builder::new()
            .name(format!("{} output", config.name))
            .spawn(move || {
                for line in BufReader::new(stream).lines() {
                    let Ok(line) = line else { break };
                    log(&format!("{prefix}{line}"));
                    let mut tail = tail.lock().unwrap_or_else(|e| e.into_inner());
                    if tail.len() == TAIL {
                        tail.pop_front();
                    }
                    tail.push_back(line);
                }
            });
    }
    let fail = |process: &mut std::process::Child, why: String| -> String {
        let _ = process.kill();
        let status = process.wait().ok();
        // The output threads may still be reading the last lines.
        std::thread::sleep(Duration::from_millis(100));
        let lines: Vec<String> = tail
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .cloned()
            .collect();
        let status = status.map(|s| format!(" ({s})")).unwrap_or_default();
        format!("{why}{status}; its last lines:\n{}", lines.join("\n"))
    };
    let started = Instant::now();
    let (reader, writer) = loop {
        match listener.accept() {
            Ok(Some(stream)) => break stream,
            Ok(None) => {}
            Err(e) => return Err(fail(&mut process, format!("accept: {e}"))),
        }
        if let Ok(Some(_)) = process.try_wait() {
            return Err(fail(
                &mut process,
                format!("{} exited before connecting", config.name),
            ));
        }
        if started.elapsed() > config.connect_timeout {
            return Err(fail(
                &mut process,
                format!("{} didn't connect", config.name),
            ));
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    drop(listener);
    let mut peer = Peer::new(reader, writer);
    let hello = match peer.read_hello() {
        Ok(values) => values,
        Err(e) => return Err(fail(&mut process, format!("hello: {e}"))),
    };
    let text = |i: usize| {
        hello
            .get(i)
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string()
    };
    if hello.first().and_then(Value::as_i32) != Some(PROTOCOL) {
        return Err(fail(
            &mut process,
            format!("protocol {:?}, expected {PROTOCOL}", hello.first()),
        ));
    }
    if text(1) != token {
        return Err(fail(&mut process, "a connection without the token".into()));
    }
    let hello = Hello {
        runtime: text(2),
        java_version: text(3),
        pid: hello.get(4).and_then(Value::as_i64).unwrap_or(0),
    };
    // The runtime's arguments go in the welcome, not on the command line: Java decodes its
    // command line in the system's locale, which garbles paths like /home/Ivér under a C locale.
    let welcome = [
        Value::I32(PROTOCOL),
        Value::Strs(config.runtime_args.clone()),
    ];
    if let Err(e) = peer.welcome(&welcome) {
        return Err(fail(&mut process, format!("welcome: {e}")));
    }
    Ok(Child {
        peer,
        hello,
        process,
        tail,
    })
}

impl Child {
    /// A call into the child (see [`Peer::call`]).
    pub fn call(
        &mut self,
        method: &str,
        args: &[Value],
        handler: &mut dyn Handler,
    ) -> Result<Vec<Value>, Error> {
        self.peer.call(method, args, handler)
    }

    /// The child's last output lines (for an error message).
    pub fn last_lines(&self) -> Vec<String> {
        self.tail
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .cloned()
            .collect()
    }

    /// Whether the process still runs.
    pub fn running(&mut self) -> bool {
        matches!(self.process.try_wait(), Ok(None))
    }

    /// Says goodbye and waits up to `grace` for the process to end, then kills it.
    pub fn stop(&mut self, grace: Duration) -> Option<std::process::ExitStatus> {
        let _ = self.peer.bye();
        let until = Instant::now() + grace;
        while Instant::now() < until {
            if let Ok(Some(status)) = self.process.try_wait() {
                return Some(status);
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let _ = self.process.kill();
        self.process.wait().ok()
    }
}

impl Drop for Child {
    fn drop(&mut self) {
        if self.running() {
            self.stop(Duration::from_secs(2));
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn tokens_are_long_and_differ() {
        let (a, b) = (super::token(), super::token());
        assert_eq!(a.len(), 32);
        assert_ne!(a, b);
    }
}
