#!/usr/bin/env python3
"""The 1.20.1 tracker (docs/java-bridge-120.md, phase 8): boots each of Modrinth's most
downloaded Fabric 1.20.1 mods alone, with its required dependencies, in the bridge's 1.20.1
runtime, and says how it went.

    python3 -I tools/java_bridge/survey120.py [--top 60] [--out ~/.cache/horizon-120/survey]
                                               [--only slug,slug] [--java /usr/lib/jvm/java-17-openjdk/bin/java]

Writes OUT/survey.tsv and OUT/survey.json: per mod its version, the verdict
  works    every entrypoint ran, no problems
  partial  the runtime booted, some entrypoint failed (which, and why)
  refused  Fabric Loader refused the set (a dependency range: the plan's reason 1)
  broken   the boot failed otherwise (the first error line)
the blocks and items it added, and scan120.py's findings for its jars. Jars come from Modrinth
(checked by SHA-512) into OUT/jars and are kept between runs. Needs fetch120.py.
"""
import argparse, hashlib, json, os, re, subprocess, sys, tempfile, time, urllib.parse, urllib.request

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import scan120  # noqa: E402

UA = {"User-Agent": "horizon-bridge-120-survey (github.com/iaxivers/horizon-java-bridge)"}
FABRIC_API = "P7dR8mSH"
VANILLA_BLOCKS, VANILLA_ITEMS = 1003, 1255


def api(path):
    url = "https://api.modrinth.com/v2/" + path
    for attempt in range(5):
        try:
            with urllib.request.urlopen(urllib.request.Request(url, headers=UA), timeout=60) as r:
                return json.load(r)
        except urllib.error.HTTPError as e:
            if e.code != 429 or attempt == 4:
                raise
            time.sleep(2 ** attempt)


def top(n):
    facets = urllib.parse.quote(json.dumps([["categories:fabric"], ["versions:1.20.1"], ["project_type:mod"]]))
    out = []
    while len(out) < n:
        hits = api(f"search?facets={facets}&index=downloads&limit={min(100, n - len(out))}&offset={len(out)}")["hits"]
        if not hits:
            break
        out += [(h["project_id"], h["slug"]) for h in hits]
    return out


def latest(project):
    q = urllib.parse.urlencode({"loaders": '["fabric"]', "game_versions": '["1.20.1"]'})
    versions = api(f"project/{project}/version?{q}")
    return versions[0] if versions else None


def download(version, jars):
    f = next((f for f in version["files"] if f["primary"]), version["files"][0])
    path = os.path.join(jars, f["filename"])
    if not os.path.exists(path):
        data = urllib.request.urlopen(urllib.request.Request(f["url"], headers=UA), timeout=180).read()
        if hashlib.sha512(data).hexdigest() != f["hashes"]["sha512"]:
            raise RuntimeError(f"{f['url']}: SHA-512 differs")
        with open(path + ".tmp", "wb") as fh:
            fh.write(data)
        os.replace(path + ".tmp", path)
    return path


def resolve(project, jars, cache):
    """The mod's jar and its required dependencies' jars (transitively), or an error."""
    paths, queue, seen, first = [], [project], set(), None
    while queue:
        p = queue.pop(0)
        if p in seen or p == FABRIC_API:
            continue
        seen.add(p)
        if p not in cache:
            cache[p] = latest(p)
        v = cache[p]
        if v is None:
            if p == project:
                return None, None, "no Fabric 1.20.1 version"
            return None, None, f"a required dependency ({p}) has no Fabric 1.20.1 version"
        first = first or v
        paths.append(download(v, jars))
        for d in v["dependencies"]:
            if d["dependency_type"] == "required" and d.get("project_id"):
                queue.append(d["project_id"])
    if first is None:
        return None, None, "Fabric API itself (always in the runtime)"
    # Modrinth's dependency lists miss some of what fabric.mod.json requires (More Culling:
    # Cloth Config): fetch those by mod id, which is usually the Modrinth slug.
    for _ in range(5):
        missing = undeclared(paths)
        if not missing:
            break
        for mod_id in missing:
            key = "id:" + mod_id
            if key not in cache:
                try:
                    cache[key] = latest(mod_id)
                except urllib.error.HTTPError:
                    cache[key] = None
            if cache[key] is None:
                return None, None, f"requires {mod_id}, which isn't on Modrinth under that name"
            paths.append(download(cache[key], jars))
    return first, paths, None


