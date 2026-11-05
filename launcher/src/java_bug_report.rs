//! Fabric 26.3 mods (beta): the bug report the Java mods tab copies ("Copy bug report") and
//! `horizon-launcher java-mods report` prints. One paste-ready text: the launcher and the
//! instance's Java mods build, the system, the Java the mods run on, the jars, the game log's
//! tail and its errors, and the last run's crash report and JVM crash log. Nothing is uploaded;
//! secrets are taken out first ([`Redact`]): account tokens, emails and UUIDs, and the home
//! folder and user name (as `~`).
use crate::{backend, build_import, instances, java, java_mods, java_release, java_variant};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// Where testers post the report.
pub const ISSUES: &str = "https://github.com/iaxivers/horizon-java-bridge/issues";
const TAIL: usize = 300;
const MAX_LINES: usize = 1500;
const HEAD: usize = 150;

/// What the report says about one instance, gathered by [`for_instance`] (tests fill it in).
pub struct Inputs {
    pub instance: String,
    /// The game's folder: `logs/`, `crash-reports/`, `mods-java/`, `hs_err_pid*.log`.
    pub home: PathBuf,
    pub java_mods_on: bool,
    /// The version played, and what its Java mods build is.
    pub version: Option<String>,
    pub build: String,
    pub java_home: Option<PathBuf>,
}

/// The selected (or named) instance's report, redacted.
pub fn for_instance(i: &instances::Instance) -> String {
    let version = i.resolve_version(None).ok();
    let build = version.as_deref().map_or("no version".into(), build_of);
    let inputs = Inputs {
        instance: i.name.clone(),
        home: i.home.clone(),
        java_mods_on: i.java_mods,
        version,
        build,
        java_home: java_variant::java_home(),
    };
    report(&inputs, &Redact::here())
}

/// Installed version `tag`'s Java mods build: the fork release's commit, imported, or missing.
fn build_of(tag: &str) -> String {
    if build_import::is_imported(tag) {
        return "imported build (a game of your own)".into();
    }
    if !java_variant::is_installed(tag) {
        return "not installed".into();
    }
    let marker = fs::read_to_string(java_variant::dir(tag).join(".installed")).unwrap_or_default();
    match java_release::marked_commit(&marker) {
        Some(commit) => format!("fork build, commit {commit}"),
        None if marker.trim().is_empty() => "release build or use-build (no commit marker)".into(),
        None => format!("marker {}", marker.trim()),
    }
}

/// The report's text for `inputs`, with `redact` applied to all of it.
pub fn report(inputs: &Inputs, redact: &Redact) -> String {
    let mut out = String::new();
    let mut put = |s: &str| {
        out.push_str(s);
        out.push('\n');
    };
    let log_path = inputs.home.join("logs").join("latest.log");
    let log = fs::read(&log_path)
        .map(|b| String::from_utf8_lossy(&b).into_owned())
        .unwrap_or_default();
    put("### Horizon Fabric mods bug report");
    put(&format!(
        "Post this at {ISSUES} (new issue), with what you did and what happened. Made by the \
         launcher; nothing was uploaded."
    ));
    put("");
    put("```");
    put(&format!("Launcher: {}", backend::VERSION));
    put(&format!(
        "Version: {} / Fabric mods build: {}",
        inputs.version.as_deref().unwrap_or("?"),
        inputs.build
    ));
    put(&format!("OS: {}", os()));
    let gpu = log
        .lines()
        .find_map(|l| l.split_once("Horizon renderer: ").map(|(_, g)| g.trim()));
    put(&format!(
        "GPU: {}",
        gpu.unwrap_or("? (no renderer line in the log)")
    ));
    put(&format!(
        "Instance: {} / Fabric mods: {}",
        inputs.instance,
        if inputs.java_mods_on { "on" } else { "off" }
    ));
    let java = match &inputs.java_home {
        Some(home) => {
            let v = java::release_version(home).map_or("? version".into(), |(v, _)| v);
            format!("{} ({v})", home.display())
        }
        None => "none found".into(),
    };
    put(&format!("Java home: {java}"));
    let mods = java_mods::list(&inputs.home);
    put(&format!("mods-java ({} jars):", mods.len()));
    for m in &mods {
        put(&format!("  {} ({} KB)", m.file, m.size.div_ceil(1024)));
    }
    put("```");
    if log.is_empty() {
        put(&format!("\nNo game log at {}.", log_path.display()));
    } else {
        let lines: Vec<&str> = log.lines().collect();
        let tail_from = lines.len().saturating_sub(TAIL);
        let mut seen = std::collections::HashSet::new();
        let important: Vec<&str> = lines[..tail_from]
            .iter()
            .copied()
            .filter(|l| important(l) && seen.insert(*l))
            .take(MAX_LINES - TAIL)
            .collect();
        if !important.is_empty() {
            put(&format!(
                "\n<details><summary>Errors in latest.log ({} lines)</summary>\n\n```",
                important.len()
            ));
            important.iter().for_each(|l| put(l));
            put("```\n</details>");
        }
        put(&format!(
            "\n<details><summary>latest.log, last {} lines</summary>\n\n```",
            lines.len() - tail_from
        ));
        lines[tail_from..].iter().for_each(|l| put(l));
        put("```\n</details>");
    }
    // The last run: from the log's start (its file's birth), else the hour before it ended.
    let since = fs::metadata(&log_path).ok().and_then(|m| {
        m.created()
            .ok()
            .or_else(|| m.modified().ok()?.checked_sub(Duration::from_secs(3600)))
    });
    let crash = newest(&inputs.home.join("crash-reports"), "crash-", ".txt", since);
    let hs_err = newest(&inputs.home, "hs_err_pid", ".log", since);
    for (what, file) in [("Crash report", crash), ("JVM crash log", hs_err)] {
        let Some(file) = file else { continue };
        let text = fs::read(&file).map(|b| String::from_utf8_lossy(&b).into_owned());
        let name = file.file_name().unwrap_or_default().to_string_lossy();
        put(&format!(
            "\n<details><summary>{what}: {name} (first {HEAD} lines)</summary>\n\n```"
        ));
        text.unwrap_or_default()
            .lines()
            .take(HEAD)
            .for_each(&mut put);
        put("```\n</details>");
    }
    redact.apply(&out)
}

