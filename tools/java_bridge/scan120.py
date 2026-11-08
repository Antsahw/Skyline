#!/usr/bin/env python3
"""What stands between each Fabric 1.20.1 mod and the bridge's 1.20.1 runtime, read from the jars
(docs/java-bridge-120.md, "Mods that need an older Fabric Loader"). Class files only: nothing runs.

    python3 -I tools/java_bridge/scan120.py MODS_DIR [--json out.json]

Per mod (and every jar it nests in META-INF/jars):
  range     its declared fabricloader / minecraft / java ranges, and whether the runtime's
            loader (launch.json's, the pinned one) satisfies them  -> reason 1: a dependency override
  missing   Fabric Loader classes, methods and fields it references that the runtime's loader
            jar doesn't have                                         -> reason 3: loader-compat
  internal  how many references go to net.fabricmc.loader.impl (works today; breaks easily)
  jdk       APIs newer Javas removed or locked down (SecurityManager, Thread.stop, Unsafe)
  java      the newest class file version (61 = Java 17)
The loader jar comes from fetch120.py's launch.json (HORIZON_120_CACHE, default
~/.cache/horizon-120) unless --loader names one.
"""
import argparse, io, json, os, re, struct, sys, zipfile

# Java APIs newer Javas removed or restricted: (owner, member or None for any use).
JDK_RISKS = {
    ("java/lang/SecurityManager", None): "SecurityManager (Java 24+: setting one throws)",
    ("java/lang/System", "setSecurityManager"): "System.setSecurityManager (Java 24+: throws)",
    ("java/lang/Thread", "stop"): "Thread.stop (Java 20+: throws)",
    ("java/lang/Thread", "suspend"): "Thread.suspend (removed)",
    ("java/lang/Thread", "resume"): "Thread.resume (removed)",
    ("sun/misc/Unsafe", None): "sun.misc.Unsafe (memory methods warn from Java 24, go away later)",
}


def parse_class(data):
    """(this class, super class, referenced classes, referenced members [(kind, owner, name, desc)]),
    or None for a damaged class."""
    if data[:4] != b"\xca\xfe\xba\xbe":
        return None
    major = struct.unpack_from(">H", data, 6)[0]
    count = struct.unpack_from(">H", data, 8)[0]
    pool = [None] * count
    pos, i = 10, 1
    try:
        while i < count:
            tag = data[pos]
            pos += 1
            if tag == 1:
                n = struct.unpack_from(">H", data, pos)[0]
                pool[i] = data[pos + 2: pos + 2 + n].decode("utf-8", "replace")
                pos += 2 + n
            elif tag in (3, 4):
                pos += 4
            elif tag in (5, 6):
                pos += 8
                i += 1
            elif tag in (7, 8, 16, 19, 20):
                pool[i] = (tag, struct.unpack_from(">H", data, pos)[0])
                pos += 2
            elif tag in (9, 10, 11, 12):
                pool[i] = (tag,) + struct.unpack_from(">HH", data, pos)
                pos += 4
            elif tag in (17, 18):
                pos += 4
            elif tag == 15:
                pos += 3
            else:
                return None
            i += 1
        access, this_idx, super_idx = struct.unpack_from(">HHH", data, pos)
    except (IndexError, struct.error):
        return None

    def utf(idx):
        v = pool[idx] if 0 < idx < count else None
        return v if isinstance(v, str) else None

    def cls(idx):
        v = pool[idx] if 0 < idx < count else None
        return utf(v[1]) if isinstance(v, tuple) and v[0] == 7 else None

    classes, members = set(), set()
    for v in pool:
        if not isinstance(v, tuple):
            continue
        if v[0] == 7:
            name = utf(v[1])
            if name:
                classes.add(name.lstrip("[").removeprefix("L").removesuffix(";"))
        elif v[0] in (9, 10, 11):
            owner = cls(v[1])
            nat = pool[v[2]] if 0 < v[2] < count else None
            if owner and isinstance(nat, tuple) and nat[0] == 12:
                kind = "field" if v[0] == 9 else "method"
                members.add((kind, owner, utf(nat[1]), utf(nat[2])))
    return cls(this_idx), cls(super_idx), classes, members, major


