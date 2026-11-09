//! Builds the child for the tests and the benchmark: the bridge's `remote` classes and the
//! `remotetest` fixture (`crates/horizon-jvm/java/fixtures/remotetest`), compiled once into the
//! target folder.

#![allow(dead_code, unused_imports)]

use horizon_jvm_remote::child::{self, Config};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

/// The Java home: `HORIZON_JAVA_HOME`, `JAVA_HOME`, or the `javac` on the PATH.
pub fn java_home() -> Option<PathBuf> {
    for var in ["HORIZON_JAVA_HOME", "JAVA_HOME"] {
        if let Some(home) = std::env::var_os(var).map(PathBuf::from)
            && home.join("bin/javac").exists()
        {
            return Some(home);
        }
    }
    let out = std::process::Command::new("sh")
        .args(["-c", "readlink -f \"$(command -v javac)\""])
        .output()
        .ok()?;
    let javac = PathBuf::from(String::from_utf8(out.stdout).ok()?.trim());
    Some(javac.parent()?.parent()?.to_path_buf()).filter(|h| h.join("bin/java").exists())
}

/// The compiled classes, or `None` (and a note) with no JDK.
pub fn classes() -> Option<PathBuf> {
    static DIR: OnceLock<Option<PathBuf>> = OnceLock::new();
    DIR.get_or_init(|| {
        let Some(home) = java_home() else {
            eprintln!("no JDK found: skipping the tests that start a Java child");
            return None;
        };
        let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
        let out = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("remote-test-classes");
        let _ = std::fs::remove_dir_all(&out);
        let remote = manifest.join("../horizon-jvm/java/dev/horizonlab/bridge/remote");
        let mut sources: Vec<PathBuf> = std::fs::read_dir(&remote)
            .expect("bridge remote sources")
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|e| e == "java"))
            .collect();
        sources.push(manifest.join(
            "../horizon-jvm/java/fixtures/remotetest/dev/horizonlab/remotetest/TestRuntime.java",
        ));
        let status = std::process::Command::new(home.join("bin/javac"))
            .args(["--release", "17", "-proc:none", "-d"])
            .arg(&out)
            .args(&sources)
            .status()
            .expect("javac");
        assert!(status.success(), "javac failed");
        Some(out)
    })
    .clone()
}

/// The log lines a child printed (shared with its output threads).
pub type Lines = Arc<Mutex<Vec<String>>>;

pub fn config(name: &str, args: &[&str]) -> Option<(Config, Lines)> {
    let classes = classes()?;
    let lines: Lines = Arc::new(Mutex::new(Vec::new()));
    let sink = lines.clone();
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("remote-{name}"));
    Some((
        Config {
            name: name.to_string(),
            java: java_home()?.join("bin/java"),
            jvm_args: vec!["-Xshare:auto".into(), "-XX:TieredStopAtLevel=1".into()],
            class_path: vec![classes],
            runtime: "dev.horizonlab.remotetest.TestRuntime".into(),
            runtime_args: args.iter().map(|s| s.to_string()).collect(),
            dir,
            tcp: false,
            connect_timeout: Duration::from_secs(30),
            read_timeout: Some(Duration::from_secs(60)),
            log: Arc::new(move |line: &str| sink.lock().unwrap().push(line.to_string())),
        },
        lines,
    ))
}

pub use child::spawn;