/// A line the report keeps from the whole log.
fn important(line: &str) -> bool {
    line.contains("Java mods")
        || (line.contains("[java]") && (line.contains("ERROR") || line.contains("WARN")))
        || line.contains("Exception")
        || line.contains("Caused by")
}

/// The newest `<prefix>*<suffix>` in `dir` changed at or after `since` (any, without one).
fn newest(dir: &Path, prefix: &str, suffix: &str, since: Option<SystemTime>) -> Option<PathBuf> {
    fs::read_dir(dir)
        .ok()?
        .flatten()
        .filter(|e| {
            let n = e.file_name();
            let n = n.to_string_lossy();
            n.starts_with(prefix) && n.ends_with(suffix)
        })
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
        .filter(|(t, _)| since.is_none_or(|s| *t >= s))
        .max()
        .map(|(_, p)| p)
}

fn os() -> String {
    let (os, arch) = (std::env::consts::OS, std::env::consts::ARCH);
    let release = fs::read_to_string("/etc/os-release").unwrap_or_default();
    let name = release
        .lines()
        .find_map(|l| l.strip_prefix("PRETTY_NAME="))
        .map(|n| n.trim().trim_matches('"').to_string());
    let kernel = fs::read_to_string("/proc/sys/kernel/osrelease").unwrap_or_default();
    match name {
        Some(name) => format!("{name} ({os} {arch}, kernel {})", kernel.trim()),
        None => format!("{os} {arch}"),
    }
}

/// What the report must not carry: the accounts' values (tokens, emails, UUIDs), any email,
/// UUID or token-shaped word, the home folder and the user name (both as `~`).
pub struct Redact {
    home: Option<String>,
    user: Option<String>,
    secrets: Vec<String>,
}

impl Redact {
    /// This computer's home and user, and the launcher's `accounts.json`.
    pub fn here() -> Self {
        let user = std::env::var("USER")
            .or_else(|_| std::env::var("USERNAME"))
            .ok();
        let accounts = backend::data_dir().join("accounts.json");
        Self::new(dirs::home_dir().as_deref(), user.as_deref(), &[accounts])
    }

    /// `files`: JSON files whose every string value (6 characters or more) is a secret.
    pub fn new(home: Option<&Path>, user: Option<&str>, files: &[PathBuf]) -> Self {
        let mut secrets = Vec::new();
        for file in files {
            if let Some(v) = fs::read_to_string(file)
                .ok()
                .and_then(|t| serde_json::from_str(&t).ok())
            {
                strings(&v, &mut secrets);
            }
        }
        // The longest first: a value holding another goes whole.
        secrets.sort_by_key(|s| std::cmp::Reverse(s.len()));
        Redact {
            home: home.map(|h| h.to_string_lossy().trim_end_matches('/').to_string()),
            user: user.filter(|u| u.len() >= 3).map(str::to_string),
            secrets,
        }
    }

    pub fn apply(&self, text: &str) -> String {
        let mut out = text.to_string();
        for s in &self.secrets {
            out = out.replace(s.as_str(), "<redacted>");
        }
        if let Some(home) = self.home.as_deref().filter(|h| h.len() > 1) {
            out = out.replace(home, "~");
        }
        out = words(&out, |w| {
            let hex = |s: &str| s.chars().all(|c| c.is_ascii_hexdigit());
            let dashed = w.len() == 36
                && w.split('-').map(str::len).eq([8, 4, 4, 4, 12])
                && hex(&w.replace('-', ""));
            if dashed || (w.len() == 32 && hex(w)) {
                Some("<uuid>".into())
            } else if w
                .split_once('@')
                .is_some_and(|(a, b)| !a.is_empty() && b.contains('.') && !b.starts_with('.'))
            {
                Some("<email>".into())
            } else if (w.starts_with("eyJ") && w.len() >= 20) || w.len() >= 100 {
                Some("<token>".into())
            } else if self.user.as_deref() == Some(w) {
                Some("~".into())
            } else {
                None
            }
        });
        out
    }
}

