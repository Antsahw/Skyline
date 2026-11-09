//! Compiles the bridge's classes for runtimes in processes of their own (the transport,
//! `java/dev/horizonlab/bridge/remote`, and the 1.20.1 boot, `.../r120`: JDK types only) into
//! a jar the crate carries ([`BRIDGE120_JAR`] in lib.rs), as horizon-jvm carries its boot jar.
//! Without a JDK the jar is empty and the 1.20.1 runtime says so when asked to start.

use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    let root =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../horizon-jvm/java/dev/horizonlab/bridge");
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let jar = out.join("bridge120.jar");
    let mut sources = Vec::new();
    for dir in ["remote", "r120"] {
        let dir = root.join(dir);
        println!("cargo:rerun-if-changed={}", dir.display());
        for entry in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "java") {
                sources.push(path);
            }
        }
    }
    println!("cargo:rerun-if-env-changed=JAVA_HOME");
    let _ = std::fs::write(&jar, b"");
    let javac = std::env::var_os("JAVA_HOME")
        .map(|h| PathBuf::from(h).join("bin/javac"))
        .filter(|p| p.exists())
        .unwrap_or_else(|| PathBuf::from("javac"));
    let classes = out.join("bridge120-classes");
    let _ = std::fs::remove_dir_all(&classes);
    let compiled = Command::new(&javac)
        .args([
            "--release",
            "17",
            "-proc:none",
            "-nowarn",
            "-encoding",
            "UTF-8",
            "-d",
        ])
        .arg(&classes)
        .args(&sources)
        .status()
        .is_ok_and(|s| s.success());
    if !compiled {
        println!("cargo:warning=no JDK (javac): the 1.20.1 runtime's classes are not built in");
        return;
    }
    let jar_tool = javac.with_file_name("jar");
    let packed = Command::new(&jar_tool)
        .arg("cf")
        .arg(&jar)
        .arg("-C")
        .arg(&classes)
        .arg(".")
        .status()
        .is_ok_and(|s| s.success());
    if !packed {
        let _ = std::fs::write(&jar, b"");
        println!("cargo:warning=jar failed: the 1.20.1 runtime's classes are not built in");
    }
}
