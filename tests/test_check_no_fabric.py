import importlib.util
import io
import tarfile
import tempfile
import unittest
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("check_no_fabric", ROOT / "tools/java_bridge/check_no_fabric.py")
check = importlib.util.module_from_spec(spec)
spec.loader.exec_module(check)


def jar(entries):
    data = io.BytesIO()
    with zipfile.ZipFile(data, "w", zipfile.ZIP_DEFLATED) as z:
        for name, body in entries.items():
            z.writestr(name, body)
    return data.getvalue()


class CheckNoFabricTests(unittest.TestCase):
    def scan(self, name, data):
        with tempfile.TemporaryDirectory() as d:
            path = Path(d) / name
            path.write_bytes(data)
            found = []
            check.scan_file(path, found)
            return found

    def release(self, members):
        data = io.BytesIO()
        with tarfile.open(fileobj=data, mode="w:gz") as t:
            for name, body in members.items():
                info = tarfile.TarInfo(name)
                info.size = len(body)
                t.addfile(info, io.BytesIO(body))
        return data.getvalue()

    def test_the_bridges_own_jar_inside_a_program_passes(self):
        bridge = jar({"dev/horizonlab/bridge/Boot.class": b"\xca\xfe\xba\xbe net/fabricmc/loader/api/FabricLoader",
                      "META-INF/services/net.fabricmc.loader.impl.game.GameProvider": b"dev.horizonlab.X\n"})
        program = b"\x7fELF" + b"\0" * 64 + bridge + b"fabric-loader-0.20.1.jar https://maven.fabricmc.net/"
        self.assertEqual(self.scan("horizon-client", program), [])

    def test_a_fabric_class_embedded_in_a_program_fails(self):
        program = b"\x7fELF" + jar({"net/fabricmc/loader/impl/FabricLoaderImpl.class": b"\xca\xfe"})
        self.assertTrue(self.scan("horizon-client", program))

    def test_fabric_jars_in_an_archive_fail_also_nested(self):
        mixin = jar({"org/spongepowered/asm/mixin/Mixin.class": b"\xca\xfe"})
        found = self.scan("game.tar.gz", self.release({"horizon/libs/sponge-mixin-0.17.5.jar": mixin}))
        self.assertTrue(any("ships horizon/libs/sponge-mixin-0.17.5.jar" in f for f in found), found)
        sat4j = jar({"org/sat4j/core/Vec.class": b"\xca\xfe"})
        found = self.scan("game.zip", jar({"horizon/x.zip": jar({"inner.bin": sat4j})}))
        self.assertTrue(any("org/sat4j/core/Vec.class" in f for f in found), found)

    def test_the_repository_holds_none(self):
        found = []
        check.scan_tree(found)
        self.assertEqual(found, [])


if __name__ == "__main__":
    unittest.main()
