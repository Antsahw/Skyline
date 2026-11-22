#!/usr/bin/env python3
"""Fetch and install Forge 1.20.1 for the bridge's Forge runtime (docs/java-bridge-120-forge.md, phase 1).

    python3 -I tools/java_bridge/fetch120forge.py [--out ~/.cache/horizon-120] [--forge 47.4.0] [--boot]

Run fetch120.py first: Forge installs over its Mojang 1.20.1 client. Forge's own installer
does the install, as a launcher runs it (Forge is LGPL-2.1: it is run as it ships, never read,
never committed). The Forge version is the one asked for, else Forge's recommended 1.20.1 build;
the installer is checked against the SHA-1 Forge's maven publishes beside it.

Writes into OUT/1.20.1/forge/:
  installer/forge-<v>-installer.jar   the installer
  versions/, libraries/               the installed client: Forge's version JSON, its libraries,
                                      the patched client its processors made
  launch.json                         the client's launch: main class, JVM and game arguments
                                      with the paths filled in, the class path
  server/                             with --boot: a Forge dedicated server, booted once with no
                                      mods to see Forge start (phase 1's check)
"""
import argparse, hashlib, json, os, shutil, subprocess, sys, urllib.request

MAVEN = "https://maven.minecraftforge.net/net/minecraftforge/forge"
PROMOTIONS = "https://files.minecraftforge.net/net/minecraftforge/forge/promotions_slim.json"


def get(url):
    req = urllib.request.Request(url, headers={"User-Agent": "horizon-bridge-120"})
    with urllib.request.urlopen(req, timeout=180) as r:
        return r.read()


def java():
    """Forge 1.20.1's Java: HORIZON_FORGE_JAVA_HOME, else a 17 or 21 (its ModLauncher reads
    classes up to Java 21's), else the default one."""
    home = os.environ.get("HORIZON_FORGE_JAVA_HOME")
    if home:
        return os.path.join(home, "bin", "java")
    for v in ("21", "17"):
        for d in (f"/usr/lib/jvm/java-{v}-openjdk", f"/usr/lib/jvm/java-{v}-openjdk-amd64", f"/usr/lib/jvm/jdk-{v}"):
            if os.path.exists(os.path.join(d, "bin", "java")):
                return os.path.join(d, "bin", "java")
    home = os.environ.get("JAVA_HOME")
    if home:
        return os.path.join(home, "bin", "java")
    return shutil.which("java") or "java"


def installer(forge_dir, version):
    """The installer jar, downloaded once and checked against Forge's published SHA-1."""
    full = f"1.20.1-{version}"
    path = os.path.join(forge_dir, "installer", f"forge-{full}-installer.jar")
    if os.path.exists(path):
        return path
    os.makedirs(os.path.dirname(path), exist_ok=True)
    url = f"{MAVEN}/{full}/forge-{full}-installer.jar"
    data = get(url)
    expected = get(url + ".sha1").decode().split()[0].strip()
    actual = hashlib.sha1(data).hexdigest()
    if actual != expected:
        sys.exit(f"{url}: SHA-1 {actual}, Forge publishes {expected}")
    with open(path + ".part", "wb") as f:
        f.write(data)
    os.replace(path + ".part", path)
    print(f"installer: {path} (sha1 {actual})")
    return path


def launcher_dir(base, forge_dir):
    """A launcher's folder, as the installer expects one: Mojang's 1.20.1 client in it."""
    vdir = os.path.join(forge_dir, "versions", "1.20.1")
    os.makedirs(vdir, exist_ok=True)
    for name in ("1.20.1.json", "1.20.1.jar"):
        target = os.path.join(vdir, name)
        if not os.path.exists(target):
            shutil.copyfile(os.path.join(base, "versions", name), target)
    profiles = os.path.join(forge_dir, "launcher_profiles.json")
    if not os.path.exists(profiles):
        with open(profiles, "w") as f:
            json.dump({"profiles": {}}, f)


def substitute(arg, values):
    for k, v in values.items():
        arg = arg.replace("${" + k + "}", v)
    return arg


def plain(args):
    """A version JSON's arguments, the plain strings only (rule-gated ones are the launcher's)."""
    return [a for a in args if isinstance(a, str)]


def library_path(forge_dir, lib):
    art = lib.get("downloads", {}).get("artifact")
    if art and art.get("path"):
        return os.path.join(forge_dir, "libraries", art["path"])
    group, name, ver = lib["name"].split(":")[:3]
    return os.path.join(forge_dir, "libraries", *group.split("."), name, ver, f"{name}-{ver}.jar")