/// `text` with each word (letters, digits and `-_.@+=`) that `f` replaces replaced.
fn words(text: &str, f: impl Fn(&str) -> Option<String>) -> String {
    let word = |c: char| c.is_alphanumeric() || "-_.@+=".contains(c);
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find(word) {
        out.push_str(&rest[..start]);
        rest = &rest[start..];
        let end = rest.find(|c: char| !word(c)).unwrap_or(rest.len());
        let w = &rest[..end];
        // A word's trailing dots end a sentence, not the word.
        let core = w.trim_end_matches('.');
        match f(core) {
            Some(r) => {
                out.push_str(&r);
                out.push_str(&w[core.len()..]);
            }
            None => out.push_str(w),
        }
        rest = &rest[end..];
    }
    out.push_str(rest);
    out
}

fn strings(v: &serde_json::Value, out: &mut Vec<String>) {
    match v {
        serde_json::Value::String(s) if s.len() >= 6 => out.push(s.clone()),
        serde_json::Value::Array(a) => a.iter().for_each(|v| strings(v, out)),
        // Skins and capes chosen (`looks`) are no secret, and hold common words.
        serde_json::Value::Object(o) => o
            .iter()
            .filter(|(k, _)| *k != "looks")
            .for_each(|(_, v)| strings(v, out)),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_report_holds_the_last_run_and_no_secrets() {
        let root = std::env::temp_dir().join(format!("hz-bugreport-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let home = root.join("home/tester/inst");
        for d in ["logs", "crash-reports", "mods-java"] {
            fs::create_dir_all(home.join(d)).unwrap();
        }
        let token = "eyJhbGciOiJIUzI1NiJ9.c2VjcmV0LXRva2Vu.c2ln";
        let refresh = "M.C512_BAY.0.U.-refreshsecret!abc";
        let accounts = root.join("accounts.json");
        fs::write(
            &accounts,
            format!(
                r#"{{"accounts":[{{"uuid":"0123456789abcdef0123456789abcdef","name":"Tester",
                "email":"tester@example.com","refresh":"{refresh}"}}]}}"#
            ),
        )
        .unwrap();
        let mut log = String::from(
            "[00:00:00] [main/INFO]: Horizon renderer: AMD RX 9070 XT (Vulkan, DiscreteGpu)\n",
        );
        log.push_str("[00:00:01] [main/WARN]: Java mods: not started (no Java)\n");
        log.push_str("[00:00:01] [main/WARN]: Java mods: not started (no Java)\n");
        log.push_str(&format!(
            "[00:00:02] [main/INFO]: session {token} for tester@example.com refresh {refresh}\n"
        ));
        log.push_str("[00:00:02] [main/INFO]: uuid 01234567-89ab-cdef-0123-456789abcdef\n");
        for n in 0..400 {
            log.push_str(&format!(
                "[00:01:00] [main/INFO]: line {n} in {}, user tester\n",
                home.display()
            ));
        }
        log.push_str("java.lang.IllegalStateException: boom\n");
        fs::write(home.join("logs/latest.log"), &log).unwrap();
        fs::write(
            home.join("crash-reports/crash-2026-11-06_10.00.00-client.txt"),
            "Description: boom\n",
        )
        .unwrap();
        fs::write(
            home.join("hs_err_pid42.log"),
            "# A fatal error has been detected\n",
        )
        .unwrap();
        let inputs = Inputs {
            instance: "Test".into(),
            home: home.clone(),
            java_mods_on: true,
            version: Some("v0.1".into()),
            build: "fork build, commit abc".into(),
            java_home: None,
        };
        let redact = Redact::new(Some(&root.join("home/tester")), Some("tester"), &[accounts]);
        let text = report(&inputs, &redact);
        let _ = fs::remove_dir_all(&root);

        assert!(text.contains(ISSUES));
        assert!(text.contains("GPU: AMD RX 9070 XT (Vulkan, DiscreteGpu)"));
        assert!(text.contains("Fabric mods: on"));
        // The early warning kept once from the whole log; the tail is the last 300 lines.
        assert_eq!(text.matches("Java mods: not started").count(), 1);
        assert!(text.contains("line 399 in ~/inst, user ~"));
        assert!(!text.contains("line 99 in"));
        assert!(text.contains("IllegalStateException: boom"));
        assert!(text.contains("Description: boom") && text.contains("A fatal error"));
        for secret in [
            token,
            refresh,
            "tester@example.com",
            "0123456789abcdef",
            "tester",
            "/home",
        ] {
            assert!(!text.contains(secret), "{secret} leaked:\n{text}");
        }
    }

    #[test]
    fn redaction_keeps_ordinary_words() {
        let r = Redact::new(Some(Path::new("/home/ann")), Some("ann"), &[]);
        assert_eq!(
            r.apply(
                "ann at /home/ann/x, canned ann. mail a@b.io id 0123456789abcdef0123456789abcdef"
            ),
            "~ at ~/x, canned ~. mail <email> id <uuid>"
        );
    }
}