BUILT_IN = {"minecraft", "java", "fabricloader", "fabric", "fabric-api"}


def undeclared(paths):
    """Mod ids the jars' fabric.mod.json files require that none of them provides."""
    import zipfile
    provided, required = set(BUILT_IN), set()

    def read(z):
        try:
            meta = json.loads(z.read("fabric.mod.json").decode("utf-8-sig"), strict=False)
        except (KeyError, ValueError):
            return
        provided.add(meta.get("id"))
        provided.update(meta.get("provides", []) or [])
        required.update((meta.get("depends") or {}).keys())
        for n in z.namelist():
            if n.startswith("META-INF/jars/") and n.endswith(".jar"):
                import io
                try:
                    with zipfile.ZipFile(io.BytesIO(z.read(n))) as nested:
                        read(nested)
                except zipfile.BadZipFile:
                    pass

    for p in paths:
        with zipfile.ZipFile(p) as z:
            read(z)
    return sorted(r for r in required - provided if not r.startswith("fabric-"))


def build_bridge(java_home, out):
    src = os.path.join(ROOT, "crates/horizon-jvm/java/dev/horizonlab/bridge")
    files = [os.path.join(src, d, f) for d in ("remote", "r120") for f in sorted(os.listdir(os.path.join(src, d))) if f.endswith(".java")]
    subprocess.run([os.path.join(java_home, "bin", "javac"), "-proc:none", "-nowarn", "--release", "17", "-d", out] + files, check=True)


def java_options(java):
    out = subprocess.run([java, "-XshowSettings:properties", "-version"], capture_output=True, text=True).stderr
    m = re.search(r"java\.specification\.version = (\d+)", out)
    major = int(m.group(1)) if m else 0
    return major, (["--enable-native-access=ALL-UNNAMED"] if major >= 22 else []) + \
        (["--sun-misc-unsafe-memory-access=allow"] if major >= 24 else [])


def boot(java, options, classes, launch, jars, timeout):
    game = tempfile.mkdtemp(prefix="survey120-")
    os.makedirs(os.path.join(game, "mods"))
    for j in jars:
        os.symlink(j, os.path.join(game, "mods", os.path.basename(j)))
    report = os.path.join(game, "report.json")
    cmd = [java, "-Xmx2G"] + options + ["-cp", classes + os.pathsep + os.pathsep.join(launch["class_path"]),
           "dev.horizonlab.bridge.r120.Boot120", "--report", report, "--game-jar", launch["game_jar"],
           "--mappings", launch["mojang_mappings"], "--game-dir", game, "--add-mods", launch["fabric_api"]]
    started = time.time()
    try:
        run = subprocess.run(cmd, cwd=game, capture_output=True, text=True, timeout=timeout)
        output, code = run.stdout + run.stderr, run.returncode
    except subprocess.TimeoutExpired as e:
        output, code = (e.stdout or b"").decode(errors="replace") if isinstance(e.stdout, bytes) else (e.stdout or ""), "timeout"
    seconds = round(time.time() - started, 1)
    data = json.load(open(report)) if os.path.exists(report) else None
    subprocess.run(["rm", "-rf", game])
    return data, output, code, seconds