def launch(base, forge_dir, version):
    """launch.json from the installed version JSON over Mojang's."""
    vid = f"1.20.1-forge-{version}"
    vjson = os.path.join(forge_dir, "versions", vid, f"{vid}.json")
    if not os.path.exists(vjson):
        sys.exit(f"the installer wrote no {vjson}")
    forge = json.load(open(vjson))
    mojang = json.load(open(os.path.join(base, "versions", "1.20.1.json")))
    sep = os.pathsep
    values = {
        "library_directory": os.path.join(forge_dir, "libraries"),
        "classpath_separator": sep,
        "version_name": vid,
    }
    class_path, seen = [], set()
    for lib in forge.get("libraries", []):
        p = library_path(forge_dir, lib)
        if p not in seen:
            seen.add(p)
            class_path.append(p)
    # Mojang's libraries after Forge's (the launcher's order), the client jar itself left out:
    # Forge runs the patched client its processors made, from its libraries.
    for p in json.load(open(os.path.join(base, "launch.json")))["class_path"]:
        if "/libraries/" in p and p not in seen and "/net/fabricmc/" not in p:
            seen.add(p)
            class_path.append(p)
    missing = [p for p in class_path if not os.path.exists(p)]
    out = {
        "minecraft": "1.20.1",
        "forge": version,
        "version_id": vid,
        "main_class": forge["mainClass"],
        "jvm_args": [substitute(a, values) for a in plain(forge.get("arguments", {}).get("jvm", []))],
        "game_args": [substitute(a, values) for a in plain(forge.get("arguments", {}).get("game", []))],
        "class_path": class_path,
        "java_major": mojang.get("javaVersion", {}).get("majorVersion", 17),
    }
    path = os.path.join(forge_dir, "launch.json")
    with open(path, "w") as f:
        json.dump(out, f, indent=1)
    print(f"launch: {path} ({len(class_path)} class path entries, {len(missing)} missing)")
    for p in missing[:10]:
        print(f"  missing: {p}")
    return out


def boot(forge_dir, version, jar):
    """Phase 1's check: a Forge 1.20.1 dedicated server, no mods, booted until it is ready."""
    server = os.path.join(forge_dir, "server")
    os.makedirs(server, exist_ok=True)
    if not os.path.exists(os.path.join(server, "run.sh")):
        subprocess.run([java(), "-jar", jar, "--installServer", server], check=True)
    with open(os.path.join(server, "eula.txt"), "w") as f:
        f.write("eula=true\n")
    with open(os.path.join(server, "server.properties"), "w") as f:
        f.write("online-mode=false\nlevel-type=minecraft\\:flat\nspawn-protection=0\nserver-port=25599\n")
    args_file = os.path.join(server, "libraries", "net", "minecraftforge", "forge",
                             f"1.20.1-{version}", "unix_args.txt")
    proc = subprocess.Popen([java(), "-Xmx2G", f"@{args_file}", "nogui"], cwd=server,
                            stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
    ok = False
    log = open(os.path.join(server, "boot.log"), "w")
    for line in proc.stdout:
        log.write(line)
        if "Done (" in line and "For help" in line:
            ok = True
            proc.stdin.write("stop\n")
            proc.stdin.flush()
    proc.wait()
    print(f"boot: {'Forge 1.20.1 ' + version + ' started' if ok else 'did not start'} ({server}/boot.log)")
    return ok


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", default=os.path.expanduser("~/.cache/horizon-120"))
    ap.add_argument("--forge", help="the Forge version (47.x); default: Forge's recommended 1.20.1 build")
    ap.add_argument("--boot", action="store_true", help="boot a Forge dedicated server once, no mods")
    a = ap.parse_args()
    base = os.path.join(a.out, "1.20.1")
    if not os.path.exists(os.path.join(base, "launch.json")):
        sys.exit("run tools/java_bridge/fetch120.py first")
    version = a.forge
    if not version:
        promos = json.loads(get(PROMOTIONS))["promos"]
        version = promos.get("1.20.1-recommended") or promos["1.20.1-latest"]
    print(f"Forge {version}, Java {java()}")
    forge_dir = os.path.join(base, "forge")
    jar = installer(forge_dir, version)
    launcher_dir(base, forge_dir)
    if not os.path.isdir(os.path.join(forge_dir, "versions", f"1.20.1-forge-{version}")):
        subprocess.run([java(), "-jar", jar, "--installClient", forge_dir], check=True)
    launch(base, forge_dir, version)
    if a.boot and not boot(forge_dir, version, jar):
        sys.exit(1)


if __name__ == "__main__":
    main()
