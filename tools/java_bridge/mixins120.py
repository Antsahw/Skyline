#!/usr/bin/env python3
"""Which vanilla methods Fabric 1.20.1 mods change with mixins, counted over Modrinth's most
downloaded mods (docs/java-bridge-120.md, "Mixins on vanilla behaviour").

    python3 -I tools/java_bridge/mixins120.py [--top 150] [--out ~/.cache/horizon-120/mixins]
    python3 -I tools/java_bridge/mixins120.py --jars DIR       # the jars in DIR instead

For each mod's mixin classes it reads the class's @Mixin targets and each handler's target
method (@Inject, @Redirect, @ModifyArg(s), @ModifyVariable, @ModifyConstant, MixinExtras'
@WrapOperation, @ModifyExpressionValue, @ModifyReturnValue, @WrapWithCondition, and
@Overwrite), resolves it through the mod's refmap to intermediary names, and names it as
Mojang does (the runtime's mappings-1.20.1.tiny). Accessors and invokers change nothing and are
left out. Writes OUT/mixins.tsv (method, mods, handlers, side, the mods) and prints the top.
"""
import argparse
import collections
import json
import os
import re
import struct
import sys
import zipfile

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

HANDLERS = {
    "Lorg/spongepowered/asm/mixin/injection/Inject;": "inject",
    "Lorg/spongepowered/asm/mixin/injection/Redirect;": "redirect",
    "Lorg/spongepowered/asm/mixin/injection/ModifyArg;": "modify_arg",
    "Lorg/spongepowered/asm/mixin/injection/ModifyArgs;": "modify_args",
    "Lorg/spongepowered/asm/mixin/injection/ModifyVariable;": "modify_variable",
    "Lorg/spongepowered/asm/mixin/injection/ModifyConstant;": "modify_constant",
    "Lcom/llamalad7/mixinextras/injector/wrapoperation/WrapOperation;": "wrap_operation",
    "Lcom/llamalad7/mixinextras/injector/ModifyExpressionValue;": "modify_expression",
    "Lcom/llamalad7/mixinextras/injector/ModifyReturnValue;": "modify_return",
    "Lcom/llamalad7/mixinextras/injector/WrapWithCondition;": "wrap_condition",
    "Lcom/llamalad7/mixinextras/injector/v2/WrapWithCondition;": "wrap_condition",
    "Lcom/llamalad7/mixinextras/injector/wrapmethod/WrapMethod;": "wrap_method",
}
MIXIN = "Lorg/spongepowered/asm/mixin/Mixin;"
OVERWRITE = "Lorg/spongepowered/asm/mixin/Overwrite;"
DEFAULT_TINY = os.path.expanduser("~/.cache/horizon-120/1.20.1/dev/mappings-1.20.1.tiny")


# ---------------------------------------------------------------------------------------------
# Class files: just enough to read a class's and its methods' annotations.

class Reader:
    def __init__(self, data):
        self.d, self.p = data, 0

    def u1(self):
        self.p += 1
        return self.d[self.p - 1]

    def u2(self):
        self.p += 2
        return struct.unpack_from(">H", self.d, self.p - 2)[0]

    def u4(self):
        self.p += 4
        return struct.unpack_from(">I", self.d, self.p - 4)[0]


