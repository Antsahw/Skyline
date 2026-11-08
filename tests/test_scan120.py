"""tools/java_bridge/scan120.py: Fabric's version ranges, and finding the loader API a mod uses
that the runtime's loader lacks (a fake loader and mod, compiled here; skipped without javac)."""
import importlib.util, json, os, shutil, subprocess, tempfile, unittest, zipfile

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
spec = importlib.util.spec_from_file_location("scan120", os.path.join(ROOT, "tools/java_bridge/scan120.py"))
scan120 = importlib.util.module_from_spec(spec)
spec.loader.exec_module(scan120)


class Ranges(unittest.TestCase):
    def test_what_mods_write_in_fabric_mod_json(self):
        cases = [
            ("0.20.1", ">=0.14.21", True),
            ("0.20.1", "<=0.16.4", False),
            ("0.16.4", "<=0.16.4", True),
            ("0.16.5", "<0.16.5", False),
            ("0.20.1", "0.16.x", False),
            ("0.16.14", "0.16.x", True),
            ("0.20.1", "~0.15", False),
            ("0.15.11", "~0.15", True),
            ("0.20.1", "^0.14", False),  # caret on 0.x pins the minor
            ("0.14.25", "^0.14", True),
            ("0.20.1", "*", True),
            ("0.20.1", ">=0.14 <0.17", False),
            ("0.16.10", ">=0.14 <0.17", True),
            ("0.20.1", "0.16.4", False),
            ("0.20.1", ">=0.15.0-beta.1", True),
            ("1.20.1", "1.20.x", True),
            ("1.20.1", ">=1.20 <1.20.2-", True),
        ]
        for version, predicate, expected in cases:
            self.assertEqual(scan120.satisfies(version, predicate), expected, f"{version} vs {predicate}")
        # A list means any of them.
        self.assertTrue(scan120.satisfies_any("0.20.1", ["<0.15", ">=0.19"]))
        self.assertTrue(scan120.satisfies_any("0.20.1", None))


@unittest.skipUnless(shutil.which("javac"), "no javac")
class MissingLoaderApi(unittest.TestCase):
    def test_classes_and_methods_the_loader_lacks_are_listed_and_present_ones_are_not(self):
        tmp = tempfile.mkdtemp()
        try:
            def compile_to(out, sources, cp=None):
                src = os.path.join(tmp, "src-" + os.path.basename(out))
                files = []
                for name, text in sources.items():
                    path = os.path.join(src, name)
                    os.makedirs(os.path.dirname(path), exist_ok=True)
                    open(path, "w").write(text)
                    files.append(path)
                cmd = ["javac", "-nowarn", "-d", out] + (["-cp", cp] if cp else []) + files
                subprocess.run(cmd, check=True, capture_output=True)

            def jar(path, folder, extra=None):
                with zipfile.ZipFile(path, "w") as z:
                    for base, _, names in os.walk(folder):
                        for n in names:
                            full = os.path.join(base, n)
                            z.write(full, os.path.relpath(full, folder))
                    for name, text in (extra or {}).items():
                        z.writestr(name, text)

            old_api = {
                "net/fabricmc/loader/api/Thing.java": "package net.fabricmc.loader.api; public class Thing { public static void kept() {} public static void dropped() {} }",
                "net/fabricmc/loader/impl/Gone.java": "package net.fabricmc.loader.impl; public class Gone { public static void x() {} }",
            }
            new_api = {
                "net/fabricmc/loader/api/Thing.java": "package net.fabricmc.loader.api; public class Thing { public static void kept() {} }",
            }
            mod = {"m/Mod.java": "package m; public class Mod { void f() { net.fabricmc.loader.api.Thing.kept();"
                                 " net.fabricmc.loader.api.Thing.dropped(); net.fabricmc.loader.impl.Gone.x(); } }"}
            compile_to(os.path.join(tmp, "old"), old_api)
            compile_to(os.path.join(tmp, "new"), new_api)
            compile_to(os.path.join(tmp, "mod"), mod, os.path.join(tmp, "old"))
            loader_jar = os.path.join(tmp, "loader.jar")
            jar(loader_jar, os.path.join(tmp, "new"), {"fabric.mod.json": json.dumps({"id": "fabricloader", "version": "0.20.1"})})
            mods = os.path.join(tmp, "mods")
            os.makedirs(mods)
            jar(os.path.join(mods, "old-mod.jar"), os.path.join(tmp, "mod"),
                {"fabric.mod.json": json.dumps({"id": "oldmod", "version": "1", "depends": {"fabricloader": "<=0.16.4"}})})

            loader = scan120.Loader([loader_jar])
            out = {}
            with zipfile.ZipFile(os.path.join(mods, "old-mod.jar")) as z:
                scan120.scan_jar(z, "old-mod.jar", loader, out)
            facts = out["oldmod"]
            self.assertEqual(facts["missing"], ["net.fabricmc.loader.api.Thing.dropped()V", "net.fabricmc.loader.impl.Gone"])
            self.assertEqual(facts["internal"], 1)
            self.assertFalse(facts["loader_ok"])
            self.assertEqual(scan120.verdict(facts), ["1:range", "3:missing-loader-api"])
        finally:
            shutil.rmtree(tmp)


if __name__ == "__main__":
    unittest.main()
