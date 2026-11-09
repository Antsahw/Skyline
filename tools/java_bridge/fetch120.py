#!/usr/bin/env python3
"""Fetch Mojang's 1.20.1 client and the 1.20.1 Fabric stack for the bridge's 1.20.1 runtime.

    python3 -I tools/java_bridge/fetch120.py [--out ~/.cache/horizon-120] [--assets]

The 1.20.1 runtime runs in a JVM of its own (docs/java-bridge-120.md), with its own pins in
research-lock.json ("fabric120"), apart from the 26.3 side's: Mojang's version JSON by SHA-1
(it vouches for the client jar, Mojang's mappings and every library by SHA-1), Fabric Loader
and its libraries, the 1.20.1 intermediary mappings and Fabric API by SHA-256. Everything lands
outside the repo: Mojang's files are never committed.

Writes into OUT/1.20.1/:
  versions/1.20.1.json, versions/1.20.1.jar   Mojang's client (official names: Fabric Loader
                                              remaps it to intermediary at the first boot)
  versions/client.txt                         Mojang's mappings (official <- Mojang's names:
                                              the boot and report code are written in those)
  libraries/...                               the client's libraries for this OS and arch
  fabric/...                                  Fabric Loader + ASM + Mixin, intermediary, Fabric API;
                                              fabric/tools/: build120.sh's remapper
  assets/                                     with --assets: the index and objects (later phases)
  launch.json                                 read by horizon-jvm-remote's boot120.rs and run120.sh
"""
import argparse, concurrent.futures as cf, hashlib, json, os, platform, sys, time, urllib.error, urllib.request

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))


def get(url):
    req = urllib.request.Request(url, headers={"User-Agent": "horizon-bridge-120"})
    for attempt in range(6):
        try:
            with urllib.request.urlopen(req, timeout=180) as r:
                return r.read()
        except urllib.error.HTTPError as e:
            if e.code != 429 or attempt == 5:
                raise
            time.sleep(2 ** attempt)


def digest(path, algo):
    h = hashlib.new(algo)
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def save(urls, path, algo, expected):
    """Downloads the first of `urls` that answers into `path`, unless a good copy is there.
    Refuses bytes whose hash differs from `expected`."""
    if isinstance(urls, str):
        urls = [urls]
    if os.path.exists(path) and digest(path, algo) == expected:
        return path
    os.makedirs(os.path.dirname(path), exist_ok=True)
    last = None
    for url in urls:
        try:
            data = get(url)
        except (urllib.error.URLError, OSError) as e:
            last = e
            continue
        got = hashlib.new(algo, data).hexdigest()
        if got != expected:
            raise SystemExit(f"{url}: {algo} {got}, expected {expected}: refusing it")
        with open(path + ".tmp", "wb") as f:
            f.write(data)
        os.replace(path + ".tmp", path)
        return path
    raise SystemExit(f"{os.path.basename(path)}: no source answered ({last})")


def os_name():
    return {"Linux": "linux", "Darwin": "osx", "Windows": "windows"}[platform.system()]


def arch():
    m = platform.machine().lower()
    return {"x86_64": "x86_64", "amd64": "x86_64", "aarch64": "arm64", "arm64": "arm64"}.get(m, m)


def allowed(lib, name=None, machine=None):
    """Mojang's library rules: the last rule that matches this OS decides; no rules, allowed."""
    name, machine = name or os_name(), machine or arch()
    rules = lib.get("rules")
    if not rules:
        ok = True
    else:
        ok = False
        for r in rules:
            o = r.get("os", {})
            if ("name" not in o or o["name"] == name) and ("arch" not in o or o["arch"] == machine):
                ok = r["action"] == "allow"
    if not ok:
        return False
    # 1.20.1 lists LWJGL's natives per architecture by classifier, without an arch rule:
    # keep only this machine's (natives-linux, natives-linux-arm64, natives-macos-arm64...).
    lib_name = lib.get("name", "")
    if ":natives-" in lib_name:
        classifier = lib_name.rsplit(":natives-", 1)[1]
        wants_arm = machine == "arm64"
        has_arm = classifier.endswith("-arm64") or classifier.endswith("-aarch64")
        if wants_arm != has_arm or classifier.endswith("-x86"):
            return False
    return True


