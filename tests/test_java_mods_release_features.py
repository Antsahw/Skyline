"""The Fabric mods release builds (java-mods-release.yml, package_java_variant.sh) build the game with
`java-mods` alone: never `java-mods-dev` (the HORIZON_JAVA_MODS=1 opt-in that skips the owner
record, horizon-jvm's gate.rs) nor `java-mods-prototypes` (NeoForge, Forge 1.12), and `java-mods`
itself pulls in neither (docs/java-bridge.md, "Ownership")."""
import re
import tomllib
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
RELEASE_BUILDS = [ROOT / ".github/workflows/java-mods-release.yml",
                  ROOT / "tools/java_bridge/package_java_variant.sh"]
DEV_ONLY = ("java-mods-dev", "java-mods-prototypes", "horizon-jvm/dev", "horizon-jvm/prototypes")


def features_enabled_by(manifest, feature, seen=None):
    """Every feature `feature` turns on in `manifest`'s [features], itself included."""
    seen = set() if seen is None else seen
    if feature in seen:
        return seen
    seen.add(feature)
    for dep in manifest["features"].get(feature, []):
        if "/" in dep:
            seen.add(dep)
        elif not dep.startswith("dep:"):
            features_enabled_by(manifest, dep, seen)
    return seen


class ReleaseBuildFeatures(unittest.TestCase):
    def test_release_builds_name_java_mods_alone(self):
        for path in RELEASE_BUILDS:
            text = path.read_text()
            builds = [line for line in text.splitlines() if "cargo build" in line and "horizon-client" in line]
            self.assertTrue(builds, f"{path.name} builds the game")
            for line in builds:
                self.assertRegex(line, r"--features java-mods(\s|$)", f"{path.name}: {line.strip()}")
            for name in DEV_ONLY:
                self.assertNotIn(name, text, f"{path.name} enables {name}")

    def test_java_mods_turns_on_no_developer_feature(self):
        manifest = tomllib.loads((ROOT / "crates/horizon-client/Cargo.toml").read_text())
        on = features_enabled_by(manifest, "java-mods")
        self.assertFalse(on & set(DEV_ONLY), on)
        # The developers' feature does add the opt-in (the harnesses depend on it).
        self.assertIn("horizon-jvm/dev", features_enabled_by(manifest, "java-mods-dev"))
        self.assertIn("horizon-jvm/prototypes", features_enabled_by(manifest, "java-mods-prototypes"))
        jvm = tomllib.loads((ROOT / "crates/horizon-jvm/Cargo.toml").read_text())
        self.assertNotIn("default", jvm.get("features", {}), "horizon-jvm has no default features")


if __name__ == "__main__":
    unittest.main()
