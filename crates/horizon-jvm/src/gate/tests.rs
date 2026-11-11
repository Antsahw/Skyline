use super::*;
use std::io::{BufRead, BufReader, Write};

const UUID: &str = "0123456789abcdef0123456789abcdef";

/// A scratch launcher folder: `accounts.json` listing `accounts`, and `java-mods-owner/`.
struct Launcher(PathBuf);

impl Launcher {
    fn new(name: &str, accounts: &[&str]) -> Self {
        let dir = std::env::temp_dir().join(format!("hz-gate-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("java-mods-owner")).unwrap();
        let list: Vec<_> = accounts
            .iter()
            .map(|u| serde_json::json!({"uuid": u, "name": "Steve"}))
            .collect();
        std::fs::write(
            dir.join("accounts.json"),
            serde_json::json!({"accounts": list, "selected": accounts.first()}).to_string(),
        )
        .unwrap();
        Launcher(dir)
    }

    /// A record as the launcher writes it (`account::write_session_file`): private, fresh.
    fn record(&self, uuid: &str) -> PathBuf {
        let path = self.0.join("java-mods-owner/session-1.json");
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
        let mut file = options.open(&path).unwrap();
        let body = serde_json::json!({"name": "Steve", "uuid": uuid, "access_token": "tok",
            "user_type": "msa", "xuid": null});
        file.write_all(body.to_string().as_bytes()).unwrap();
        path
    }
}

impl Drop for Launcher {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// [`decide`] with `vars`; `services` answers, and the calls it got are counted.
fn run(args: &[&str], vars: &[(&str, &str)], services: Services) -> (OptIn, bool) {
    let mut asked = false;
    let got = decide(
        args.iter().copied(),
        |k| {
            vars.iter()
                .find(|(n, _)| *n == k)
                .map(|(_, v)| v.to_string())
        },
        |record| {
            asked = true;
            assert_eq!(record.uuid, UUID);
            assert_eq!(record.access_token, "tok");
            services
        },
    );
    (got, asked)
}

#[test]
fn variables_set_by_hand_run_no_mods() {
    let refused = |vars: &[(&str, &str)]| run(&["horizon-client", ARG], vars, Services::Owns);
    // The game started directly with the switch's argument: no launcher, no record.
    assert_eq!(refused(&[]), (OptIn::NoOwner(Refused::NoRecord), false));
    assert_eq!(
        refused(&[(OWNER_VAR, "")]),
        (OptIn::NoOwner(Refused::NoRecord), false)
    );
    // The old launcher word, or any path that is no record: refused before any services call.
    assert_eq!(
        refused(&[(OWNER_VAR, "1")]),
        (OptIn::NoOwner(Refused::BadRecord), false)
    );
    let dir = Launcher::new("dir", &[UUID]);
    let folder = dir.0.to_string_lossy().into_owned();
    assert_eq!(
        refused(&[(OWNER_VAR, &folder)]),
        (OptIn::NoOwner(Refused::BadRecord), false)
    );
    // In a release build the developers' variable does nothing.
    #[cfg(not(feature = "dev"))]
    assert_eq!(
        refused(&[("HORIZON_JAVA_MODS", "1")]),
        (OptIn::NoOwner(Refused::NoRecord), false)
    );
    #[cfg(not(feature = "dev"))]
    assert_eq!(
        run(
            &["--world", "W"],
            &[("HORIZON_JAVA_MODS", "1")],
            Services::Owns
        )
        .0,
        OptIn::Off
    );
    assert!(!OptIn::NoOwner(Refused::NoRecord).runs());
}

#[test]
fn a_record_must_be_fresh_private_and_the_launchers() {
    let launcher = Launcher::new("record", &[UUID]);
    let check = |path: &Path| {
        let p = path.to_string_lossy().into_owned();
        run(&[ARG], &[(OWNER_VAR, &p)], Services::Owns)
    };
    // Too old: the launcher writes it as it starts the game.
    let old = launcher.record(UUID);
    std::fs::File::options()
        .write(true)
        .open(&old)
        .unwrap()
        .set_modified(SystemTime::now() - RECORD_LIFE - Duration::from_secs(5))
        .unwrap();
    assert_eq!(check(&old), (OptIn::NoOwner(Refused::BadRecord), false));
    assert!(!old.exists(), "a refused record is deleted too");
    // Written in the future: no proof of freshness.
    let future = launcher.record(UUID);
    std::fs::File::options()
        .write(true)
        .open(&future)
        .unwrap()
        .set_modified(SystemTime::now() + Duration::from_secs(3600))
        .unwrap();
    assert_eq!(check(&future), (OptIn::NoOwner(Refused::BadRecord), false));
    // Readable by others.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let shared = launcher.record(UUID);
        std::fs::set_permissions(&shared, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert_eq!(check(&shared), (OptIn::NoOwner(Refused::BadRecord), false));
    }
    // An account the launcher doesn't have.
    let stranger = launcher.record("ffffffffffffffffffffffffffffffff");
    assert_eq!(
        check(&stranger),
        (OptIn::NoOwner(Refused::NotInLauncher), false)
    );
    // Fine: the services are asked, and the record is read once.
    let good = launcher.record(UUID);
    assert_eq!(check(&good), (OptIn::Launcher, true));
    assert!(!good.exists(), "the game deletes the record it read");
    assert_eq!(check(&good), (OptIn::NoOwner(Refused::BadRecord), false));
}

#[test]
fn only_the_services_say_who_owns_the_game() {
    let launcher = Launcher::new("services", &[UUID]);
    let ask = |answer: Services| {
        let p = launcher.record(UUID).to_string_lossy().into_owned();
        run(&["--resources", "r", ARG], &[(OWNER_VAR, &p)], answer).0
    };
    assert_eq!(ask(Services::Owns), OptIn::Launcher);
    assert!(OptIn::Launcher.runs());
    assert_eq!(ask(Services::DoesNot), OptIn::NoOwner(Refused::NotOwner));
    // Offline: no mods (the launcher's gate only passes offline on an earlier answer).
    assert_eq!(
        ask(Services::NoAnswer("no route".into())),
        OptIn::NoOwner(Refused::NoAnswer)
    );
    // A record without the argument (the switch off) runs nothing and isn't read.
    let record = launcher.record(UUID);
    let p = record.to_string_lossy().into_owned();
    assert_eq!(
        run(&["--resources", "r"], &[(OWNER_VAR, &p)], Services::Owns),
        (OptIn::Off, false)
    );
    assert!(!OptIn::Off.runs());
}

#[cfg(feature = "dev")]
#[test]
fn developers_opt_in_explicitly() {
    assert_eq!(
        run(&["--world", "W"], &[(DEV_VAR, "1")], Services::DoesNot).0,
        OptIn::Developer
    );
    assert!(OptIn::Developer.runs());
    assert_eq!(
        run(&[ARG], &[(DEV_VAR, "0")], Services::Owns).0,
        OptIn::NoOwner(Refused::NoRecord)
    );
}

/// A stand-in for the Minecraft services on localhost: `answers` by path (status, body).
fn services(answers: Vec<(&'static str, u16, String)>) -> String {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            let path = line.split_whitespace().nth(1).unwrap_or("").to_string();
            let mut auth = String::new();
            loop {
                let mut header = String::new();
                if reader.read_line(&mut header).unwrap_or(0) == 0 || header == "\r\n" {
                    break;
                }
                if header.to_ascii_lowercase().starts_with("authorization:") {
                    auth = header.trim().to_string();
                }
            }
            let (status, body) = answers
                .iter()
                .find(|(p, _, _)| *p == path)
                .filter(|_| auth.ends_with("Bearer tok"))
                .map(|(_, s, b)| (*s, b.clone()))
                .unwrap_or((401, "{}".into()));
            let mut out = stream;
            let _ = write!(
                out,
                "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
        }
    });
    base
}

/// [`services_check`] at `base`, with no proxy between (the stand-in is on localhost).
fn check_at(base: &str, record: &Record) -> Services {
    services_check_with(&ureq::AgentBuilder::new().build(), base, record)
}

#[test]
fn the_services_check_reads_profile_and_entitlements() {
    let record = Record {
        uuid: UUID.into(),
        access_token: "tok".into(),
    };
    let profile = format!(r#"{{"id":"{UUID}","name":"Steve"}}"#);
    let owns = r#"{"items":[{"name":"product_minecraft"},{"name":"game_minecraft"}]}"#.to_string();
    let base = services(vec![
        ("/minecraft/profile", 200, profile.clone()),
        ("/entitlements/mcstore", 200, owns.clone()),
    ]);
    assert_eq!(check_at(&base, &record), Services::Owns);
    // Game Pass or Bedrock only: no Java Edition.
    let bedrock = services(vec![
        ("/minecraft/profile", 200, profile.clone()),
        (
            "/entitlements/mcstore",
            200,
            r#"{"items":[{"name":"product_minecraft_bedrock"}]}"#.into(),
        ),
    ]);
    assert_eq!(check_at(&bedrock, &record), Services::DoesNot);
    // The token is another profile's.
    let other = services(vec![
        (
            "/minecraft/profile",
            200,
            r#"{"id":"ffffffffffffffffffffffffffffffff"}"#.into(),
        ),
        ("/entitlements/mcstore", 200, owns.clone()),
    ]);
    assert_eq!(check_at(&other, &record), Services::DoesNot);
    // A token the services refuse (made up by hand).
    let forged = Record {
        access_token: "made-up".into(),
        ..record.clone()
    };
    assert_eq!(check_at(&base, &forged), Services::DoesNot);
    // A server error, or nobody there: no answer.
    let down = services(vec![("/minecraft/profile", 503, "{}".into())]);
    assert!(matches!(check_at(&down, &record), Services::NoAnswer(_)));
    // Too many requests is no answer either, not a "no".
    let busy = services(vec![
        ("/minecraft/profile", 200, profile.clone()),
        ("/entitlements/mcstore", 429, "{}".into()),
    ]);
    assert!(matches!(check_at(&busy, &record), Services::NoAnswer(_)));
    let closed = {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        format!("http://{}", l.local_addr().unwrap())
    };
    assert!(matches!(check_at(&closed, &record), Services::NoAnswer(_)));
}