class Loader:
    """The runtime's Fabric Loader jar: which classes and members it has."""

    def __init__(self, jars):
        self.classes = {}  # name -> ([super and interfaces], {(kind, name, desc)})
        self.version = None
        for jar in jars:
            with zipfile.ZipFile(jar) as z:
                for n in z.namelist():
                    if n == "fabric.mod.json" and self.version is None:
                        try:
                            self.version = json.loads(z.read(n))["version"]
                        except (ValueError, KeyError):
                            pass
                    if not n.endswith(".class"):
                        continue
                    data = z.read(n)
                    parsed = parse_class(data)
                    if not parsed:
                        continue
                    supers, members = own_members(data)
                    self.classes[parsed[0]] = (supers, members)

    def has_class(self, name):
        return name in self.classes

    def has_member(self, kind, owner, name, desc):
        """Whether `owner` or a supertype declares the member. A supertype outside the loader
        jar other than Object (a JDK class such as URLClassLoader) can't be checked here, so it
        counts as having it."""
        queue, seen = [owner], set()
        while queue:
            c = queue.pop(0)
            if c in seen:
                continue
            seen.add(c)
            if c in self.classes:
                supers, members = self.classes[c]
                if (kind, name, desc) in members:
                    return True
                queue.extend(supers)
            elif c == "java/lang/Object":
                if kind == "method" and name in OBJECT_METHODS:
                    return True
            else:
                return True
        return False


OBJECT_METHODS = {"toString", "hashCode", "equals", "getClass", "notify", "notifyAll", "wait", "clone", "finalize"}


def own_members(data):
    """A class's direct supertypes (superclass first, then interfaces) and the members it
    declares: ([names], {(kind, name, descriptor)})."""
    count = struct.unpack_from(">H", data, 8)[0]
    pool = [None] * count
    pos, i = 10, 1
    while i < count:
        tag = data[pos]
        pos += 1
        if tag == 1:
            n = struct.unpack_from(">H", data, pos)[0]
            pool[i] = data[pos + 2: pos + 2 + n].decode("utf-8", "replace")
            pos += 2 + n
        elif tag in (5, 6):
            pos += 8
            i += 1
        elif tag in (7, 8, 16, 19, 20):
            pos += 2
        elif tag == 15:
            pos += 3
        else:  # 3, 4, 9-12, 17, 18
            pos += 4
        i += 1
    classes = {}
    p, i = 10, 1
    # A second, light walk for the class entries' names (the first stored only UTF-8 strings).
    while i < count:
        tag = data[p]
        if tag == 7:
            classes[i] = struct.unpack_from(">H", data, p + 1)[0]
        p += {1: 3 + (struct.unpack_from(">H", data, p + 1)[0] if tag == 1 else 0), 5: 9, 6: 9, 7: 3, 8: 3, 16: 3,
              19: 3, 20: 3, 15: 4}.get(tag, 5)
        i += 2 if tag in (5, 6) else 1
    name = lambda idx: pool[classes[idx]] if idx in classes else None
    super_idx = struct.unpack_from(">H", data, pos + 4)[0]
    pos += 6
    interfaces = struct.unpack_from(">H", data, pos)[0]
    supers = [name(super_idx)] if super_idx else []
    supers += [name(struct.unpack_from(">H", data, pos + 2 + 2 * k)[0]) for k in range(interfaces)]
    pos += 2 + 2 * interfaces
    out = set()
    for kind in ("field", "method"):
        n = struct.unpack_from(">H", data, pos)[0]
        pos += 2
        for _ in range(n):
            _, name_idx, desc_idx, attrs = struct.unpack_from(">HHHH", data, pos)
            pos += 8
            for _ in range(attrs):
                length = struct.unpack_from(">I", data, pos + 2)[0]
                pos += 6 + length
            out.add((kind, pool[name_idx], pool[desc_idx]))
    return [x for x in supers if x], out


# Fabric's version predicates, as far as mods use them: ">=0.14.21", "<0.17", "~0.15", "^0.14",
# "0.16.x", "*", a bare version (exact), several separated by spaces (all must hold); a list in
# fabric.mod.json is any of them.
def parse_version(text):
    core = re.split(r"[-+]", text, maxsplit=1)[0]
    parts = []
    for p in core.split("."):
        if p in ("x", "X", "*"):
            parts.append(None)
        else:
            m = re.match(r"\d+", p)
            parts.append(int(m.group()) if m else 0)
    return parts


def cmp(a, b):
    n = max(len(a), len(b))
    a = [x or 0 for x in a] + [0] * (n - len(a))
    b = [x or 0 for x in b] + [0] * (n - len(b))
    return (a > b) - (a < b)


def satisfies(version, predicate):
    v = parse_version(version)
    for term in predicate.split():
        if term == "*":
            continue
        m = re.match(r"(>=|<=|>|<|=|~|\^)?(.+)", term)
        op, target = m.group(1) or "=", m.group(2)
        t = parse_version(target)
        if None in t:  # 0.16.x: the fixed parts must match
            fixed = t[: t.index(None)]
            if v[: len(fixed)] != fixed:
                return False
            continue
        c = cmp(v, t)
        if op == ">=" and c < 0 or op == ">" and c <= 0 or op == "<=" and c > 0 or op == "<" and c >= 0:
            return False
        if op == "=" and c != 0:
            return False
        if op == "~" and (c < 0 or v[:2] != (t + [0, 0])[:2]):
            return False
        if op == "^" and (c < 0 or v[:1] != t[:1] or (t[0] == 0 and v[:2] != (t + [0])[:2])):
            return False
    return True