def first_error(output):
    lines = output.splitlines()
    # Fabric Loader's own refusal, in its words ("More details:" and the list under it).
    for i, l in enumerate(lines):
        if "More details:" in l:
            detail = [x.strip() for x in lines[i + 1: i + 6] if x.strip().startswith("-")]
            if detail:
                return "refused", detail[0][:300]
    for l in lines:
        if "Mod resolution failed" in l or "incompatible with the game or each other" in l:
            return "refused", l.strip()[:300]
    for l in lines:
        if re.search(r"(Exception|Error)(:|$)", l) and "WARN" not in l:
            return "broken", l.strip()[:300]
    return "broken", (lines[-1].strip() if lines else "no output")[:300]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--top", type=int, default=60)
    ap.add_argument("--only", help="comma-separated Modrinth slugs or ids instead of the top list")
    ap.add_argument("--out", default=os.path.expanduser("~/.cache/horizon-120/survey"))
    ap.add_argument("--java", help="the java executable (default: JAVA_HOME's, then the PATH's)")
    ap.add_argument("--timeout", type=int, default=240)
    a = ap.parse_args()
    cache_dir = os.environ.get("HORIZON_120_CACHE", os.path.expanduser("~/.cache/horizon-120"))
    launch = json.load(open(os.path.join(cache_dir, "1.20.1", "launch.json")))
    java = a.java or (os.path.join(os.environ["JAVA_HOME"], "bin", "java") if os.environ.get("JAVA_HOME") else "java")
    java_home = os.path.dirname(os.path.dirname(os.path.realpath(subprocess.run(
        ["sh", "-c", "command -v javac"], capture_output=True, text=True).stdout.strip() or java)))
    major, options = java_options(java)
    jars = os.path.join(a.out, "jars")
    os.makedirs(jars, exist_ok=True)
    classes = tempfile.mkdtemp(prefix="survey120-classes-")
    build_bridge(java_home, classes)
    loader = scan120.Loader([p for p in launch["class_path"] if os.path.basename(p).startswith("fabric-loader-")])
    projects = [(p, p) for p in a.only.split(",")] if a.only else top(a.top)
    projects = [(p, s) for p, s in projects if p != FABRIC_API and s != "fabric-api"]
    versions, rows = {}, []
    for n, (project, slug) in enumerate(projects, 1):
        row = {"slug": slug}
        try:
            version, paths, why = resolve(project, jars, versions)
        except Exception as e:  # a Modrinth hiccup: say so, go on
            version, paths, why = None, None, f"download: {e}"
        if why:
            row.update(verdict="skipped", why=why)
        else:
            row["version"] = version["version_number"]
            row["jars"] = [os.path.basename(p) for p in paths]
            scanned = {}
            import zipfile
            for p in paths:
                with zipfile.ZipFile(p) as z:
                    scan120.scan_jar(z, os.path.basename(p), loader, scanned)
            row["scan"] = {k: scan120.verdict(v) for k, v in scanned.items() if scan120.verdict(v)}
            report, output, code, seconds = boot(java, options, classes, launch, paths, a.timeout)
            row["seconds"] = seconds
            if report:
                failed = {f"{m} {k}": v for m, e in report["entrypoints"].items() for k, v in e.items() if v != "ok"}
                row["blocks"] = len(report["blocks"]) - VANILLA_BLOCKS if isinstance(report["blocks"], dict) else None
                row["items"] = len(report["items"]) - VANILLA_ITEMS if isinstance(report["items"], dict) else None
                row["mods"] = len(report["mods"])
                if failed or report["problems"]:
                    row.update(verdict="partial", why="; ".join(f"{k}: {v[:160]}" for k, v in failed.items()) or "; ".join(report["problems"])[:400])
                else:
                    row["verdict"] = "works"
            else:
                verdict, why = first_error(output)
                row.update(verdict=verdict, why=f"exit {code}: {why}")
        rows.append(row)
        print(f"[{n}/{len(projects)}] {slug} {row.get('version', '')}: {row['verdict']}"
              f"{'  ' + row['why'][:140] if row.get('why') else ''}", flush=True)
    subprocess.run(["rm", "-rf", classes])
    with open(os.path.join(a.out, "survey.json"), "w") as fh:
        json.dump({"java": major, "loader": loader.version, "mods": rows}, fh, indent=1)
    with open(os.path.join(a.out, "survey.tsv"), "w") as fh:
        fh.write("slug\tversion\tverdict\tblocks\titems\tseconds\tscan\twhy\n")
        for r in rows:
            fh.write("\t".join(str(r.get(k, "")) for k in ("slug", "version", "verdict", "blocks", "items", "seconds"))
                     + "\t" + ",".join(sorted({x for v in r.get("scan", {}).values() for x in v})) + "\t" + r.get("why", "") + "\n")
    counts = {}
    for r in rows:
        counts[r["verdict"]] = counts.get(r["verdict"], 0) + 1
    print(f"Java {major}, Fabric Loader {loader.version}: " + ", ".join(f"{v} {k}" for k, v in sorted(counts.items())))
    print(os.path.join(a.out, "survey.tsv"))


if __name__ == "__main__":
    main()
