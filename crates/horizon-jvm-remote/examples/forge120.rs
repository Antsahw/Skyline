//! Starts Forge 1.20.1 with the bridge's platform and the mods in `<game>/mods`, and prints its
//! report (docs/java-bridge-120-forge.md, phase 4):
//!
//!     cargo run -p horizon-jvm-remote --example forge120 -- INSTALL PLATFORM_JAR GAME_DIR [JAVA]
//!
//! INSTALL is fetch120.py's folder for 1.20.1 (`~/.cache/horizon-120/1.20.1`), PLATFORM_JAR
//! build120forge.sh's `platform120-forge.jar`. Forge's client needs a display (its window is
//! hidden once the mods have loaded).

use horizon_jvm_remote::forge120::{Forge120, start};
use std::time::Duration;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 3 {
        eprintln!("usage: forge120 INSTALL PLATFORM_JAR GAME_DIR [JAVA]");
        std::process::exit(2);
    }
    let config = Forge120 {
        java: args.get(3).map(Into::into).unwrap_or_else(|| "java".into()),
        install: args[0].clone().into(),
        platform: args[1].clone().into(),
        game_dir: args[2].clone().into(),
        max_heap: "3G".into(),
        jvm_args: Vec::new(),
        timeout: Duration::from_secs(600),
        log: std::sync::Arc::new(|l| eprintln!("{l}")),
    };
    match start(&config) {
        Ok((mut runtime, report)) => {
            println!("{report}");
            println!("ping: {:?}", runtime.call("ping"));
            // The content facts too, to a file, when asked (`FORGE120_FACTS=PATH`).
            if let Ok(path) = std::env::var("FORGE120_FACTS") {
                let path = horizon_jvm_remote::wire::Value::Str(path);
                println!("facts: {:?}", runtime.call_with("facts", &[path]));
            }
            runtime.stop();
        }
        Err(e) => {
            eprintln!("Forge 1.20.1 didn't start: {e}");
            std::process::exit(1);
        }
    }
}