def satisfies_any(version, predicates):
    if predicates is None:
        return True
    if isinstance(predicates, str):
        predicates = [predicates]
    return any(satisfies(version, p) for p in predicates)


def scan_jar(z, label, loader, out):
    """Scans one jar (an open ZipFile) and its nested jars into `out` (mod id -> facts)."""
    try:
        meta = json.loads(z.read("fabric.mod.json").decode("utf-8-sig"), strict=False)
    except (KeyError, ValueError):
        meta = None
    if meta is None:
        return
    mod_id = meta.get("id", label)
    depends = meta.get("depends", {}) or {}
    facts = {
        "jar": label,
        "version": meta.get("version"),
        "fabricloader": depends.get("fabricloader"),
        "minecraft": depends.get("minecraft"),
        "java": depends.get("java"),
        "missing": set(),
        "internal": 0,
        "jdk": set(),
        "java_class": 0,
    }
    for n in z.namelist():
        if n.endswith(".class"):
            parsed = parse_class(z.read(n))
            if not parsed:
                continue
            _, _, classes, members, major = parsed
            facts["java_class"] = max(facts["java_class"], major)
            for c in classes:
                if c.startswith("net/fabricmc/loader/"):
                    if c.startswith("net/fabricmc/loader/impl/"):
                        facts["internal"] += 1
                    if not loader.has_class(c):
                        facts["missing"].add(c.replace("/", "."))
                for (owner, member), why in JDK_RISKS.items():
                    if member is None and c == owner:
                        facts["jdk"].add(why)
            for kind, owner, name, desc in members:
                if owner.startswith("net/fabricmc/loader/") and loader.has_class(owner):
                    if not loader.has_member(kind, owner, name, desc):
                        facts["missing"].add(f"{owner.replace('/', '.')}.{name}{desc if kind == 'method' else ''}")
                why = JDK_RISKS.get((owner, name))
                if why:
                    facts["jdk"].add(why)
        elif n.startswith("META-INF/jars/") and n.endswith(".jar"):
            try:
                with zipfile.ZipFile(io.BytesIO(z.read(n))) as nested:
                    scan_jar(nested, f"{label}!{os.path.basename(n)}", loader, out)
            except zipfile.BadZipFile:
                pass
    facts["loader_ok"] = satisfies_any(loader.version, facts["fabricloader"]) if loader.version else None
    facts["minecraft_ok"] = satisfies_any("1.20.1", facts["minecraft"])
    facts["missing"] = sorted(facts["missing"])
    facts["jdk"] = sorted(facts["jdk"])
    out[mod_id] = facts


def verdict(f):
    """Which of the plan's reasons apply (empty: nothing found)."""
    reasons = []
    if f["loader_ok"] is False:
        reasons.append("1:range")
    if f["missing"]:
        reasons.append("3:missing-loader-api")
    if not f["minecraft_ok"]:
        reasons.append("not-1.20.1")
    if f["jdk"]:
        reasons.append("jdk")
    return reasons


def loader_jars(path):
    if path:
        return [path]
    cache = os.environ.get("HORIZON_120_CACHE", os.path.expanduser("~/.cache/horizon-120"))
    launch = json.load(open(os.path.join(cache, "1.20.1", "launch.json")))
    return [p for p in launch["class_path"] if os.path.basename(p).startswith("fabric-loader-")]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("mods")
    ap.add_argument("--loader", help="the Fabric Loader jar to check against (default: launch.json's)")
    ap.add_argument("--json", help="write every fact here")
    a = ap.parse_args()
    loader = Loader(loader_jars(a.loader))
    out = {}
    for name in sorted(os.listdir(a.mods)):
        if name.endswith(".jar"):
            try:
                with zipfile.ZipFile(os.path.join(a.mods, name)) as z:
                    scan_jar(z, name, loader, out)
            except zipfile.BadZipFile:
                print(f"{name}: not a jar", file=sys.stderr)
    flagged = 0
    for mod_id, f in sorted(out.items()):
        reasons = verdict(f)
        if not reasons and not f["internal"]:
            continue
        flagged += bool(reasons)
        print(f"{mod_id} {f['version']}  [{', '.join(reasons) or 'ok'}]  loader {f['fabricloader']}"
              f"{'  internal refs ' + str(f['internal']) if f['internal'] else ''}")
        for m in f["missing"][:12]:
            print(f"    missing: {m}")
        for j in f["jdk"]:
            print(f"    jdk: {j}")
    print(f"{len(out)} mods (nested included) against Fabric Loader {loader.version}: {flagged} with a reason")
    if a.json:
        with open(a.json, "w") as fh:
            json.dump({"loader": loader.version, "mods": {k: dict(v, reasons=verdict(v)) for k, v in out.items()}}, fh, indent=1)


if __name__ == "__main__":
    main()
