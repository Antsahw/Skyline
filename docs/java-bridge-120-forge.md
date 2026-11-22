# Unmodified Forge 1.20.1 mods on Horizon 26.3 (plan, 2026-11-22)

Status: phase 1 done, phase 2 started.

**Phase 1 (2026-11-22):** `tools/java_bridge/fetch120forge.py --boot` fetched Forge 47.4.28 (its
recommended 1.20.1 build), installed the client over fetch120.py's Mojang files (81 class path
entries, none missing; `cpw.mods.bootstraplauncher.BootstrapLauncher`, launch target
`forgeclient`), and booted a dedicated server with no mods to "Done" in 2 s, on Java 21.

**Phase 2:** `Loader120` is the seam, found once by which loader's classes exist;
`fabric/FabricLoader120` is today's Fabric API calls behind it. In so far: using an entity
(`UseEntityCallback`) and breaking blocks (`PlayerBlockBreakEvents`). Still direct: Events120,
Packets120, Hoppers120, Keys120, Models120, Hud120 and the three mixins on Fabric API's classes;
the reflective calls (entity events, tooltips, fluid renderers, dynamic registries) already
fall back when Fabric isn't there.

The 1.20.1 Fabric bridge ([java-bridge-120.md](java-bridge-120.md)) runs Fabric mods in a JVM
of its own, with a mirror of Horizon's world. Forge 1.20.1 mods get **a JVM of their own too**,
beside it, not inside it: no Fabric-on-Forge translator (Sinytra Connector) between the mods and
the bridge, so a bug is in the mod, in Forge, or in our code, never in a fourth layer.

## License

Forge (MinecraftForge) is LGPL-2.1: off limits even to read ([CONTRIBUTING.md](../CONTRIBUTING.md#only-original-code-or-code-whose-license-allows-it)).
Forge is run as it ships, from its installer, the way a launcher runs it; what we write against it
comes from its public documentation (<https://docs.minecraftforge.net/en/1.20.x/>) and from
vanilla 1.20.1. Mixin (MIT) and ModLauncher's public launch contract are fine.

## What carries over unchanged

Everything written against **Minecraft** 1.20.1 rather than against Fabric:

- The mirror (`Mirror120`, `Entities120`, `Worldgen120`, `Light120`, ...): the world, blocks,
  block entities, breaking, explosions, mobs, structures, worldgen features.
- The child JVM and its wire (`horizon-jvm-remote`): the same process model, the same calls.
- Content conversion (`content120.rs`): data packs, worldgen, tags and assets are Minecraft's
  formats; only where a jar keeps its metadata differs.
- The item conversions (1.20.1 NBT up and down).

## What changes

1. **Names at run time.** Fabric 1.20.1 runs on intermediary names, Forge 1.20.1 on SRG. The
   SRG table comes from MCPConfig if its license allows it (to check first); otherwise from
   the installer's own output, read at the user's machine like Mojang's files, never committed. `build120.sh` gains `LOADER=forge`: the same named sources, remapped
   to SRG instead of intermediary, with the class table `Names` reads.
2. **The loader seam.** platform120 calls Fabric API in a few places: tick and lifecycle
   events, networking, Fabric's transfer API (hoppers), key bindings, biome modification, the
   model loading and Indigo renderer hooks, `PlayerBlockBreakEvents`, `ServerLivingEntityEvents`.
   These move behind a small `Loader120` interface, with a Fabric implementation (today's code)
   and a Forge one (Forge's documented events and capabilities: `IItemHandler` for hoppers,
   `BlockEvent.BreakEvent`, `TickEvent`, `SimpleChannel`, `RegisterKeyMappingsEvent`).
3. **Launch.** `fetch120.py` gains Forge pins: the installer, run once headless to produce the
   patched client and its libraries (its processors), and the launch arguments it writes
   (ModLauncher's `BootstrapLauncher`, the module path). The child starts that instead of Knot.
4. **The platform as a Forge mod.** platform120 ships a `META-INF/mods.toml` and its mixin
   config, as it ships `fabric.mod.json` now; its 62 mixins run on Forge's bundled Mixin.
5. **Reading jars.** `mods.toml` for ids, versions and dependencies; jar-in-jar under
   `META-INF/jarjar/`; the scanner tells Forge jars from Fabric ones and sends each to its JVM.

## Phases

1. Fetch and launch: Forge 1.20.1 boots hidden in its own JVM with no mods (phase 1 of the
   Fabric bridge, again).
2. The loader seam in platform120, Fabric side only: no change in behaviour, the tests pass.
3. The SRG build and the Forge side of the seam; platform120 loads as a Forge mod.
4. The scanner and two JVMs at once: Fabric jars to one, Forge jars to the other.
5. The first tracker: CurseForge's most downloaded Forge 1.20.1 mods, as for Fabric.
