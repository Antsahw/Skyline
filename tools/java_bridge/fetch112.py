#!/usr/bin/env python3
"""Fetch Cleanroom (Forge 1.12.2) and Mojang's 1.12.2 client for the bridge's 1.12 side.

    python3 -I tools/java_bridge/fetch112.py [--out ~/.cache/horizon-112]

Everything lands outside the repo (Mojang's files and Cleanroom's jars are never committed;
Cleanroom's licence forbids redistributing its binaries too). Pinned in research-lock.json
("cleanroom"): the installer's SHA-256; every library, the client jar and the asset index are
then checked against the SHA-1s in the installer's own version.json.

Writes into OUT/<cleanroom version>/:
  libraries/...          the version.json's libraries (this OS, this arch) + Cleanroom's universal
  natives/               the LWJGL/jinput natives, extracted
  versions/1.12.2.jar    Mojang's client (FML patches it in memory from binpatches.zip)
  assets/                the 1.12 asset index and objects
  launch.json            {"class_path": [...], "main_class", "tweakers", "natives", "assets", ...}
                         read by horizon-jvm's boot112.rs and run112.sh
"""
import argparse, concurrent.futures as cf, hashlib, json, os, platform, time, urllib.error, urllib.request, zipfile

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))


def get(url):
    req = urllib.request.Request(url, headers={"User-Agent": "horizon-bridge-112"})
    for attempt in range(6):
        try:
            with urllib.request.urlopen(req, timeout=180) as r:
                return r.read()
        except urllib.error.HTTPError as e:
            if e.code != 429 or attempt == 5:
                raise
            time.sleep(2 ** attempt)


def save(url, path, sha1=None):
    if os.path.exists(path) and (sha1 is None or digest(path, "sha1") == sha1):
        return path
    os.makedirs(os.path.dirname(path), exist_ok=True)
    data = get(url)
    if sha1 and hashlib.sha1(data).hexdigest() != sha1:
        raise SystemExit(f"SHA-1 mismatch: {url}")
    with open(path + ".tmp", "wb") as f:
        f.write(data)
    os.replace(path + ".tmp", path)
    return path


def digest(path, algo):
    h = hashlib.new(algo)
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def os_name():
    return {"Linux": "linux", "Darwin": "osx", "Windows": "windows"}[platform.system()]


def arch():
    m = platform.machine().lower()
    return {"x86_64": "x64", "amd64": "x64", "aarch64": "arm64", "arm64": "arm64"}.get(m, m)


def allowed(lib):
    rules = lib.get("rules")
    if not rules:
        return True
    ok = False
    for r in rules:
        o = r.get("os", {})
        match = ("name" not in o or o["name"] == os_name()) and ("arch" not in o or o["arch"] == arch())
        if match:
            ok = r["action"] == "allow"
    return ok


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", default=os.path.expanduser("~/.cache/horizon-112"))
    ap.add_argument("--no-assets", action="store_true", help="skip the asset objects (sounds, music)")
    a = ap.parse_args()
    lock = json.load(open(os.path.join(ROOT, "research-lock.json")))["cleanroom"]
    ver = lock["version"]
    out = os.path.join(a.out, ver)
    lib_dir = os.path.join(out, "libraries")

    installer = save(lock["installer_url"], os.path.join(out, f"cleanroom-{ver}-installer.jar"))
    if digest(installer, "sha256") != lock["installer_sha256"]:
        raise SystemExit("Cleanroom installer SHA-256 differs from research-lock.json")
    with zipfile.ZipFile(installer) as z:
        profile = json.loads(z.read("install_profile.json"))
        vj = json.loads(z.read("version.json"))

    jobs, class_path, natives = [], [], []
    # Cleanroom itself (the profile's library: its URL is empty, it lives on Cleanroom's maven).
    for lib in profile["libraries"]:
        art = lib["downloads"]["artifact"]
        path = os.path.join(lib_dir, art["path"])
        jobs.append((lock["maven"] + art["path"], path, art["sha1"]))
        class_path.append(path)
    for lib in vj["libraries"]:
        if not allowed(lib):
            continue
        d = lib.get("downloads", {})
        if "natives" in lib:
            key = lib["natives"].get(os_name())
            c = d.get("classifiers", {}).get(key) if key else None
            if c:
                path = os.path.join(lib_dir, c["url"].split("/maven2/")[-1].split("libraries.minecraft.net/")[-1])
                jobs.append((c["url"], path, c["sha1"]))
                natives.append(path)
            continue
        art = d.get("artifact")
        if not art or not art.get("url"):
            continue
        path = os.path.join(lib_dir, art["path"])
        jobs.append((art["url"], path, art["sha1"]))
        if path not in class_path:
            class_path.append(path)
    client = vj["downloads"]["client"]
    client_jar = os.path.join(out, "versions", "1.12.2.jar")
    jobs.append((client["url"], client_jar, client["sha1"]))
    class_path.append(client_jar)

    with cf.ThreadPoolExecutor(4) as ex:
        list(ex.map(lambda j: save(*j), jobs))
    print(f"libraries: {len(jobs)}")

    nat_dir = os.path.join(out, "natives")
    os.makedirs(nat_dir, exist_ok=True)
    for jar in natives:
        with zipfile.ZipFile(jar) as z:
            for n in z.namelist():
                if n.startswith("META-INF/") or n.endswith("/") or n.endswith(".sha1") or n.endswith(".git"):
                    continue
                target = os.path.join(nat_dir, os.path.basename(n))
                if not os.path.exists(target):
                    with open(target, "wb") as f:
                        f.write(z.read(n))

    ai = vj["assetIndex"]
    idx_path = save(ai["url"], os.path.join(out, "assets", "indexes", ai["id"] + ".json"), ai["sha1"])
    if not a.no_assets:
        objs = json.load(open(idx_path))["objects"].values()
        obj_jobs = [(f"https://resources.download.minecraft.net/{o['hash'][:2]}/{o['hash']}",
                     os.path.join(out, "assets", "objects", o["hash"][:2], o["hash"]), o["hash"]) for o in objs]
        with cf.ThreadPoolExecutor(16) as ex:
            list(ex.map(lambda j: save(*j), obj_jobs))
        print(f"asset objects: {len(obj_jobs)}")

    launch = {
        "cleanroom": ver,
        "minecraft": profile["minecraft"],
        "main_class": vj["mainClass"],
        "tweakers": profile["tweakers"],
        "class_path": class_path,
        "natives": nat_dir,
        "assets": os.path.join(out, "assets"),
        "asset_index": ai["id"],
        "java_major": profile["java"]["minimum"],
    }
    with open(os.path.join(out, "launch.json"), "w") as f:
        json.dump(launch, f, indent=1)
    print(os.path.join(out, "launch.json"))


if __name__ == "__main__":
    main()