def read_class(data):
    """(class name, [class annotations], [(method name, method desc, [annotations])]), each
    annotation (type descriptor, {element: value}); None for anything that isn't a class."""
    if data[:4] != b"\xca\xfe\xba\xbe":
        return None
    r = Reader(data)
    r.p = 8
    count = r.u2()
    pool = [None] * count
    i = 1
    while i < count:
        tag = r.u1()
        if tag == 1:
            n = r.u2()
            pool[i] = data[r.p:r.p + n].decode("utf-8", "replace")
            r.p += n
        elif tag in (3, 4):
            pool[i] = ("num", r.u4())
        elif tag in (5, 6):
            r.p += 8
            i += 1
        elif tag in (7, 8, 16, 19, 20):
            pool[i] = (tag, r.u2())
        elif tag in (9, 10, 11, 12, 17, 18):
            pool[i] = (tag, r.u2(), r.u2())
        elif tag == 15:
            r.p += 3
        else:
            return None
        i += 1

    def utf(idx):
        v = pool[idx] if 0 < idx < count else None
        return v if isinstance(v, str) else None

    def value():
        tag = chr(r.u1())
        if tag in "BCDFIJSZs":
            idx = r.u2()
            return utf(idx) if tag == "s" else pool[idx]
        if tag == "e":
            r.u2()
            return utf(r.u2())
        if tag == "c":
            return utf(r.u2())
        if tag == "@":
            return annotation()
        if tag == "[":
            return [value() for _ in range(r.u2())]
        raise ValueError(tag)

    def annotation():
        kind = utf(r.u2())
        elements = {}
        for _ in range(r.u2()):
            name = utf(r.u2())
            elements[name] = value()
        return kind, elements

    def attributes():
        found = []
        for _ in range(r.u2()):
            name = utf(r.u2())
            length = r.u4()
            end = r.p + length
            if name in ("RuntimeInvisibleAnnotations", "RuntimeVisibleAnnotations"):
                found += [annotation() for _ in range(r.u2())]
            r.p = end
        return found

    r.u2()
    this = pool[r.u2()]
    name = utf(this[1]) if isinstance(this, tuple) else None
    r.u2()
    interfaces = r.u2()
    r.p += 2 * interfaces
    for _ in range(r.u2()):  # fields
        r.p += 6
        attributes()
    methods = []
    for _ in range(r.u2()):
        r.u2()
        m_name, m_desc = utf(r.u2()), utf(r.u2())
        methods.append((m_name, m_desc, attributes()))
    return name, attributes(), methods


# ---------------------------------------------------------------------------------------------
# Names: refmap strings to intermediary, intermediary to Mojang's.

class Names:
    def __init__(self, tiny):
        self.classes, self.methods, self.any_method = {}, {}, {}
        cls = None
        with open(tiny, encoding="utf-8") as fh:
            header = fh.readline().rstrip("\n").split("\t")
            cols = header[3:]
            inter, named = cols.index("intermediary"), cols.index("named")
            for line in fh:
                parts = line.rstrip("\n").split("\t")
                if parts[0] == "c":
                    cls = parts[1 + inter]
                    self.classes[cls] = parts[1 + named]
                elif len(parts) > 1 and parts[1] == "m" and cls:
                    # \tm\tdesc(official)\tofficial\tintermediary\tnamed
                    self.methods[(cls, parts[3 + inter])] = parts[3 + named]
                    # Intermediary method names are unique: one an owner inherits is found too.
                    self.any_method[parts[3 + inter]] = parts[3 + named]

    def cls(self, intermediary):
        return self.classes.get(intermediary, intermediary)

    def method(self, owner, intermediary):
        found = self.methods.get((owner, intermediary))
        return found or self.any_method.get(intermediary, intermediary)


TARGET = re.compile(r"^(?:L([\w/$]+);)?([\w$<>]+)?(\(.*)?$")


def resolve(target, owners, refmap, names):
    """`target` (an @Inject's method string) as [(owner, method)] in Mojang's names."""
    mapped = refmap.get(target, target)
    m = TARGET.match(mapped.strip())
    if not m:
        return []
    owner, method = m.group(1), m.group(2) or mapped
    if method.startswith("*") or not method:
        return []
    out = []
    for o in ([owner] if owner else owners):
        if not o:
            continue
        out.append((names.cls(o).replace("/", "."), names.method(o, method)))
    return out


