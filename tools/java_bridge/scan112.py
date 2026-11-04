#!/usr/bin/env python3
"""What a folder of Forge 1.12.2 mod jars needs from the bridge.

Reads class files only (never runs them): superclasses and interfaces (resolved through every
jar, so a mod's own base classes count), @SubscribeEvent handlers' event types, capability use,
coremods and mixins, GL calls, world generation hooks, GUIs, renderers, networking.

    python3 -I tools/java_bridge/scan112.py <mods-folder> [--tsv out.tsv] [--events N]

Prints a summary (totals, the most-subscribed Forge events, the mods with most of each kind).
Used for docs/java-bridge-112.md (SkyFactory 4.2.4's server pack).
"""
import argparse
import collections
import json
import struct
import sys
import zipfile
from pathlib import PurePosixPath

# Vanilla/Forge bases (MCP class names: Forge 1.12 jars name classes deobfuscated) -> kind.
BASES = {
    "net/minecraft/block/Block": "block",
    "net/minecraft/item/Item": "item",
    "net/minecraft/tileentity/TileEntity": "tile",
    "net/minecraft/entity/Entity": "entity",
    "net/minecraft/entity/EntityLiving": "mob",
    "net/minecraft/inventory/Container": "container",
    "net/minecraft/client/gui/GuiScreen": "gui",
    "net/minecraft/client/gui/inventory/GuiContainer": "gui_container",
    "net/minecraft/client/renderer/tileentity/TileEntitySpecialRenderer": "tesr",
    "net/minecraft/client/renderer/entity/Render": "entity_render",
    "net/minecraft/client/model/ModelBase": "entity_model",
    "net/minecraft/world/WorldProvider": "dimension",
    "net/minecraft/world/WorldType": "world_type",
    "net/minecraft/world/biome/Biome": "biome",
    "net/minecraft/world/gen/feature/WorldGenerator": "feature",
    "net/minecraft/world/gen/structure/MapGenStructure": "structure",
    "net/minecraft/item/crafting/IRecipe": "recipe",
    "net/minecraftforge/registries/IForgeRegistryEntry$Impl": None,
}
# Vanilla subclasses mods extend (not in any mod jar, so the walk stops there).
for _base, _subs in {
    "block": "BlockContainer BlockBush BlockCrops BlockSapling BlockLeaves BlockLog BlockRotatedPillar"
    " BlockHorizontal BlockDirectional BlockSlab BlockStairs BlockFence BlockFenceGate BlockDoor"
    " BlockTrapDoor BlockPane BlockFalling BlockBreakable BlockOre BlockTorch BlockButton"
    " BlockPressurePlate BlockChest BlockFurnace BlockLiquid BlockStaticLiquid BlockDynamicLiquid",
    "mob": "EntityMob EntityCreature EntityAnimal EntityTameable EntityFlying EntityAgeable"
    " EntityGolem EntityWaterMob EntityAmbientCreature EntityZombie EntitySkeleton EntitySpider"
    " EntityHorse AbstractHorse EntityVillager EntityDragon EntityWither EntityBat",
    "entity": "EntityLivingBase EntityThrowable EntityArrow EntityFireball EntityHanging"
    " EntityMinecart EntityBoat EntityItem EntityLiving",
}.items():
    for _s in _subs.split():
        for _pkg in ("block", "entity", "entity/monster", "entity/passive", "entity/projectile",
                     "entity/item", "entity/boss", "entity/effect"):
            BASES.setdefault(f"net/minecraft/{_pkg}/{_s}", _base)