def plan(vj, lock, out, name=None, machine=None):
    """The downloads (url(s), path, algo, hash) and the launch description, from the version
    JSON and the lock. Pure: no network, for tests/test_fetch120.py."""
    lib_dir = os.path.join(out, "libraries")
    fab_dir = os.path.join(out, "fabric")
    jobs, mc_libs, loader = [], [], []
    for lib in lock["loader"]:
        path = os.path.join(fab_dir, lib["file"])
        jobs.append(([lib["url"]], path, "sha256", lib["sha256"]))
        loader.append(path)
    intermediary = os.path.join(fab_dir, os.path.basename(lock["intermediary_url"]))
    jobs.append(([lock["intermediary_url"]], intermediary, "sha256", lock["intermediary_sha256"]))
    fabric_api = os.path.join(fab_dir, os.path.basename(lock["fabric_api_url"]))
    sources = [lock["fabric_api_url"]] + ([lock["fabric_api_mirror"]] if lock.get("fabric_api_mirror") else [])
    jobs.append((sources, fabric_api, "sha256", lock["fabric_api_sha256"]))
    build_tools = []
    for tool in lock.get("build_tools", []):
        path = os.path.join(fab_dir, "tools", tool["file"])
        jobs.append(([tool["url"]], path, "sha256", tool["sha256"]))
        build_tools.append(path)
    for lib in vj["libraries"]:
        if not allowed(lib, name, machine):
            continue
        art = lib.get("downloads", {}).get("artifact")
        if not art or not art.get("url"):
            continue
        path = os.path.join(lib_dir, art["path"])
        if path in mc_libs:
            continue
        jobs.append(([art["url"]], path, "sha1", art["sha1"]))
        mc_libs.append(path)
    client = vj["downloads"]["client"]
    game_jar = os.path.join(out, "versions", vj["id"] + ".jar")
    jobs.append(([client["url"]], game_jar, "sha1", client["sha1"]))
    mappings = vj["downloads"]["client_mappings"]
    mojang = os.path.join(out, "versions", "client.txt")
    jobs.append(([mappings["url"]], mojang, "sha1", mappings["sha1"]))
    launch = {
        "minecraft": vj["id"],
        # The class path in the order a Fabric launcher profile uses: the loader and its
        # libraries, the mappings, Mojang's libraries, then the game jar.
        "class_path": loader + [intermediary] + mc_libs + [game_jar],
        "game_jar": game_jar,
        "mojang_mappings": mojang,
        "fabric_api": fabric_api,
        # build120.sh's remapper: never on a game's class path.
        "build_tools": build_tools,
        "loader_version": os.path.basename(loader[0])[len("fabric-loader-"):-len(".jar")],
        "java_major": lock["java_major"],
        "java_major_mojang": vj["javaVersion"]["majorVersion"],
        "assets": os.path.join(out, "assets"),
        "asset_index": vj["assetIndex"]["id"],
    }
    return jobs, launch


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", default=os.path.expanduser("~/.cache/horizon-120"))
    ap.add_argument("--assets", action="store_true", help="also the asset objects (textures and sounds: later phases)")
    a = ap.parse_args()
    lock = json.load(open(os.path.join(ROOT, "research-lock.json")))["fabric120"]
    out = os.path.join(a.out, lock["minecraft"])
    vj_path = save(lock["version_json_url"], os.path.join(out, "versions", lock["minecraft"] + ".json"),
                   "sha1", lock["version_json_sha1"])
    vj = json.load(open(vj_path))
    jobs, launch = plan(vj, lock, out)
    with cf.ThreadPoolExecutor(6) as ex:
        list(ex.map(lambda j: save(*j), jobs))
    print(f"files: {len(jobs) + 1} (Fabric Loader {launch['loader_version']}, "
          f"{os.path.basename(launch['fabric_api'])})")
    if a.assets:
        ai = vj["assetIndex"]
        idx = save(ai["url"], os.path.join(out, "assets", "indexes", ai["id"] + ".json"), "sha1", ai["sha1"])
        objs = json.load(open(idx))["objects"].values()
        obj_jobs = [(f"https://resources.download.minecraft.net/{o['hash'][:2]}/{o['hash']}",
                     os.path.join(out, "assets", "objects", o["hash"][:2], o["hash"]), "sha1", o["hash"]) for o in objs]
        with cf.ThreadPoolExecutor(16) as ex:
            list(ex.map(lambda j: save(*j), obj_jobs))
        print(f"asset objects: {len(obj_jobs)}")
    with open(os.path.join(out, "launch.json"), "w") as f:
        json.dump(launch, f, indent=1)
    print(os.path.join(out, "launch.json"))


if __name__ == "__main__":
    sys.exit(main())
