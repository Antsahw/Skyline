//! A real Java child: handshake, nested calls both ways, events, failures and a crash, over a
//! Unix socket and over loopback TCP. Skips (passes) without a JDK.

mod support;

use horizon_jvm_remote::{Error, Handler, Peer, Value};
use std::time::Duration;

/// Horizon's side: answers the child's `blockState` and `down`, collects its events.
#[derive(Default)]
struct World {
    lines: Vec<i32>,
    logs: Vec<String>,
    reads: u32,
}

impl Handler for World {
    fn call(
        &mut self,
        peer: &mut Peer,
        method: &str,
        args: Vec<Value>,
    ) -> Result<Vec<Value>, String> {
        match method {
            "blockState" => {
                self.reads += 1;
                assert_eq!(args[0].as_str(), Some("minecraft:overworld"));
                assert_eq!(args[2].as_i32(), Some(64));
                Ok(vec![Value::Str("minecraft:stone".into())])
            }
            "down" => {
                let n = args[0].as_i32().unwrap();
                let got = peer
                    .call("deep", &[Value::I32(n - 1)], self)
                    .map_err(|e| e.to_string())?;
                Ok(vec![Value::Str(format!("v{}", got[0].as_str().unwrap()))])
            }
            _ => Err(format!("Horizon has no {method}")),
        }
    }

    fn event(&mut self, _: &mut Peer, method: &str, args: Vec<Value>) {
        match method {
            "line" => self.lines.push(args[0].as_i32().unwrap()),
            "log" => self.logs.push(args[0].as_str().unwrap().to_string()),
            _ => panic!("event {method}"),
        }
    }
}

fn conversation(tcp: bool) {
    let name = if tcp { "tcp" } else { "unix" };
    let Some((mut config, _lines)) = support::config(name, &["a b", "ü"]) else {
        return;
    };
    config.tcp = tcp;
    let mut child = support::spawn(&config).expect("child starts");
    assert!(
        child
            .hello
            .java_version
            .chars()
            .next()
            .unwrap()
            .is_ascii_digit()
    );
    assert!(child.hello.pid > 0);
    let mut world = World::default();

    // The runtime's arguments arrive unsplit and in UTF-8.
    let got = child.call("args", &[], &mut world).unwrap();
    assert_eq!(got, vec![Value::Strs(vec!["a b".into(), "ü".into()])]);
    // Its init's log line came as an event before the first answer.
    assert_eq!(world.logs, ["test runtime up with 2 args"]);

    let values = vec![
        Value::Null,
        Value::Bool(false),
        Value::I32(-3),
        Value::I64(-(1 << 50)),
        Value::F32(1.5),
        Value::F64(f64::MIN_POSITIVE),
        Value::Str("{id:\"minecraft:chest\"}".into()),
        Value::Bytes((0..=255).collect()),
        Value::I32s(vec![i32::MIN, 0, i32::MAX]),
        Value::Strs(vec![String::new(), "x".into()]),
    ];
    assert_eq!(child.call("echo", &values, &mut world).unwrap(), values);

    // A tick whose mods read 100 block states from Horizon.
    let got = child
        .call("nested", &[Value::I32(100)], &mut world)
        .unwrap();
    assert_eq!(world.reads, 100);
    assert_eq!(got, vec![Value::I64(100 * "minecraft:stone".len() as i64)]);

    let got = child.call("deep", &[Value::I32(3)], &mut world).unwrap();
    assert_eq!(got, vec![Value::Str("3<v2<v1<vbottom".into())]);

    child
        .call("events", &[Value::I32(500)], &mut world)
        .unwrap();
    assert_eq!(world.lines, (0..500).collect::<Vec<_>>());

    match child.call("fail", &[], &mut world) {
        Err(Error::Remote(m)) => assert_eq!(m, "java.lang.IllegalStateException: a mod threw"),
        other => panic!("{other:?}"),
    }
    // A failure leaves the connection usable.
    assert_eq!(
        child.call("ping", &[], &mut world).unwrap(),
        vec![Value::Str("pong".into())]
    );

    let status = child.stop(Duration::from_secs(10)).expect("exits");
    assert!(status.success(), "{status}");
}

#[test]
fn a_java_child_over_a_unix_socket() {
    if cfg!(unix) {
        conversation(false);
    }
}

#[test]
fn a_java_child_over_loopback_tcp() {
    conversation(true);
}

#[test]
fn a_child_that_dies_mid_call_is_an_error_with_its_last_words() {
    let Some((config, lines)) = support::config("crash", &[]) else {
        return;
    };
    let mut child = support::spawn(&config).expect("child starts");
    let result = child.call("crash", &[], &mut World::default());
    assert!(
        matches!(result, Err(Error::Closed | Error::Io(_))),
        "{result:?}"
    );
    std::thread::sleep(Duration::from_millis(300));
    assert!(!child.running());
    assert!(
        lines
            .lock()
            .unwrap()
            .iter()
            .any(|l| l == "[crash] crashing on purpose")
    );
}

#[test]
fn a_runtime_that_cannot_start_fails_spawn_or_the_first_call() {
    let Some((mut config, _)) = support::config("missing", &[]) else {
        return;
    };
    config.runtime = "no.such.Runtime".into();
    // The child connects, then fails to make the runtime and exits: the first call sees it.
    match support::spawn(&config) {
        Err(e) => assert!(e.contains("no.such.Runtime"), "{e}"),
        Ok(mut child) => {
            assert!(child.call("ping", &[], &mut World::default()).is_err());
            std::thread::sleep(Duration::from_millis(300));
            assert!(
                child
                    .last_lines()
                    .iter()
                    .any(|l| l.contains("ClassNotFoundException: no.such.Runtime"))
            );
        }
    }
}
