//! Fabric ids with `-` and `.`, and namespaces of textures only.
use super::*;

/// A Fabric id with `-` (Cloth Config's `cloth-config`) is carried: the content mod's id
/// has `_`, the original stays its namespace; a namespace of textures its code draws by
/// name (`cloth-config2`, named by no JSON) is its too, and Horizon's reader accepts it.
#[test]
fn carries_a_hyphenated_id_and_its_texture_namespace() {
    let dir = std::env::temp_dir().join(format!("horizon-port-pack-dash-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let jar = dir.join("cloth-config-1.0.jar");
    let mut zip = zip::ZipWriter::new(fs::File::create(&jar).unwrap());
    let files: &[(&str, &str)] = &[
        (
            "fabric.mod.json",
            r#"{"schemaVersion": 1, "id": "cloth-config", "version": "1.0", "license": "LGPL-3.0"}"#,
        ),
        ("assets/cloth-config2/textures/gui/cloth_config.png", "png"),
        (
            "assets/cloth-config2/lang/en_us.json",
            r#"{"text.cloth-config.reset": "Reset"}"#,
        ),
        // A namespace with data stays outside unless its files are named.
        ("data/other_lib/loot_table/x.json", r#"{"pools": []}"#),
    ];
    for (name, text) in files {
        zip.start_file(*name, zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(text.as_bytes()).unwrap();
    }
    zip.finish().unwrap();
    let info = crate::jar::read(&jar).unwrap();
    let out = dir.join("work/mods/cloth-config");
    let pack = Pack {
        loaded: &|_| true,
        mods: &|_| false,
    };
    let report = carry(&jar, &info, &out, None, &|_| true, &pack, &BTreeMap::new()).unwrap();
    assert_eq!(report.namespaces, ["cloth-config2"]);
    assert_eq!(report.outside, ["data/other_lib/loot_table/x.json"]);
    let carried = out.join("carry");
    assert!(
        carried
            .join("assets/cloth-config2/textures/gui/cloth_config.png")
            .is_file()
    );
    let package = horizon_mods::Package::read(&carried).unwrap();
    assert_eq!(package.manifest.id, "cloth_config");
    assert_eq!(
        package.manifest.namespaces,
        ["cloth-config", "cloth-config2"]
    );
    fs::remove_dir_all(&dir).unwrap();
}
