"""tools/java_bridge/fetch120.py: which of Mojang's libraries a machine gets, and the launch
description it writes (no network)."""
import importlib.util, os, unittest

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
spec = importlib.util.spec_from_file_location("fetch120", os.path.join(ROOT, "tools/java_bridge/fetch120.py"))
fetch120 = importlib.util.module_from_spec(spec)
spec.loader.exec_module(fetch120)


def lib(name, os_name=None, url=True):
    entry = {"name": name, "downloads": {"artifact": {
        "path": name.replace(":", "/") + ".jar", "sha1": "0" * 40,
        "url": "https://libraries.minecraft.net/" + name.replace(":", "/") + ".jar" if url else ""}}}
    if os_name:
        entry["rules"] = [{"action": "allow", "os": {"name": os_name}}]
    return entry


# The shape of 1.20.1's version JSON: plain libraries, LWJGL natives per OS and architecture
# by classifier, an OS-only library.
VERSION = {
    "id": "1.20.1",
    "javaVersion": {"majorVersion": 17},
    "assetIndex": {"id": "5"},
    "downloads": {"client": {"url": "https://x/client.jar", "sha1": "c" * 40},
                  "client_mappings": {"url": "https://x/client.txt", "sha1": "m" * 40}},
    "libraries": [
        lib("com.google.code.gson:gson:2.10"),
        lib("ca.weblite:java-objc-bridge:1.1", "osx"),
        lib("org.lwjgl:lwjgl:3.3.1:natives-linux", "linux"),
        lib("org.lwjgl:lwjgl:3.3.1:natives-macos", "osx"),
        lib("org.lwjgl:lwjgl:3.3.1:natives-macos-arm64", "osx"),
        lib("org.lwjgl:lwjgl:3.3.1:natives-windows", "windows"),
        lib("org.lwjgl:lwjgl:3.3.1:natives-windows-arm64", "windows"),
        lib("org.lwjgl:lwjgl:3.3.1:natives-windows-x86", "windows"),
        lib("org.lwjgl:lwjgl:3.3.1"),
        lib("com.example:no-url:1", url=False),
    ],
}

LOCK = {
    "java_major": 25,
    "loader": [{"file": "fabric-loader-0.20.1.jar", "url": "https://f/loader.jar", "sha256": "a" * 64},
               {"file": "asm-9.10.1.jar", "url": "https://f/asm.jar", "sha256": "b" * 64}],
    "intermediary_url": "https://f/intermediary-1.20.1-v2.jar", "intermediary_sha256": "d" * 64,
    "fabric_api_url": "https://f/fabric-api.jar", "fabric_api_mirror": "https://g/fabric-api.jar",
    "fabric_api_sha256": "e" * 64,
}


def names(machine_os, machine_arch):
    jobs, launch = fetch120.plan(VERSION, LOCK, "/o", machine_os, machine_arch)
    return [os.path.basename(p) for p in launch["class_path"]], jobs, launch


class Fetch120(unittest.TestCase):
    def test_linux_x86_64_gets_its_natives_only(self):
        cp, _, _ = names("linux", "x86_64")
        self.assertEqual([c for c in cp if "natives" in c], ["natives-linux.jar"])
        self.assertNotIn("1.1.jar", cp)  # java-objc-bridge is macOS only

    def test_windows_and_macs_pick_their_architecture(self):
        cp, _, _ = names("windows", "x86_64")
        self.assertEqual([c for c in cp if "natives" in c], ["natives-windows.jar"])
        cp, _, _ = names("windows", "arm64")
        self.assertEqual([c for c in cp if "natives" in c], ["natives-windows-arm64.jar"])
        cp, _, _ = names("osx", "arm64")
        self.assertEqual([c for c in cp if "natives" in c], ["natives-macos-arm64.jar"])

    def test_launch_order_and_what_is_pinned_how(self):
        cp, jobs, launch = names("linux", "x86_64")
        # Loader first, then the mappings, Mojang's libraries, the game jar last.
        self.assertEqual(cp[:3], ["fabric-loader-0.20.1.jar", "asm-9.10.1.jar", "intermediary-1.20.1-v2.jar"])
        self.assertEqual(cp[-1], "1.20.1.jar")
        self.assertEqual(launch["loader_version"], "0.20.1")
        self.assertEqual((launch["java_major"], launch["java_major_mojang"]), (25, 17))
        by_file = {os.path.basename(path): (urls, algo) for urls, path, algo, _ in jobs}
        self.assertEqual(by_file["fabric-api.jar"], (["https://f/fabric-api.jar", "https://g/fabric-api.jar"], "sha256"))
        self.assertEqual(by_file["client.txt"][1], "sha1")
        # A library without a URL is not downloaded (nor put on the class path).
        self.assertNotIn("1.jar", cp)


if __name__ == "__main__":
    unittest.main()
