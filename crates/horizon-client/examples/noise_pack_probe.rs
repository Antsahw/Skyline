//! Horizon's noise over a data pack (the Java mods' noise, `HorizonNoiseExport`): the noise
//! settings' debug functions at a few points, to compare with Java's
//! ("far terrain's noise at …" lines with HORIZON_JAVA_NOISE_EXPORT_CHECK=1).
//!
//! cargo run --profile fast -p horizon-client --example noise_pack_probe -- \
//!     .local/horizon-client/26.3 <world>/data/horizon_java_noise/minecraft_overworld [seed]

use horizon_client::assets::Assets;
use horizon_client::datapack::{self, DataPack};
use horizon_world::worldgen::generator::{GeneratorData, OverworldGenerator};
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let root = PathBuf::from(
        args.get(1)
            .ok_or("usage: noise_pack_probe <resources> <pack> [seed]")?,
    );
    let pack = PathBuf::from(
        args.get(2)
            .ok_or("usage: noise_pack_probe <resources> <pack> [seed]")?,
    );
    let seed: i64 = args.get(3).map_or(Ok(1234), |s| s.parse())?;
    // `noise:<id>` arguments: that noise of the pack at x 0 z of each point, seeded as
    // RandomState seeds it (the seed's Xoroshiro, positional, hash of the id).
    for arg in args.iter().skip(4).filter_map(|a| a.strip_prefix("noise:")) {
        use horizon_core::random::{PositionalRandom, RandomSource, XoroshiroRandom};
        let (ns, path) = arg.split_once(':').ok_or("noise:<ns>:<path>")?;
        let json =
            std::fs::read_to_string(pack.join(format!("data/{ns}/worldgen/noise/{path}.json")))?;
        let definition = horizon_world::worldgen::decode_noise(&json)?;
        let noise = definition.create(
            &mut XoroshiroRandom::new(seed)
                .fork_positional()
                .from_hash_of(arg),
        )?;
        for p in [
            [0, 64, 0],
            [500, 64, 500],
            [-800, 70, 300],
            [1200, 100, -900],
            [300, 40, -1500],
        ] {
            use horizon_core::noise::Noise;
            println!(
                "{arg} at {} {}: {}",
                p[0],
                p[2],
                noise.get3(f64::from(p[0]), 0.0, f64::from(p[2]))
            );
        }
    }
    let assets = Assets::load(&root)?;
    // More functions to show (registry ids), as the settings' debug functions.
    let ids: Vec<&String> = args
        .iter()
        .skip(4)
        .filter(|a| !a.starts_with("noise:"))
        .collect();
    let pack = if ids.is_empty() {
        pack
    } else {
        let copy = std::env::temp_dir().join("noise_pack_probe");
        let _ = std::fs::remove_dir_all(&copy);
        copy_dir(&pack, &copy)?;
        let file = copy.join("data/minecraft/worldgen/noise_settings/overworld.json");
        let mut settings: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&file)?)?;
        settings["debug_functions"] = ids
            .iter()
            .map(|id| serde_json::json!({"label": id, "function": id}))
            .collect();
        std::fs::write(&file, settings.to_string())?;
        copy
    };
    let ours = DataPack::folder("horizon/java_noise", &pack);
    let (overrides, warnings) = datapack::worldgen_overrides(&[&ours], |_| true);
    for warning in warnings {
        eprintln!("warning: {warning}");
    }
    let data = GeneratorData::load_with(&root.join("worldgen"), assets.registry, &overrides)?;
    let generator = OverworldGenerator::for_world(
        data,
        seed,
        "minecraft:normal",
        "minecraft:overworld",
        false,
    )?;
    for p in [
        [0, 64, 0],
        [500, 64, 500],
        [-800, 70, 300],
        [1200, 100, -900],
        [300, 40, -1500],
    ] {
        println!(
            "{} {} {}: {}",
            p[0],
            p[1],
            p[2],
            generator.debug_density_line(p).unwrap_or_default()
        );
    }
    Ok(())
}

fn copy_dir(from: &std::path::Path, to: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}