BASES["net/minecraft/entity/EntityLiving"] = "mob"
BASES["net/minecraft/entity/EntityCreature"] = "mob"
BASES["net/minecraft/entity/EntityAgeable"] = "mob"
BASES["net/minecraft/entity/EntityFlying"] = "mob"
INTERFACES = {
    "net/minecraft/util/ITickable": "ticking_tile",
    "net/minecraft/world/gen/IChunkGenerator": "chunk_generator",
    "net/minecraftforge/fml/common/IWorldGenerator": "world_generator",
    "net/minecraft/client/renderer/block/model/IBakedModel": "baked_model",
    "net/minecraftforge/client/model/ICustomModelLoader": "model_loader",
    "net/minecraftforge/common/capabilities/ICapabilityProvider": None,
    "net/minecraftforge/fml/common/network/simpleimpl/IMessage": "message",
    "net/minecraftforge/fml/relauncher/IFMLLoadingPlugin": "coremod",
    "net/minecraft/launchwrapper/IClassTransformer": "transformer",
    "net/minecraft/item/crafting/IRecipe": "recipe",
    "net/minecraftforge/fluids/capability/IFluidHandler": "fluid_handler",
    "net/minecraftforge/items/IItemHandler": "item_handler",
    "net/minecraftforge/energy/IEnergyStorage": "energy_storage",
}
# Constant-pool references worth counting (class -> label).
REFS = {
    "net/minecraftforge/items/CapabilityItemHandler": "cap_items",
    "net/minecraftforge/fluids/capability/CapabilityFluidHandler": "cap_fluids",
    "net/minecraftforge/energy/CapabilityEnergy": "cap_energy",
    "net/minecraftforge/common/capabilities/CapabilityManager": "cap_own",
    "org/lwjgl/opengl/GL11": "gl11",
    "org/lwjgl/opengl/GL20": "gl_shaders",
    "org/lwjgl/opengl/GL30": "gl30",
    "org/lwjgl/opengl/OpenGlHelper": "gl_helper",
    "net/minecraft/client/renderer/OpenGlHelper": "gl_helper",
    "net/minecraft/client/renderer/GlStateManager": "glstate",
    "net/minecraft/client/renderer/Tessellator": "tessellator",
    "net/minecraftforge/client/model/obj/OBJLoader": "obj_models",
    "net/minecraftforge/client/model/b3d/B3DLoader": "b3d_models",
    "net/minecraftforge/fml/common/network/simpleimpl/SimpleNetworkWrapper": "simple_net",
    "net/minecraftforge/fml/common/network/FMLEventChannel": "raw_net",
    "net/minecraftforge/oredict/OreDictionary": "oredict",
    "net/minecraftforge/common/DimensionManager": "dimensions",
    "net/minecraftforge/fml/common/registry/GameRegistry": "game_registry",
    "net/minecraftforge/common/ForgeChunkManager": "chunk_loading",
    "net/minecraftforge/fml/common/network/IGuiHandler": "gui_handler",
    "crafttweaker/annotations/ZenRegister": "zenscript",
    "stanhebben/zenscript/annotations/ZenClass": "zenscript",
}
SUBSCRIBE = "Lnet/minecraftforge/fml/common/eventhandler/SubscribeEvent;"
CAP_INJECT = "Lnet/minecraftforge/common/capabilities/CapabilityInject;"


def parse_class(data):
    """(name, super, interfaces, referenced classes, [(annotation types, descriptor)], field annotations)."""
    if data[:4] != b"\xca\xfe\xba\xbe":
        return None
    pos = 10
    count = struct.unpack_from(">H", data, 8)[0]
    pool = [None] * count
    i = 1
    while i < count:
        tag = data[pos]
        pos += 1
        if tag == 1:
            n = struct.unpack_from(">H", data, pos)[0]
            pool[i] = data[pos + 2 : pos + 2 + n].decode("utf-8", "replace")
            pos += 2 + n
        elif tag in (3, 4):
            pos += 4
        elif tag in (5, 6):
            pos += 8
            i += 1
        elif tag in (7, 8, 16, 19, 20):
            pool[i] = ("ref", tag, struct.unpack_from(">H", data, pos)[0])
            pos += 2
        elif tag in (9, 10, 11, 12, 17, 18):
            pos += 4
        elif tag == 15:
            pos += 3
        else:
            return None
        i += 1

    def cls(idx):
        e = pool[idx] if idx else None
        return pool[e[2]] if isinstance(e, tuple) else None

    access, this, sup, nif = struct.unpack_from(">HHHH", data, pos)
    pos += 8
    ifaces = [cls(struct.unpack_from(">H", data, pos + 2 * k)[0]) for k in range(nif)]
    pos += 2 * nif
    refs = {pool[e[2]] for e in pool if isinstance(e, tuple) and e[1] == 7}

    def annotations(attr_data):
        names = []
        n = struct.unpack_from(">H", attr_data, 0)[0]
        p = 2
        for _ in range(n):
            t, p = struct.unpack_from(">H", attr_data, p)[0], p + 2
            names.append(pool[t])
            p = skip_pairs(attr_data, p)
        return names

    def skip_pairs(d, p):
        pairs = struct.unpack_from(">H", d, p)[0]
        p += 2
        for _ in range(pairs):
            p = skip_value(d, p + 2)
        return p

    def skip_value(d, p):
        tag = chr(d[p])
        p += 1
        if tag in "BCDFIJSZsc":
            return p + 2
        if tag == "e":
            return p + 4
        if tag == "@":
            return skip_pairs(d, p + 2)
        if tag == "[":
            n = struct.unpack_from(">H", d, p)[0]
            p += 2
            for _ in range(n):
                p = skip_value(d, p)
            return p
        raise ValueError(tag)

    def members():
        nonlocal pos
        out = []
        n = struct.unpack_from(">H", data, pos)[0]
        pos += 2
        for _ in range(n):
            _, _, desc, na = struct.unpack_from(">HHHH", data, pos)
            pos += 8
            ann = []
            for _ in range(na):
                an, ln = struct.unpack_from(">HI", data, pos)
                pos += 6
                if pool[an] == "RuntimeVisibleAnnotations":
                    try:
                        ann = annotations(data[pos : pos + ln])
                    except (ValueError, struct.error, IndexError):
                        pass
                pos += ln
            out.append((ann, pool[desc]))
        return out

    fields = members()
    methods = members()
    return cls(this), cls(sup), ifaces, refs, methods, fields


def first_param(desc):
    if desc and desc.startswith("(L"):
        return desc[2 : desc.index(";")]
    return None


