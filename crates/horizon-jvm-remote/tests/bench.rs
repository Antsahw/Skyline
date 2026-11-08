//! The transport's cost, measured against a real Java child (`docs/java-bridge-120.md`,
//! "Measured"). Ignored by default; run it optimized:
//!   cargo test -p horizon-jvm-remote --release --test bench -- --ignored --nocapture

mod support;

use horizon_jvm_remote::{Handler, Peer, Value};
use std::time::Instant;

struct World;
impl Handler for World {
    fn call(&mut self, _: &mut Peer, _: &str, _: Vec<Value>) -> Result<Vec<Value>, String> {
        Ok(vec![Value::Str("minecraft:stone".into())])
    }
    fn event(&mut self, _: &mut Peer, _: &str, _: Vec<Value>) {}
}

fn run(tcp: bool) {
    let name = if tcp { "bench-tcp" } else { "bench-unix" };
    let Some((mut config, _)) = support::config(name, &[]) else {
        return;
    };
    config.tcp = tcp;
    config.jvm_args.clear(); // the full JIT, as a game runs
    let mut child = support::spawn(&config).expect("child");
    let w = &mut World;
    // Warm both JITs.
    for _ in 0..20_000 {
        child.call("ping", &[], w).unwrap();
    }
    child.call("nested", &[Value::I32(50_000)], w).unwrap();
    child.call("events", &[Value::I32(50_000)], w).unwrap();

    let n = 50_000;
    let t = Instant::now();
    for _ in 0..n {
        child.call("ping", &[], w).unwrap();
    }
    let ping = t.elapsed().as_nanos() as f64 / n as f64 / 1000.0;

    let t = Instant::now();
    child.call("nested", &[Value::I32(n)], w).unwrap();
    let nested = t.elapsed().as_nanos() as f64 / n as f64 / 1000.0;

    let m = 500_000;
    let t = Instant::now();
    child.call("events", &[Value::I32(m)], w).unwrap();
    let events = m as f64 / t.elapsed().as_secs_f64() / 1e6;

    let blob = Value::Bytes(vec![7; 4 << 20]);
    let t = Instant::now();
    for _ in 0..20 {
        child.call("echo", std::slice::from_ref(&blob), w).unwrap();
    }
    let mb_s = (20.0 * 2.0 * 4.0) / t.elapsed().as_secs_f64();

    println!(
        "{name}: Rust->Java call {ping:.1} us | Java->Rust call inside a call {nested:.1} us | \
         events {events:.2} M/s | bulk {mb_s:.0} MB/s"
    );
    child.stop(std::time::Duration::from_secs(5));
}

#[test]
#[ignore]
fn transport_cost() {
    run(false);
    run(true);
}