def scan_jar(path, names, nested=True):
    """{(owner, method): {handler kinds}} and the side ("client", "server", "both") of each, for
    one jar and the jars it bundles."""
    found = collections.defaultdict(lambda: [set(), set()])
    try:
        z = zipfile.ZipFile(path)
    except zipfile.BadZipFile:
        return found
    with z:
        configs = []
        try:
            meta = json.loads(z.read("fabric.mod.json"))
            for c in meta.get("mixins", []):
                if isinstance(c, str):
                    configs.append((c, "both"))
                elif isinstance(c, dict) and "config" in c:
                    configs.append((c["config"], c.get("environment", "both")))
            if nested:
                for jar in meta.get("jars", []):
                    inner = jar.get("file")
                    if inner in z.namelist():
                        import io
                        sub = scan_jar(io.BytesIO(z.read(inner)), names, False)
                        for k, (kinds, sides) in sub.items():
                            found[k][0] |= kinds
                            found[k][1] |= sides
        except (KeyError, ValueError):
            return found
        for config, env in configs:
            try:
                cfg = json.loads(z.read(config))
            except (KeyError, ValueError):
                continue
            refmap = {}
            if cfg.get("refmap"):
                try:
                    refmap_all = json.loads(z.read(cfg["refmap"])).get("mappings", {})
                except (KeyError, ValueError):
                    refmap_all = {}
            else:
                refmap_all = {}
            package = cfg.get("package", "")
            lists = [("mixins", "both"), ("client", "client"), ("server", "server")]
            for key, side in lists:
                for name in cfg.get(key, []) or []:
                    cls_path = f"{package}.{name}".replace(".", "/") + ".class"
                    try:
                        parsed = read_class(z.read(cls_path))
                    except (KeyError, ValueError, IndexError, struct.error):
                        continue
                    if not parsed:
                        continue
                    this, annotations, methods = parsed
                    owners = []
                    for kind, el in annotations:
                        if kind == MIXIN:
                            for v in el.get("value", []) or []:
                                if isinstance(v, str):
                                    owners.append(v[1:-1] if v.startswith("L") else v)
                            for v in el.get("targets", []) or []:
                                owners.append(refmap_all.get(this, {}).get(v, v).replace(".", "/"))
                    refmap = refmap_all.get(this, {})
                    s = side if env == "both" else env
                    for m_name, m_desc, m_ann in methods:
                        for kind, el in m_ann:
                            if kind in HANDLERS:
                                targets = el.get("method", []) or []
                                for t in targets if isinstance(targets, list) else [targets]:
                                    if isinstance(t, str):
                                        for key2 in resolve(t, owners, refmap, names):
                                            found[key2][0].add(HANDLERS[kind])
                                            found[key2][1].add(s)
                            elif kind == OVERWRITE:
                                for o in owners:
                                    key2 = (names.cls(o).replace("/", "."), names.method(o, m_name))
                                    found[key2][0].add("overwrite")
                                    found[key2][1].add(s)
    return found


def side_of(sides):
    if sides == {"client"}:
        return "client"
    if sides == {"server"}:
        return "server"
    return "both"


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--top", type=int, default=150)
    ap.add_argument("--out", default=os.path.expanduser("~/.cache/horizon-120/mixins"))
    ap.add_argument("--jars", help="scan the jars in this folder instead of downloading")
    ap.add_argument("--tiny", default=os.environ.get("HORIZON_120_TINY", DEFAULT_TINY))
    ap.add_argument("--show", type=int, default=60)
    args = ap.parse_args()
    names = Names(args.tiny)
    os.makedirs(args.out, exist_ok=True)
    jars = []
    if args.jars:
        jars = [(os.path.splitext(f)[0], os.path.join(args.jars, f))
                for f in sorted(os.listdir(args.jars)) if f.endswith(".jar")]
    else:
        import survey120
        folder = os.path.join(args.out, "jars")
        os.makedirs(folder, exist_ok=True)
        for project, slug in survey120.top(args.top):
            if project == survey120.FABRIC_API:
                continue  # the runtime's own: its mixins are the hooks the mods call
            try:
                version = survey120.latest(project)
                if version:
                    jars.append((slug, survey120.download(version, folder)))
            except Exception as e:  # noqa: BLE001 - one mod's download failing skips it
                print(f"{slug}: {e}", file=sys.stderr)
    per = collections.defaultdict(lambda: {"mods": set(), "kinds": set(), "sides": set()})
    for slug, path in jars:
        for key, (kinds, sides) in scan_jar(path, names).items():
            per[key]["mods"].add(slug)
            per[key]["kinds"] |= kinds
            per[key]["sides"] |= sides
    rows = sorted(per.items(), key=lambda kv: (-len(kv[1]["mods"]), kv[0]))
    with open(os.path.join(args.out, "mixins.tsv"), "w", encoding="utf-8") as fh:
        fh.write("method\tmods\thandlers\tside\tmod list\n")
        for (owner, method), v in rows:
            fh.write(f"{owner}.{method}\t{len(v['mods'])}\t{','.join(sorted(v['kinds']))}\t"
                     f"{side_of(v['sides'])}\t{','.join(sorted(v['mods']))}\n")
    print(f"{len(jars)} mods, {len(rows)} vanilla methods mixed into; {os.path.join(args.out, 'mixins.tsv')}")
    for (owner, method), v in rows[: args.show]:
        print(f"{len(v['mods']):4}  {side_of(v['sides']):6}  {owner.rsplit('.', 1)[-1]}.{method}")


if __name__ == "__main__":
    main()