def mod_name(z, jar):
    try:
        info = json.loads(z.read("mcmod.info").decode("utf-8", "replace"))
        if isinstance(info, dict):
            info = info.get("modList", [])
        if info:
            return info[0].get("modid") or jar
    except (KeyError, ValueError, AttributeError, IndexError):
        pass
    return PurePosixPath(jar).stem


def scan(folder):
    classes = {}  # name -> (mod, super, ifaces)
    per_mod = collections.defaultdict(collections.Counter)
    events = collections.Counter()
    event_mods = collections.defaultdict(set)
    import os

    for jar in sorted(os.listdir(folder)):
        if not jar.endswith(".jar"):
            continue
        try:
            z = zipfile.ZipFile(os.path.join(folder, jar))
        except zipfile.BadZipFile:
            continue
        mod = mod_name(z, jar)
        c = per_mod[mod]
        c["jars"] += 1
        try:
            mf = z.read("META-INF/MANIFEST.MF").decode("utf-8", "replace")
        except KeyError:
            mf = ""
        if "FMLCorePlugin" in mf:
            c["coremod_manifest"] += 1
        if "MixinConfigs" in mf or "TweakClass" in mf:
            c["mixins_or_tweaker"] += 1
        for entry in z.namelist():
            if entry.startswith("assets/") and "/blockstates/" in entry and entry.endswith(".json"):
                c["blockstates"] += 1
                try:
                    if b"forge_marker" in z.read(entry):
                        c["forge_blockstates"] += 1
                except KeyError:
                    pass
            elif entry.endswith((".obj", ".b3d")) and entry.startswith("assets/"):
                c["obj_b3d_files"] += 1
            elif entry.endswith(".json") and "mixins" in PurePosixPath(entry).name:
                c["mixin_configs"] += 1
            if not entry.endswith(".class"):
                continue
            try:
                parsed = parse_class(z.read(entry))
            except (struct.error, IndexError, KeyError):
                parsed = None
            if not parsed:
                continue
            name, sup, ifaces, refs, methods, fields = parsed
            classes[name] = (mod, sup, ifaces)
            c["classes"] += 1
            for r in refs:
                if r in REFS:
                    c[REFS[r]] += 1
            for ann, desc in methods:
                if SUBSCRIBE in ann:
                    ev = first_param(desc)
                    if ev:
                        events[ev] += 1
                        event_mods[ev].add(mod)
                        c["event_handlers"] += 1
            for ann, _ in fields:
                if CAP_INJECT in ann:
                    c["cap_inject"] += 1

    def kinds(name, seen=None):
        """Kinds of a class through its whole hierarchy (classes of every jar)."""
        seen = seen or set()
        out = set()
        while name and name not in seen:
            seen.add(name)
            if name in BASES and BASES[name]:
                out.add(BASES[name])
            if name in INTERFACES and INTERFACES[name]:
                out.add(INTERFACES[name])
            entry = classes.get(name)
            if not entry:
                break
            for i in entry[2]:
                out |= kinds(i, seen)
            name = entry[1]
        return out

    for name, (mod, _, _) in classes.items():
        for k in kinds(name):
            per_mod[mod]["k_" + k] += 1
    return per_mod, events, event_mods


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("folder")
    ap.add_argument("--tsv")
    ap.add_argument("--events", type=int, default=40)
    a = ap.parse_args()
    per_mod, events, event_mods = scan(a.folder)
    total = collections.Counter()
    for c in per_mod.values():
        total.update(c)
    print(f"{len(per_mod)} mods, {total['jars']} jars, {total['classes']} classes")
    print("\n## Totals (classes of each kind; k_ = through the hierarchy)")
    for k, v in sorted(total.items(), key=lambda kv: -kv[1]):
        if k not in ("jars", "classes"):
            mods = sum(1 for c in per_mod.values() if c[k])
            print(f"{k:22} {v:7}  in {mods} mods")
    print(f"\n## Forge events: {len(events)} types, {sum(events.values())} handlers; top {a.events}")
    for ev, n in events.most_common(a.events):
        print(f"{n:5} {len(event_mods[ev]):4} mods  {ev.split('/')[-1].replace('$', '.')}")
    print("\n## Mods with the most of each kind")
    for k in ("k_tile", "k_ticking_tile", "k_gui_container", "k_tesr", "k_mob", "k_dimension",
              "k_chunk_generator", "k_world_generator", "k_baked_model", "coremod_manifest",
              "mixin_configs", "gl11", "cap_own", "k_message"):
        top = sorted(((c[k], m) for m, c in per_mod.items() if c[k]), reverse=True)[:8]
        print(f"{k:20} " + ", ".join(f"{m} {n}" for n, m in top))
    if a.tsv:
        keys = sorted(total)
        with open(a.tsv, "w") as f:
            f.write("mod\t" + "\t".join(keys) + "\n")
            for m in sorted(per_mod):
                f.write(m + "\t" + "\t".join(str(per_mod[m][k]) for k in keys) + "\n")


if __name__ == "__main__":
    sys.exit(main())
