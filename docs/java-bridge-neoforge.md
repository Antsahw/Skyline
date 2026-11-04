# NeoForge 26.3 in the Java bridge (research, 2026-11-04, `bridge-neoforge-research`)

How NeoForge mods could run inside Horizon the way Fabric mods do ([java-bridge.md](java-bridge.md)),
what is different from Fabric, the plan, and what already runs.

**Short answer: it fits the bridge well.** NeoForge 26.3 uses Mojang's names (26.x jars aren't
obfuscated, same as Fabric's), the same Sponge Mixin fork Fabric uses, and a plain class path (no
ModLauncher, no module layers since FML 10+). Almost all of the mirror (`HorizonServer`, block reads,
entities, worldgen, GPU capture) is vanilla code and carries over. What changes: the loader's start,
the events (NeoForge fires most of its own events from its patches inside vanilla methods the
mirror never runs), capabilities, and the networking glue.

**Already runs (prototype):** NeoForge 26.3.0.101 starts in the bridge's JVM the bridge's way
(Java's `NeoBoot` on the system class path, FML's launcher on its own thread). It loads a test mod,
registers its block and item, runs common setup, and hands Horizon the game's class loader. No
dedicated server, eula.txt or world gets made. That's headless, standalone (`run_proto.sh`) and
through the game (`HORIZON_NEOFORGE`, see "Try it").

## The license rule (read first)

NeoForge and FancyModLoader (FML) are **LGPL**. [CONTRIBUTING.md](../CONTRIBUTING.md) puts LGPL
source off limits, *even to read*. The bridge only **runs** their jars as they are, which the LGPL
allows. Everything on this page and in the code comes from:
- NeoForge's public docs (<https://docs.neoforged.net/>),
- the installer's own files (`install_profile.json`, `version.json`, `unix_args.txt`),
- what the loader does when run: its log, stack traces (class and method names, line numbers),
  `getMethods()` on a class at run time, a `@ModifyArg` that prints what vanilla passes.

Never open, decompile or `javap -c` NeoForge's or FML's classes, and don't paste their code into a
chat. Mojang's game code stays as before (the decompiled 26.3 sources in `.local/`).
The NeoForge-patched game jar holds NeoForge's patches, so don't read *its* decompiled source
either: read vanilla's.

## What NeoForge 26.3 is (facts found 2026-11-04)

