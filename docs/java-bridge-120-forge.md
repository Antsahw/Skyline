# Unmodified Forge 1.20.1 mods on Horizon 26.3 (plan, 2026-11-22)

Status: phases 1 and 2 done; phase 3's platform loads in Forge's client.

**Phase 1 (2026-11-22):** `tools/java_bridge/fetch120forge.py --boot` fetched Forge 47.4.28 (its
recommended 1.20.1 build), installed the client over fetch120.py's Mojang files (81 class path
entries, none missing; `cpw.mods.bootstraplauncher.BootstrapLauncher`, launch target
`forgeclient`), and booted a dedicated server with no mods to "Done" in 2 s, on Java 21.

**Phase 2 (2026-11-22):** `Loader120` is the seam, found once per JVM by which loader's classes
exist; `FabricLoader120` is Fabric's side. Behind it: server events (start, levels, players,
chunks, ticks), client ticks and the mods' key mappings, entity use, block breaking, hoppers on
the mods' storages, packets between the headless client and the server half, the mods' resource
and data packs, biome modifications, client models, and the bridge's own probes. Events120,
Hoppers120, Packets120 and Models120 are Fabric's modules now, reached only through
`FabricLoader120`. The rest of the platform names no Fabric class: what is left reaches Fabric by
name and falls back without it (entity death and damage events, tooltips, fluid renderers,
dynamic registries, Config120's settings screens, Mixins120's index of the mods' mixins). Three
mixins target Fabric API's own classes (`ClientPlayNetworkingMixin`, `ServerPlayNetworkingMixin`,
`PlayerLookupMixin`): the Forge build leaves them out of its mixin config (phase 3).

**Phase 3 (2026-11-22, started):** `tools/java_bridge/build120forge.sh` builds the same sources plus
`platform120/forge` (`ForgeLoader120`, the `@Mod` class `ForgeMod120`, `META-INF/mods.toml`)
into `platform120-forge.jar`: compiled against Forge's patched client, the rest of the client
and Forge itself, remapped from srg to Mojang's names; then remapped to srg, Forge's runtime
names (Mojang's class names, SRG member names), from a fourth namespace `Remap120 srg` adds to
build120.sh's mappings out of the installer's mcp_config. Forge's client (`forgeclient`, as the
installer's launch arguments start it) loads it beside its own mod: "Horizon platform loaded
under Forge (ForgeLoader120)", the platform's mixins applied, and the client runs on to its
title screen (checked under Xvfb).

Unlike the Fabric runtime's headless client, Forge's client is a real one: it makes its window
(to be hidden) and runs its own loop, and its start needs GL. So the Forge jar leaves out the
mixins that stop GL calls and stand in for the window (`GlStateManagerMixin`,
`RenderSystemMixin`, `RawGlMixin`, `MinecraftMixin`, `InputConstantsMixin`), and
`BlockableEventLoopMixin` runs work at once only for the mirror's server
(`Loader120.clientRunsItsLoop`).

`ForgeLoader120` so far: server, level, player, chunk and tick events posted on Forge's bus;
`BreakEvent` and `EntityInteract(Specific)` for breaking and entity use; the item handler
capability for hoppers; the client's own resources and baked models; each mod file's pack.
Still to do: the bridge's connection to Horizon from the Forge JVM (phase 4), packets to the
client half, models drawn by code, biome modifiers on the mirror's registries, mod key mappings
by registration rather than by category, hiding the window.

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
