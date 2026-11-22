# Unmodified Forge 1.20.1 mods on Horizon 26.3 (plan, 2026-11-22)

Status: phases 1 and 2 done; Forge's client runs the platform and talks to Horizon (phases 3 and 4, first half).

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

Unlike the Fabric runtime's headless client, Forge's client is a real one while it starts: it
makes its window and runs its own loop, and its start needs GL. So the mixins that stop GL
calls and stand in for the window (`GlStateManagerMixin`, `RenderSystemMixin`, `RawGlMixin`,
`MinecraftMixin`, `InputConstantsMixin`, and `BlockableEventLoopMixin`'s client half) act only
while `Loader120.headless()`: always under Fabric; under Forge once every mod has loaded, when
`ForgeMod120` hides the window and parks the client (`Loader120.goHeadless`,
`MinecraftParkMixin`: no more frames or ticks) before answering Horizon. Its first test in the
game showed why: the HUD drawn from the conversation thread hit Forge's render-thread checks,
and the client ticking the platform's level crashed it.

`ForgeLoader120` so far: server, level, player, chunk and tick events posted on Forge's bus;
`BreakEvent` and `EntityInteract(Specific)` for breaking and entity use; the item handler
capability for hoppers; the client's own resources and baked models; each mod file's pack.
**Phase 4 (2026-11-22, first half):** `horizon-jvm-remote`'s `forge120::start` starts Forge's client as
its installer's arguments do (`child::Config::main`: the game's own main class in place of
`remote.Child`), with the platform in `<game>/mods`, an empty asset index and Forge's early
window off. Once every mod has loaded (`FMLLoadCompleteEvent`), `ForgeMod120` hides the window
and connects back (`Child.serveInGame`, told where by `HORIZON_REMOTE_CONNECT`);
`ForgeRuntime120` answers as Boot120 does: `start` (the mods Forge loaded), `facts` (Content120),
and every Mirror120 call by name. Checked end to end with `examples/forge120.rs`: the report,
a ping, and the content facts of Forge's registries (16 MB); with Farmer's Delight for Forge
(1.20.1-1.3.4, from Modrinth) Forge loads it and the facts hold its 132 blocks and 185 items.

**In the game:** Forge's 1.20.1 mods go in `mods-java-forge-1.20.1/` beside `options.txt`, Fabric's
stay in `mods-java-1.20.1/`; each folder with jars gets its runtime, both at once. Each has its
cache (`java-120/`, `java-120-forge/`: the content, its game folder), its content made by the
same carry-over (told the instance is Forge's: `Runtime120::loader`; Forge's Jar-in-Jar,
`META-INF/jarjar/`, read too), and the Mods screen lists them under "Fabric 1.20.1" and
"Forge 1.20.1".

The game's calls go to the runtime whose mods they're about (`java_mods_120::route`, a
thread's choice of `RUNTIMES`): a block's by its namespace (using, breaking, exploding,
hoppers, shapes and signals, neighbours, block entities ticked and drawn, blocks drawn by
code, fluids), an item's by its id (using it, its tooltip, its effects), a mob's by its id (the
Forge runtime's from `Entities120.FORGE_FIRST_ID`, the upper half of the mods' range), a
command, a settings screen or a screen by the runtime it came from, a feature, structure or
set by its namespace (a structure start's handle marked as the Forge runtime's). What every
mod hears goes to both: ticks, players joining, leaving, dying and being hurt, vanilla mobs
dying and being hurt, keys. What both give is put together: the HUD's operations, the mobs
sent to the players' games (a runtime that skipped a tick sends its last), commands, key
bindings, settings screens, fluids' looks, recipes, block entity types, loot added, worldgen
packs. Each runtime knows only its own mods' blocks: the other runtime's are strangers to it,
stood in for by their shape as 26.3's own new blocks are.

Mods' packets go between the halves through Forge's own hand-off (NetworkHooks.onCustomPayload),
with every channel taken as present on the client (NetworkInstanceMixin): the client is this same
game. The window is hidden from creation and the client parks once the mods' models are baked.

Models drawn by code: a mod's state whose baked model is a class of its own (not vanilla's or
Forge's) is drawn where it stands, from its quads with Forge's model data (as for Fabric, only
those with a block entity are drawn again as they change).

Biome modifiers: the mirror's registries are loaded with Forge's data pack registries, and the
mods' biome modifiers applied to each biome's modifiable info as a server's start applies them
(Farmer's Delight: 9 modifiers, its wild crops in 64 biomes).

Mod key mappings are the game's but vanilla's own (the options' fields). Tooltips
(ItemTooltipEvent), Horizon's mobs dying (LivingDeathEvent) and being hurt (LivingAttackEvent,
LivingHurtEvent: cancelled, not hurt) and the mods' settings screens (their ConfigScreenFactory)
go through the loader seam like the rest.

A block that is neither runtime's (vanilla's, 26.3's) breaking is heard by both runtimes' mods,
and either can stop it; a runtime's own block goes to that runtime only.

The mods' mixins on vanilla behaviour Horizon runs itself are indexed from each mod file's
`MixinConfigs` (its manifest), srg method names read as Mojang's from a table build120forge.sh
makes into the jar (Farmer's Delight's `Rabbit.isFood`, `CampfireBlock.isSmokeSource`). Mod
fluids' looks come from Forge's client fluid type extensions. Tooltip images need nothing: Forge's
`ClientTooltipComponent.create` asks its own registry. Layers on renderers (AddLayers) were posted
by Forge's own client as it loaded.

Still to do: testing with more Forge mods (Create, JEI, Waystones); hoppers reach only Forge's item
handlers (not its fluid or energy ones).

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