- Newest build on `maven.neoforged.net`: **26.3.0.101** (26.3's first builds were `26.3.0.x-beta`).
  FML **12.0.12**, bus 8.0.5, `net.fabricmc:sponge-mixin:0.17.4+mixin.0.8.7` (the bridge's Fabric
  side has 0.17.5), MixinExtras 0.5.5 inside NeoForge's jar. Java 25.
- **The installer patches the player's own jar.** Its processor
  (`installertools … --task PROCESS_MINECRAFT_JAR --apply-patches data/client.lzma`) turns Mojang's
  jar into `libraries/net/neoforged/minecraft-{client,server}-patched/<ver>/…jar` (binary patches,
  ~6 s). Nothing of Mojang's is shipped, same as Fabric. `--installServer DIR` downloads the server
  jar from Mojang; `--installClient DIR` needs a launcher folder (`launcher_profiles.json`,
  `versions/26.3/`).
- **Launch** (`unix_args.txt`): `java --add-opens java.base/java.lang.invoke=ALL-UNNAMED
  -DlibraryDirectory=libraries -cp <FML + game libraries> net.neoforged.fml.startup.Server
  --fml.neoForgeVersion 26.3.0.101 --fml.mcVersion 26.3 --fml.neoFormVersion 1`
  (`…startup.Client` for the client). FML finds the patched jar through `libraryDirectory` (an
  absolute path if the working folder differs), mods in `<gameDir>/mods`, and accepts `--gameDir`.
- The log's "Building game content classloader" lists every jar of the game layer: `minecraft`
  (the patched jar), `neoforge` (the universal jar), `mixinextras.neoforge`, the mods. Game classes
  live in `net.neoforged.fml.classloading.transformation.TransformingClassLoader`.
- The loader closes when its launcher returns ("Closing FML Loader", "Clearing ModLoader"): the
  launcher's thread must stay alive while the game runs.

### How a NeoForge game starts (seen at run time)

Dedicated server (`fml.startup.Server.main`):
1. FML: early services, mod discovery, Jar-in-Jar, Mixin configs, coremods ("FML Plugins"), the
   game's class loader.
2. The patched `net.minecraft.server.Main.main`: arguments, eula.txt, server.properties, then
   **`net.neoforged.neoforge.server.loading.ServerModLoader.load(false)`** (Main.java:117 in
   26.3.0.101; vanilla passes `false`). It runs `CommonModLoader.begin` (mods constructed in
   parallel on `modloading-worker-*`, their `@Mod` constructors taking `(IEventBus modBus,
   ModContainer)`), registration, and `CommonModLoader.load` (`FMLCommonSetupEvent`).
3. Vanilla's server starts. NeoForge's game events (`ServerTickEvent.Pre/Post`, `LevelTickEvent`,
   …) fire from NeoForge's patches inside vanilla's methods.

Client (`fml.startup.Client.main`):
1. The early loading window (`earlydisplay`) needs OpenGL. Headless it fails ("Couldn't find
   matching GLX visual") unless `config/fml.toml` has `earlyWindowControl = false`.
2. The patched `net.minecraft.client.main.Main.main` calls
   **`ClientModLoader.begin`** (Main.java:167): mods constructed, before any window.
3. `new Minecraft(...)` (Main.java:280) makes the window and GPU device, then calls
   **`ClientModLoader.finish`** (Minecraft.java:729): common setup, client setup, and the rest,
   before the first resource reload.

## Fabric and NeoForge, side by side

| | Fabric (the bridge today) | NeoForge 26.3 | What the bridge does for NeoForge |
|---|---|---|---|
| Loader start | Knot + `HorizonGameProvider` (Apache-2.0: read freely) | FML's launcher → patched vanilla `Main.main` | Run FML's launcher on a thread; a platform mixin on `Main.main` takes over (done, server side) |
| Game jar | Vanilla jar, mixins only | Vanilla + NeoForge's binary patches (made on the player's machine) | Run the installer's processor once into `.local/bridge/neoforge/` (to do) |
| Mod metadata | `fabric.mod.json`, entrypoints | `META-INF/neoforge.mods.toml`, `@Mod` class; mixin configs as `[[mixins]] config=…` | FML reads them; the content build must read `neoforge.mods.toml` for ids, names, deps |
| Events | Fabric API callbacks (`ServerTickEvents`…); the mirror fires them (`HorizonServer.fire`) | Two buses: the mod bus (lifecycle, registration: `RegisterEvent`, `FMLCommonSetupEvent`, `RegisterCapabilitiesEvent`, `RegisterPayloadHandlersEvent`, client `EntityRenderersEvent`, `RegisterMenuScreensEvent`…) and `NeoForge.EVENT_BUS` (game: ticks, player, block, entity events) | Game events fired **by NeoForge's patches** in vanilla methods the mirror never runs (`MinecraftServer.tickServer`, `ServerLevel.tick`…) must be posted by the mirror, as `HorizonServer.fire` does for Fabric. Events in methods the mirror runs (a mob's `tick`, `Block.useItemOn`) fire by themselves |
| Registries | Vanilla's, unfrozen by Fabric API during init | `DeferredRegister` → `RegisterEvent`, then frozen; vanilla registries carry NeoForge's extras | Same mirror; the content export (`HorizonContent`) reads `BuiltInRegistries` after load: works as is (neohello's block and item seen there) |
| Data maps | none (Fabric registries and vanilla tags) | `data_maps/` JSON (fuels, compostables, …) attached to registries | Export them with the content (Horizon's furnaces and composters read fuel and compost values in Rust) |
| Capabilities | Fabric Transfer API (`ItemStorage.SIDED`) | `BlockCapability`/`EntityCapability`/`ItemCapability`, providers in `RegisterCapabilitiesEvent`, queried with `level.getCapability(cap, pos, side)`; built-ins `Capabilities.Item.BLOCK` (`ResourceHandler<ItemResource>`), `Capabilities.Fluid.*`, `Capabilities.Energy.*` | Mods' pipes on the mirror ask the mirror's level: works if `getBlockEntity` answers (`COPIES`, `LIVE`). Horizon's Rust hoppers into a mod's block need one call into Java (`Capabilities.Item.BLOCK` at a position and side); [native-hooks-neoforge.md](native-hooks-neoforge.md) already names this |
| Data attachments | Fabric's attachments API | `AttachmentType`s on block entities, entities, chunks, levels; saved in their NBT | Saved with the mirror's own block entity and entity files; chunk/level attachments need a store |
| Networking | `PayloadTypeRegistry`, `ServerPlayNetworking`; `HorizonLoop` via mixins | `RegisterPayloadHandlersEvent` → `PayloadRegistrar`; `PacketDistributor.sendToPlayer`; `IPayloadContext` | A NeoForge `HorizonLoop`: catch sends to mirrored players, deliver to the headless client's handlers, and back |
| Client registration | `ClientModInitializer`, `ScreenRegistry`, `EntityRendererRegistry` | Mod-bus events fired inside `ClientModLoader.finish` and the first resource reload | The headless client (made without `Minecraft`'s constructor) calls the loading stages itself (the part to work out) |
| Mixins | Sponge Mixin (Fabric's fork) | The same fork, plus coremods (`ITransformer` services) and access transformers | The platform's vanilla-targeting mixins mostly carry over (same names). Coremods and ATs just run |
| Jar-in-Jar | `META-INF/jars/` | `META-INF/jarjar/metadata.json` | FML handles loading; the content build's `nested_jars` must read the jarjar list |

## What carries over, what doesn't

- **Loader-neutral (vanilla API, same names on both):** the mirror (`HorizonServer.setUp`, levels made
  without constructors, `HorizonBlockEntities`, `HorizonEntities`, `HorizonTicks`, `HorizonLight`,
  `HorizonWorldgen`, `HorizonChunks`), `HorizonGraphics`, `HorizonGpu`, `HorizonEntityRender`,
  `HorizonContent`'s exporters, every Rust part (`events.rs`, `java_server.rs`, `java_screen.rs`).
- **Fabric-only:** `FabricBoot`, `HorizonGameProvider`, the events fired by name from Fabric API
  (`HorizonServer.fire`), `PlayNetworkingMixin`/`ClientPlayNetworkingMixin`, `HorizonScreenEvents`
  (Fabric's screen events), Mod Menu's config screens.
- **NeoForge patches the same vanilla classes the platform patches.** A platform mixin whose target
  NeoForge changed may miss its injection point. Each must be checked against the NeoForge game.
  Mixin says which ones failed ("failed injection check"); `defaultRequire: 1` makes that loud.

## Plan

0. **Done (this branch):** FML starts inside the bridge's JVM, dedicated-server half: mods
   constructed and registered, common setup, the game's class loader handed to Horizon
   (`NeoBoot`, `horizon-jvm/src/neoforge.rs`, `HORIZON_NEOFORGE`).
1. **Split the platform** (`java/platform/`) into a loader-neutral core, Fabric glue and NeoForge
   glue. Build the NeoForge glue on the player's machine against their patched jar, as
   `platform.rs` builds the Fabric platform. Make its `neoforge.mods.toml` and mixin config.
2. **The mirror on NeoForge (server half):** `HorizonServer.setUp` on NeoForge's loader, game
   events posted where the mirror skips vanilla (`ServerTickEvent.Pre/Post`,
   `LevelTickEvent.Pre/Post`, `ServerStartingEvent`/`ServerStartedEvent`,
   `PlayerEvent.PlayerLoggedInEvent`, `RegisterCommandsEvent`), the content build reading
   `neoforge.mods.toml`. Test: neohello's block given, placed and saved in Horizon (regress case).
3. **Installing NeoForge from Horizon:** pin the installer (SHA-256, like `fabric-libs.tsv`), run
   it (or its `installertools` processor) on the player's own 26.3 jar into
   `.local/bridge/neoforge/<ver>/`, once. Server half first, then the client jar.
4. **The client half:** FML's client launcher, with the platform mixin on the client `Main.main`
   after `ClientModLoader.begin`. The headless client is allocated as today, and the bridge runs
   what `Minecraft`'s constructor would (`ClientModLoader.finish`, the first reload's model and
   atlas steps). Then client events (`RegisterMenuScreensEvent`, `EntityRenderersEvent`,
   `RegisterKeyMappingsEvent`, `RegisterGuiLayersEvent`) feed the existing screen, HUD and
   renderer capture. `earlyWindowControl = false`.
5. **Capabilities and transfer:** `Capabilities.Item.BLOCK` from Horizon's hoppers and containers
   (Rust → Java at a position and side), vanilla containers as capabilities (NeoForge already
   gives vanilla's blocks theirs on the mirror's `COPIES`), fluids and energy for machines.
6. **Real mods,** smallest first: JEI (NeoForge build), Farmer's Delight (NeoForge is its main
   line), then Create (NeoForge first), Mekanism, AE2. Many big mods are NeoForge-first or
   NeoForge-only: that's the payoff.

Fabric and NeoForge in one JVM (both at once) is not planned: one loader a game.

## Risks

- `ServerModLoader.load(boolean)` and `ClientModLoader.begin/finish` are NeoForge internals
  (`…neoforge.server.loading`, `…client.loading`), not API: they can change in any build. Pin the
  NeoForge version (as Fabric's is). The platform finds `load` by name at run time and says what
  it found if the shape changes, rather than guessing.
- The LGPL rule makes debugging slower: only docs, logs and stack traces. A stack trace into
  NeoForge is fine to read. Its source isn't.
- NeoForge's patches inside vanilla methods the mirror *does* run will fire events with mirror
  objects (levels made without constructors). Mods' handlers may reach fields the mirror never
  set. Same class of bug as with Fabric, more of it.
- FML sets up Log4j itself (its own config) and parks a thread for good. The bridge's
  `log4j2-horizon.xml` must still win, so mods' lines reach Horizon's log.

## 26.3 traps found

- **FML closes its loader when its launcher returns.** Taking over from vanilla's main and
  returning ends it ("Closing FML Loader"): park the launcher's thread (`NeoBoot.ready`).
- **Vanilla's server main reads eula.txt and server.properties before the mods load**, from the
  working folder. Take over at `Main.main`'s head and run version detection, `Bootstrap.bootStrap`
  and `ServerModLoader.load(false)` yourself.
- **Vanilla's server main refuses `--gameDir`** ("gameDir is not a recognized option"), which FML
  needs: the platform must not pass FML's arguments on to vanilla.
- **`-DlibraryDirectory=libraries` is relative to the working folder.** Elsewhere FML says "Your
  NeoForge installation is corrupted". Give it the absolute path.
- **The client's early window needs OpenGL**: `config/fml.toml` `earlyWindowControl = false`
  headless.
- **FML's server launcher waits for vanilla's "Server thread"** and fails with "Couldn't find
  Minecraft server thread" when the main returns without one: another reason to park.
- `@At(INVOKE)` on `ServerModLoader.load` with `()V` scanned no targets: it takes a `boolean`.
  Match by owner and name when the descriptor is unknown, and print `getMethods()` to learn it.

## Try it

```sh
# 1. NeoForge's own installer, into a scratch folder (downloads Mojang's server jar).
curl -LO https://maven.neoforged.net/releases/net/neoforged/neoforge/26.3.0.101/neoforge-26.3.0.101-installer.jar
java -jar neoforge-26.3.0.101-installer.jar --installServer ~/nf/server
# 2. The test mod (a block, an item, setup and tick listeners), built against that install.
tools/java_bridge/neoforge/build_fixture.sh ~/nf/server ~/nf/server/mods/neohello.jar
# 3. Standalone, as the bridge starts it: builds the platform mod, loads, prints what registered.
tools/java_bridge/neoforge/run_proto.sh ~/nf/server
#    [horizon-neo] NeoForge loaded 4 mods in 14313 ms
#    [horizon-neo] block neohello:hello_block registered: true, item: true
# 4. Through Horizon's game (after step 3 built the platform mod): the mods in mods-java/ load
#    with NeoForge instead of Fabric; the log says which. Nothing else runs on them yet.
mkdir -p ~/nfjar && cp ~/nf/server/mods/neohello.jar ~/nfjar/
HORIZON_NEOFORGE=~/nf/server MODS=~/nfjar SCENE=none tools/java_bridge/screen_test.sh
#    Java mods: JVM 25… and NeoForge started in … ms; loaded minecraft 26.3, neohello 1.0.0, …
```

The fixture logs who drives each loading stage with `-Dneohello.stack=true` (the main and render
threads' stacks at construction and common setup): how the start sequence above was found.

Files: `crates/horizon-jvm/java/dev/horizonlab/bridge/neo/NeoBoot.java`,
`crates/horizon-jvm/src/neoforge.rs`, `crates/horizon-client/src/game/java_mods.rs`
(`start_neoforge`), `crates/horizon-jvm/java/neoforge/` (the fixture `neohello`, the prototype platform mod), `tools/java_bridge/neoforge/` (scripts).
