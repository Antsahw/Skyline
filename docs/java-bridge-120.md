# Unmodified Fabric 1.20.1 mods on Horizon 26.3

Plan and status of the bridge's 1.20.1 runtime. Based on `iaxivers/horizon-java-bridge`, branch
`java-bridge`, modeled on [java-bridge-112.md](java-bridge-112.md) (Forge 1.12 via Cleanroom).
Read [java-bridge.md](java-bridge.md) first.

## The idea

Horizon stays the game. A JVM loads **Mojang's real 1.20.1 jar + Fabric Loader + Fabric API
1.20.1**. It sets up a mirror server, levels and client (no constructors, no game loop), and the
mods run against the real 1.20.1 classes, so every Mixin applies exactly as written. Only *world
state* is translated between 1.20.1 and Horizon's 26.3, never the mods' code.

## One JVM per runtime

JNI allows one JVM per process, and the bridge's JVM already runs Fabric 26.3 (or Forge 1.12,
or NeoForge). So **1.20.1 runs in a JVM of its own, in a child process**, and Horizon talks to it
through a socket instead of JNI. That gives:

- **Two `net.minecraft` trees in one game.** Fabric 26.3 mods stay in the bridge's JVM, and
  1.20.1 mods live in theirs. Neither JVM sees the other's classes, so nothing clashes. Later,
  Forge 1.20.1 or NeoForge 1.21.1 become more children of the same kind.
- **Each runtime has its own, best stack.** The 1.20.1 child runs the newest Fabric Loader and
  the newest Fabric API built for 1.20.1, pinned apart from the 26.3 side (`research-lock.json`
  → `fabric120`, not `fabric-libs.tsv`). Either side can be updated without risk to the other.
- **Each runtime chooses its Java.** Java 25 by default (the bridge's own). A pack that hits a
  mod broken on newer Java can run just its child on 17 (on Arch: `jdk17-openjdk` beside
  `jdk-openjdk`, `HORIZON_JAVA_HOME` for that runtime).
- **A crash stays in its process.** A child that dies is an error with its last log lines,
  and Horizon keeps running.

The cost: mods in different runtimes can't call each other's Java code. They meet only through
Horizon's world (blocks, items, entities, containers). An addon must run in the same JVM as the
mod it extends. Each extra JVM also costs a few hundred MB.

## The transport

Crate `crates/horizon-jvm-remote` (Rust, no JNI) and `java/dev/horizonlab/bridge/remote/`
(Java 17+, JDK types only):

| Piece | Rust | Java |
|---|---|---|
| Frames: CALL, REPLY, ERROR, EVENT, HELLO, BYE; values are the types `Natives` passes over JNI (strings for SNBT/JSON, numbers, byte/int/string arrays) | `wire.rs` (layout in its docs) | `Wire.java` |
| Synchronous calls that nest both ways: while one side waits for an answer it handles the other's calls and events, so a mod inside Horizon's `tick` can call `blockState` back, as with JNI on one thread | `peer.rs` | `Peer.java` |
| Start: a socket in a private folder (Unix socket; loopback TCP on Windows or with `HORIZON_REMOTE_TCP=1`), `java ... remote.Child`, a token in the child's environment (not on its command line), the runtime's arguments in the welcome frame (UTF-8: the command line is decoded in the system locale and garbles paths like `/home/Ivér`), stdout/stderr as prefixed log lines | `child.rs` | `Child.java`, `RemoteRuntime.java`, `Host.java` (what `Natives` is, over the socket) |

**Lanes.** One connection is one conversation: one thread on each side at a time (the Java
side's *conversation thread*). Events may come from any thread. A second Java thread that must
call Horizon (the client/render thread, later) gets a connection of its own.

### Measured

Same 2-core box, release builds, warm JITs (`cargo test -p horizon-jvm-remote --release --test bench -- --ignored --nocapture`):

| | Unix socket | Loopback TCP | JNI (bridge today) |
|---|---:|---:|---:|
| Rust → Java call, round trip | 14 µs | 16 µs | 0.46 µs |
| Java → Rust call inside a call (a mod reading the world) | 24 µs | 32 µs | 0.11 µs |
| One-way events (log lines, frames), batched | 10.4 M/s | 10.4 M/s | – |
| Bulk data (4 MB echoed) | 940 MB/s | 885 MB/s | – |

**What this decides.** A socket call costs 30–200× a JNI call. Coarse calls are fine:
`tick`, `useBlock`, menu clicks, joins, and events such as screen frames and item changes. But
the hot reads mods make thousands of times per tick (`blockState`, `light`, `sectionIds`,
`chunkBlocks`) must **not** cross the socket one by one. The child keeps a mirror of the chunk
sections it needs, which Horizon pushes as batched deltas keyed by the chunk revisions
`Natives` already exposes (`chunkRevision`, `viewEpoch`). Reads are then answered inside the
child. For the epoch counters, which `Natives.threadEpochBuffer` already shares as a
`ByteBuffer`, both processes can map the same file (`memfd` / `FileChannel.map`).

## Phase 1 status: Fabric 1.20.1 boots hidden in its own JVM (2026-11-08)

**Runs:** Horizon's side (`boot120::start`) starts the child and the child connects. `Boot120`
then starts Fabric Loader **0.20.1** with its own Minecraft provider, which finds Mojang's
1.20.1 jar and remaps it to intermediary on the first boot, cached in `.fabric/remappedJars`.
Next it loads the mods and Fabric API **0.92.12+1.20.1**, applies their Mixins, bootstraps the
registries as vanilla does, makes a headless client (below), and runs every mod's `main` and
`client` entrypoints **one by one**. A mod that fails is recorded, and the others still run. The
report comes back over the socket.

| Run (2-core cloud box) | Start | Heap after | Mods | Blocks / items | Entrypoints failed |
|---|---:|---:|---:|---:|---:|
| Vanilla + Fabric API, Java 21 | 7 s | 300 MB | 58 | 1003 / 1255 | 0 of 23 |
| + 14 popular mods with their libraries, Java 21 | 11–16 s | 290 MB | 78 | 1628 / 2312 | 0 |
| The same, child on Java 25.0.1 | 10.6 s | – | 78 | 1628 / 2312 | 0 |

The 14: Lithium, Sodium, FerriteCore, Mod Menu, JEI, AppleSkin, Waystones (+ Balm), Farmer's
Delight, Tech Reborn (+ RebornCore, Team Reborn Energy), Cloth Config, Jade, Supplementaries
(+ Moonlight), from Modrinth's latest 1.20.1 Fabric builds.

The report matches facts of vanilla 1.20.1 that were known independently: a diamond sword has
1561 durability, oak stairs have 80 states, glowstone has light 15, there are 64 biomes, the
sniffer egg exists and the crafter (1.20.3) doesn't. `tests/boot120.rs` asserts these.

**The headless client** (`Headless120`, the start of what the 26.3 bridge's `HorizonClient` is).
Client code reaches `Minecraft.getInstance()`. Without it, 5 of the 14 mods failed (Jade, JEI,
Mod Menu, Tech Reborn's client, Moonlight). Now a `Minecraft` is allocated without its
constructor and filled in vanilla's order: game folder, event-loop task queue and an offline
`User` before the entrypoints, then real `Options` after the client entrypoints. That order
matters, because Fabric API refuses key mappings once Options exist. The tasks mods queue with
`execute()` wait until the client ticks (a later phase).

**Mojang's names at run time** (`Names120`). 1.20.1 runs under intermediary names
(`class_2248`). The boot and report code is written with Mojang's names, as the 26.3 platform
is, and looks them up through Mojang's `client.txt` (Mojang → official) and Fabric's own
`MappingResolver` (official → intermediary), walking supertypes. 7436 classes are mapped in
about 200 ms.

**Java 25.** It runs with the two options launchers pass on newer Java
(`--enable-native-access=ALL-UNNAMED` for LWJGL, `--sun-misc-unsafe-memory-access=allow` for
JOML and friends). They're added only for Java ≥ 22 / ≥ 24, read from the Java home's
`release` file, because Java 17 rejects them.

**Try it** (on Arch, `jdk-openjdk` is Java 25):

```sh
python3 -I tools/java_bridge/fetch120.py                 # ~92 MB into ~/.cache/horizon-120 (pinned, hash-checked)
tools/java_bridge/run120.sh ~/mods-1.20.1 report.json    # standalone: boot + report + summary
cargo test -p horizon-jvm-remote                         # transport, child, and the boot through Rust (skips without the cache)
HORIZON_120_MODS=~/mods-1.20.1 cargo test -p horizon-jvm-remote --test boot120 -- --nocapture
```

**Files:**

| Piece | Files |
|---|---|
| Transport (Rust) | `crates/horizon-jvm-remote/src/{wire,peer,child}.rs`; tests `tests/child.rs` (a real Java child over Unix and TCP: nesting, events, errors, a crash), `tests/bench.rs` |
| Transport (Java) | `java/dev/horizonlab/bridge/remote/`: `Wire`, `Peer`, `Child`, `RemoteRuntime`, `Host` |
| 1.20.1 start (Rust) | `crates/horizon-jvm-remote/src/boot120.rs` (`launch.json`, arguments, Java choice and options, `start`, `Runtime120::call`); `tests/boot120.rs` |
| 1.20.1 boot (Java, JDK types only, Fabric by reflection) | `java/dev/horizonlab/bridge/r120/`: `Boot120`, `Headless120`, `Names120`, `Report120`, `Api`, `Json` |
| Scripts, pins | `tools/java_bridge/fetch120.py` (+ `tests/test_fetch120.py`), `tools/java_bridge/run120.sh`; `research-lock.json` → `fabric120` |

## Phase 1b status: older-loader mods, the scanner, the first tracker (2026-11-09)

**Mods held back only by their loader range now run.** Before Fabric Loader resolves the mods,
`Overrides120` reads every mod's `fabric.mod.json`, nested jars included, and tests its
`fabricloader` range with Fabric's own public `VersionPredicate`, so a range means exactly what
it means to Fabric. Each mod the running loader falls outside of gets an entry in Fabric's own
dependency override file (`config/fabric_loader_dependencies.json`, `"-depends":
{"fabricloader": "*"}`). Nothing else about the mod changes, and its jar stays as it is. The
report lists each override (`dependency_overrides`), and `run120.sh` prints them.

- The player's `horizon-dependency-overrides.json` in the game folder (Fabric's format) wins
  for any mod it names. `{"modid": {}}` keeps a mod's range as declared, and Fabric then
  refuses it in its own words.
- An override file the bridge didn't write is kept as the player's file first.
- `tests/boot120.rs` builds a mod declaring `fabricloader <=0.16.4`. The test checks that it
  boots with the automatic override (its code runs on 0.20.1), and that it is refused, naming
  the mod and the range, when the player keeps the range.

**`scan120.py`** reads a mods folder (nested jars too) without running anything:
- the declared loader, Minecraft and Java ranges, and whether the runtime's loader meets them
  (reason 1);
- every Fabric Loader class, method and field a mod references that the runtime's loader jar
  lacks (reason 3). It walks superclasses and interfaces. Only `Object`'s methods count as
  inherited from the JDK; another JDK supertype gets the benefit of the doubt.
- Java methods newer Javas removed, which give the newest Java a mod can run on
  (`java<=21`), plus APIs that newer Javas lock down.

`tests/test_scan120.py` compiles a fake loader and mod to check these.

**Reason 3, found for real: Rhino (KubeJS's script engine).** It references
`MappingConfiguration.getMappings()` (the old `TinyTree`, removed in loader 0.15) and
`FabricLauncher.getTargetNamespace()` (removed in 0.17). Scanning it against nine loader
versions (0.14.21 → 0.20.1) shows exactly that: on 0.16.x one of the two still exists. That is
the "stuck on 0.16" shape. In this case the code sits behind Rhino's dev-only
`RemappingHelper.GENERATE` flag (normally it downloads ready-made mappings), so it never runs
in play and KubeJS works. The scanner finds references; the boot and play decide whether they
matter. A `loader-compat` shim for these two calls is the first candidate for reason 3.

**Java version, found for real: Cobblemon 1.5.2** bundles GraalVM Truffle (it runs Pokémon
Showdown), which calls `sun.misc.Unsafe.ensureClassInitialized`. Java 22 removed that method.
On Java 25 its Showdown thread dies and the boot waits forever; on Java 21 it works (+169
blocks, Showdown starts). The scanner flags it `java<=21`. So a pack with Cobblemon runs its
1.20.1 child on Java 21 (Arch: `jdk21-openjdk`). A child that hangs can no longer hang
Horizon: every call has a read timeout (`Fabric120::timeout`, 10 minutes by default), after
which the child is stopped and the error carries its last log lines
(`tests/child.rs: a_child_stuck_in_a_call_times_out_and_is_stopped`).

**The headless client grew.** `Minecraft`'s resource pack repository is now built as vanilla's
constructor builds it: the built-in pack, the servers' downloaded packs and `resourcepacks/`.
Simple Voice Chat and KubeJS add pack sources through Mixins on it at start.

### The first tracker: Modrinth's 150 most downloaded Fabric 1.20.1 mods

`tools/java_bridge/survey120.py --top 150` boots each mod **alone**, with its required
dependencies (from Modrinth and from its own `fabric.mod.json`, since Modrinth's lists miss
some), on Java 25, and classifies the result. 149 mods (Fabric API itself is always in the
runtime):

| Verdict | Mods |
|---|---:|
| works on Java 25 | 148 |
| works on Java 21 only (Cobblemon 1.5.2) | 1 |
| partial / refused / broken | 0 |

The first run had 5 failures: Simple Voice Chat and KubeJS (the headless client's resource
packs, fixed), More Culling and Sodium Options API (dependencies missing from Modrinth's
lists: the survey's own fault, fixed), and Cobblemon (Java). A boot of one mod takes 11–30 s
on a 2-core box (median 13 s; Cobblemon's 150 s was its Java 25 timeout).

"Boots" means each mod's entrypoints run and its content registers. Chipped alone adds 6981
blocks, which matters for Horizon's block-state space in phase 2. It does **not** mean the
mod works in a world yet; that's phases 2–7. 39 of the 149 use APIs that newer Javas warn
about (mostly `sun.misc.Unsafe`, in bundled libraries). None of those is a blocker on Java 25
today. The table: `survey120-top150.tsv` beside this plan.

## Phase 2 status: the mods' blocks and items as Horizon content (2026-11-09)

Common mods first: the blocks and items they add become Horizon content mods, made the way the
26.3 bridge makes its own (horizon-jvm's `content.rs`). The 1.20.1 runtime writes the oracle
facts of what the mods register, and `horizon-port-pack`'s carry-over turns each jar into a
folder content mod: block declarations from the vanilla template of the same class and shapes,
with the mod's own numbers; models, textures, recipes, loot and tags upgraded to 26.3's formats.

**platform120: the bridge's code inside 1.20.1, built as Loom builds a mod.**
`java/platform120/src` is plain Java written with Mojang's names, as the 26.3 platform is.
`tools/java_bridge/build120.sh`:
1. merges Mojang's mappings with intermediary into one Tiny file (`Remap120`, on Fabric's
   tiny-remapper, pinned in `fabric120.build_tools`);
2. remaps Mojang's jar to Mojang's names to compile against;
3. compiles platform120;
4. remaps it to intermediary, with an intermediary → Mojang class table, into
   `~/.cache/horizon-120/1.20.1/horizon/platform120.jar`.

Every output derives from Mojang's files and stays in the cache. A build takes about 1 minute
the first time and seconds after. Later phases' Mixins and mirror code go into the same mod.

**Content120** (platform120) writes the facts from 1.20.1's registries. For blocks: class
chain, properties, every state's hardness, light, shapes, sounds and map colour, set and wood
types. For items: stack size, rarity, durability, food and armour, as 1.21.1 components.
**Tags120** loads the data packs as a server does but without a world (vanilla's, Fabric API's
convention tags, every mod's own data through Fabric API's pack source) and resolves every
block and item tag with vanilla's `TagLoader`. With 14 mods that's 312 block tags and 652 item
tags, so the old Fabric convention tags (`c:iron_ingots`, `c:shears`) that 1.20.1 recipes name
get their real members. Drops and placement need a level: the mods' loot tables come from their
jars.

**1.20.x data in 26.3's shape** (`horizon-port-pack/src/legacy120.rs`). The carry-over upgrades
1.21.1's data to 26.3's, and this first takes 1.20.x's to 1.21.1's:
- the plural folders (`recipes/` → `recipe/`, `loot_tables/` → `loot_table/`, `tags/items/` →
  `tags/item/`, ...);
- recipe results as item stacks (`{"item"}` and smelting's or stonecutting's bare id →
  `{"id", "count"}`);
- loot tool matches (`items` list and `tag` → one holder set, `enchantments` → the
  `minecraft:enchantments` sub-predicate, which the 26.3 step names `tool/can_silk_touch` and
  `tool/can_shear`);
- `set_nbt` / `copy_nbt` → their custom-data successors.

A jar's files in the old folders are read as 1.20.x's, since 1.21 and later never read those
folders.

**The 14 mods** (the Phase 1 set) become 16 content mods in about 40 s, including the boot:

| Mod | Blocks | Items | Recipes | Loot tables |
|---|---:|---:|---:|---:|
| Farmer's Delight | 90 | 90 | 147 | 100 |
| Supplementaries | 203 | 43 | 261 | 177 |
| Tech Reborn | 307 | 373 | 760 | 274 |
| Waystones | 25 | 7 | 32 | 25 |

Recipes of the mods' own types (Farmer's Delight's cooking pot, Tech Reborn's machines) are
carried as they are: they need the mods' machines (phase 3).

**In the game:** `game/java_mods_120.rs` behind the `java-mods-120` feature (no JNI: the
1.20.1 runtime is a child process). When `mods-java-1.20.1/` beside `options.txt` holds jars,
the game makes their content mods once per set of jars (cached under their stamp in
`.horizon-cache/java-120`) and layers them in like the 26.3 bridge's. The bridge's classes come
compiled into `horizon-jvm-remote` (its build.rs, `BRIDGE120_JAR`). `fetch120.py`'s folder and
`build120.sh`'s platform jar must exist on the machine; the game says which command to run if
they don't.

### In the game: the phase 2 milestone (2026-11-09)

The 14 mods in `mods-java-1.20.1/` of a scratch game folder, the game built with
`--features java-mods-120` and run off-screen (llvmpipe under Xvfb) through `horizon-ctl`:

- **Made and loaded:** the first start makes the 16 content mods in about 25 s (boot included)
  and layers them in. Later starts read them from the cache: no Java starts.
- **Placed and drawn:** Farmer's Delight's rice bag, stove, cooking pot and tomato crate,
  Supplementaries' checker block, Tech Reborn's ruby block and machine frame, and the Waystones
  waystone, each with its own model and textures.
- **Saved and reloaded:** all eight are still there after quit and restart (`execute if block`).
- **Drops:** each drops itself through its own upgraded loot table (`/loot ... mine`).
- **Crafting:** Farmer's Delight's beetroot crate → 9 beetroot, and Waystones' ender pearl +
  amethyst shard → 4 warp dust, crafted in the inventory grid.
- **Items:** the skillet, tomato, cooking pot, rice bag, warp dust, waystone, sconce and iron
  knife show in the hotbar with their icons and names. Picking up the knife gives vanilla's
  "Hunt and Gather", so the mods' item tags hold.

**A bigger pack, the same way: 36 jars** (Biomes O' Plenty with TerraBlender and GlitchCore,
Chipped, Handcrafted, Supplementaries, Farmer's Delight, Waystones, Traveler's Backpack, Carry
On, Lootr, Comforts, Dungeons and Taverns, Terralith, YUNG's Better Dungeons, their libraries,
JEI, Jade, Sodium...) became 36 content mods in 37 s at the game's first start. That's
Biomes O' Plenty's 429 blocks, 308 recipes and 401 loot tables, Chipped's 6981 blocks,
Handcrafted's 267 blocks and 285 recipes, and Comforts' and Traveler's Backpack's. In the
game, Handcrafted's chair and table, Chipped's woven wool and decorated terracotta, Biomes
O' Plenty's rose quartz, redwood log and jacaranda planks, and the backpack were placed and
drawn with their own models. Lootr's barrel draws as the missing texture: Lootr draws its
blocks through a custom Fabric model loader, the mod's own rendering code (phase 7).

Found and fixed on the way:
- One vanilla state the facts named but 26.3 doesn't have (`minecraft:piglin_head[rotation=0]`:
  26.3's heads gained `powered`) made the game refuse **every** content mod. `content120` now
  keeps only the vanilla blocks and states that 26.3 also has, from the prepared `facts.json`.
- Horizon's recipe book index, which `/recipe` reads, lists vanilla's and the data packs'
  recipes only, not content mods'. Crafting reads both. That's Horizon's, the same for every
  content mod: content mods' recipes don't show in the recipe book yet.
- A nested mod was only written out as a mod of its own when it shipped textures, and the check
  ran before looking inside it. Fabric also lets a jar name its nested jars anywhere, not only
  under `META-INF/jars/`. Both together lost LibMultiPart: Wired Redstone carries a *dependency
  container* (no textures, its four jars at the root), so the mod that registers
  `libmultipart:container` -- in code, with no assets at all -- was never inventoried, and the
  game refused the block every cable is a part of (`a mod set libmultipart:container[...]:
  unknown block`). `content120` now reads the `jars` list from each `fabric.mod.json`, writes
  out every nested jar that names itself a mod, and looks inside every one of them either way.
  A carried folder with nothing in it but its manifest (Kotlin's standard library) is not
  loaded as a content mod.

Not yet: what the mods' code does (the cooking pot cooks, the stove lights, machines run,
screens open). That is phase 3 on.

## Phase 3a status: the mods' code runs on Horizon's blocks (2026-11-09)

A right-click on a block runs in the 1.20.1 runtime as vanilla 1.20.1 runs it:
`ServerPlayerGameMode.useItemOn`, so the block's own `use`, Fabric API's `UseBlockCallback`
and the item's `useOn` are the mods' and vanilla's own code. They run on a **mirror world**
(platform120's `Mirror120`) whose blocks are Horizon's:

- **Reads** (`Level.getBlockState`/`getFluidState`) call Horizon (`blockState`, answered by the
  `World120` Horizon passes to `Runtime120::use_block`). **Writes** (`Level.setBlock`) are
  kept for the rest of the call (the mod reads its own write) and sent back as `setBlock`
  events; Horizon applies them when the call returns. States cross by name; a property one
  side lacks is dropped.
- The mirror's server, levels and player list are made without their constructors (there is
  no real server in this runtime); its players with vanilla's. Mixins quiet what needs a
  running server: sounds, particles, level and game events, neighbour updates (Horizon makes
  its own for what was set). Scheduled ticks are kept, not run (yet).
- Its **data** is loaded as a server loads it: the registry layers, with the data packs'
  worldgen and dimensions (and the registries mods add through Fabric API's
  `DynamicRegistries`: Moonlight's), then recipes, loot tables, advancements and functions,
  with the **tags bound** (`CampfireBlock.canLight` asks `#minecraft:campfires`).
- What **other mods do at the end of those constructors** is replayed (`Constructed120`, the
  26.3 bridge's `HorizonHooks.constructed` in small): the handlers merged into each class
  (`@MixinMerged`) whose `@Inject` is at `<init>` RETURN/TAIL are called with the arguments
  a server would have given. Without it Lithium's cached world height was 0 and every block
  read as outside the world.

Tested (`tests/boot120.rs`, a one-block fake world, with the 14 mods loaded): flint and steel
lights vanilla's campfire (`CONSUME`, worn by 1), makes fire on stone, and **Farmer's
Delight's own `StoveBlock.use` lights its stove** (`SUCCESS`, worn by 1).

In the game (`game/java_mods_120.rs`): the runtime stays up after the content is made, or
boots in the background when the content came from the cache (~30 s for the 36-jar pack;
clicks pass until then). A click on a block of a 1.20.1 mod's namespace, or with one of
their items, goes to it at `useItemOn`'s head (`java120_use_block`, beside the 26.3 bridge's
hook in `lan_actions.rs`); what it set and what it did to the held stack land in Horizon.

## Phase 6f status: the mods' dimensions (2026-11-14)

A mod's dimension (Deeper and Darker's Otherside) is a level stem, a dimension type, noise
settings and biomes in its data. A jar with a level stem of its own now gets the world
generation pack too (phase 6g's, `content120::needs_pack`), and each stem in that pack is
registered as one of Horizon's extra levels when the mods' content is ready, before a world
opens (`java_mods_120/levels.rs`, as the 26.3 bridge registers its mods' levels): its dimension
type is in the registries a joining player is sent, `/execute in deeperdarker:otherside` reaches
it, and Horizon's generator makes it from its stem, the pack's being read before any saved one.
The mods' placed features in it are placed by the runtime, as in the Overworld. The runtime's
mirror level for a dimension now takes that dimension's type (`Mirror120.typeOf`), so the mods'
code there sees its build range, ceiling and skylight.

Two safety nets came with it. A carver of a mod's own type (the Otherside's `noodles`) has no
Rust implementation, and Horizon's generator refused the whole world over it: such carvers are
left out of the pack, with the biomes' mentions of them, and logged. And if Horizon's generator
still can't build a level with the mods' pack, the world generates without it, with a warning,
rather than not opening (`world_loading.rs`, `refuse_mod_worldgen_packs`).

Checked in a game: the Otherside registers as a level (y 0 to 128), the player stands in it,
its chunks reach the player (sculk stone, sculk permafrost, shadow crystal and calcite, biome
`deeperdarker:deeplands`), and the runtime places its features (icicle patches, sculk spruce
forests, crystal cones). Not yet: the noodle carver (most of the Otherside's open caves; it
needs the runtime to carve, as features place), Deeper and Darker's portal and its teleports,
its two maze structures (a structure type of its own), and the dimension's sky and fog (drawn
as the Nether's).

## Phase 6g status: the mods' world generation over vanilla's (2026-11-14)

Terralith (data only, no code) puts its 101 biomes into the overworld by replacing vanilla's
files: `data/minecraft/dimension/overworld.json`, the overworld's noise settings and density
functions, and 35 of vanilla's biomes. The carry-over keeps a mod's files under its own
namespaces, so all 137 of those stayed out (`outside/`) and the world was vanilla's.

Now a mod whose jar has world generation under `minecraft` (a level stem or a worldgen file)
has all its world generation, its own namespaces' too, written as one 26.3 data pack at
content-build time (`content120::worldgen_pack`, `<cache>/java-120/content/worldgen`). Horizon's
generator and the world's biome table read that pack under the world's own packs
(`java_mods_120::worldgen_packs`, chained in `world_loading.rs` beside the 26.3 bridge's
structure packs), as Fabric adds a mod's data as a pack. The files go through the 26.2 to 26.3
pack upgrade (`upgrade::upgrade_pack`) after what 1.20.x wrote differently (`legacy120.rs`):

- int and float providers with their bounds in a `value` object (1.20.5 flattened them);
- a biome's `effects` (sky, fog and underwater fog colours, particle, sounds, music) as
  environment attributes, and its carvers by step as one list;
- surface rules naming biomes nothing registers (Terralith names a few it never made): 26.3
  reads `biome_is` as a biome list and refuses the whole rule, so those names are dropped;
- the biomes' feature lists in 26.3's vanilla order: 26.3 reordered some of vanilla's (sugar
  cane after pumpkins), and Terralith's oceans in 1.20.1's order beside 26.3's own oceans made
  the feature sorter find a cycle.

The pack upgrade itself learned what 26.3 replaced with data: `random_patch` and the flower
features as count and offset placement under a selector (as 26.3's vanilla writes its
patches), `nether_forest_vegetation` the same on nylium, `weird_scaled_sampler` as an interval
selection of scaled noises (as 26.3's vanilla caves), `dripstone_cluster` as a dripstone
`speleothem_cluster`, `forest_rock` as a `block_blob`, and a disk's untyped rule-based provider.
Horizon's generator reads a noise settings' default block or fluid in 26.3's `{"id",
"properties"}` form too (it read only the older `Name` form or a bare id).

The mods' own placed features (Terralith's 552) are placed by the runtime with their 1.20.1
code, as phase 6b's.

Checked: a Terralith world generates its biomes (`/locate biome terralith:yosemite_cliffs`
finds one 2,557 blocks from spawn, and its white cliffs and forest look as Terralith's), with
the runtime placing Terralith's features. Not yet: the pack's dimension types (the level shapes
still read only the world's packs).

Not planned for now: biomes added by code through TerraBlender (Biomes O' Plenty, Regions
Unexplored, Oh The Biomes We've Gone). It is the common way for 1.20.1 biome mods, but those
mods have 26.3 releases, and the 26.3 bridge already generates Biomes O' Plenty's overworld
(`java-bridge-perf.md`). On the 1.20.1 side it would need TerraBlender's regions exported from
the runtime and a hook in Horizon's biome source.

## Phase 6e status: the mods' fluids in the world (2026-11-14)

A mod's fluid block (Tech Reborn's 35: oil, diesel...) is carried on water's or lava's
template, so Horizon's fluid code ran it as that: a placed oil source spread as
`minecraft:water`. Now their states are foreign to Horizon's fluids (`LevelAccess::
foreign_fluid`, as the 26.3 bridge's): Horizon's water and lava leave them, and where Horizon
would schedule their tick (placed, a neighbour changed) it tells
`game/java_mods_120/fluids.rs` instead. Each server tick those positions go to the runtime
(`fluidChanged`), which schedules each fluid's tick at the mod's own delay in the mirror level's
fluid ticks (`FluidTicks120`: a schedule kept by position, as a level without chunks drops
vanilla's), and the ticks due run there (`fluidTicks`: `FluidState.tick`, the mod's own spread
and drop-off); the blocks they set land in Horizon's world, which tells the module again, and
so it flows. A level with nothing pending costs no call.

They are drawn with their own sprites: the runtime reads each fluid's still and flowing texture
where it's kept (Fabric's `SimpleFluidRenderHandler`, a fluid's own settings as Reborn Core's,
else the handler's sprites; this client stitches no atlas) and the game hands them to Horizon's
mesher as the 26.3 bridge's fluid looks (`fluidLooks`). Only where the runtime runs in the
game's process (single player, a LAN host), as the 26.3 bridge's.

Checked: in the runtime test, an oil source spreads to its four sides as oil one level down,
on its tick and not before, and the spread oil's ticks are scheduled when Horizon says it placed
them; in a game, a placed oil source spreads as oil down to level 7 (eight blocks) and stops,
drawn black with its own sprites. Not yet: entities in them (Horizon's physics read the
template: a player in oil swims as in water), Horizon's water and lava meeting them, their looks
on a remote player's game, and Tech Reborn's oil lakes, whose feature fails in the runtime
("Should always be able to create a chunk").

## Phase 5n status: the mods' tooltip lines (2026-11-14)

What the mods add to an item's tooltip shows in Horizon's, which builds the rest itself: a mod
item's own lines (`Item.appendHoverText`: Farmer's Delight's meals list their effects) under the
name, and what tooltip mods append to any stack (Fabric's `ItemTooltipCallback`, fired as at
the end of `getTooltipLines`) at the end (`tooltip`). The runtime is the server's and a tooltip
shows in a player's game, so the server tells each game once that it answers tooltips; the game
asks for a stack the first time its tooltip shows, and again a second later while it still
shows; the answer is kept per stack and its lines go in as 1.20.1 puts them
(`game/java_mods_120/tooltips.rs`). The stack goes down as 1.20.1 NBT with what the fixes kept
of its own data (`minecraft:custom_data`: a battery's charge) back under its `tag`.

Checked: in the runtime test, beef stew's own line is Comfort for 3:00 and bread has none from
the runtime (vanilla's lines are Horizon's); in a game, the stew's tooltip shows it in blue under
its name. Not yet: tooltip images (`TooltipComponent`: AppleSkin's food preview, shulker box
contents), which draw rather than add lines.

## Phase 5m status: the mods' tools, weapons and armour (2026-11-14)

A mod's tools and weapons came over as sticks with durability: their own classes (Farmer's
Delight's `KnifeItem`, Tech Reborn's tools) matched no template, and the export had no attribute
modifiers or mining rules. Now each item says the vanilla class it extends (`vanilla_class`: a
mod's sword plays as a sword, its pickaxe as a pickaxe; the port-pack's template follows it),
its default attribute modifiers in the hand or its armour slot (attack damage and speed;
armour, toughness and knockback resistance, under 1.21's ids), and a tool's rules as 1.21.1's
`tool` component: a `DiggerItem` mines its block tag at its tier's speed and drops it where its
tier is enough (`incorrect_for_<tier>_tool` by the tier's level), a sword cuts cobwebs and what
swords cut. Durability and repair items were exported before.

Checked in a game: Tech Reborn's bronze pickaxe mines iron ore in about 2 seconds with the walk
to it, drops raw iron and wears one point (a stick: 15 seconds, nothing); Farmer's Delight's iron
knife hits a zombie for 3.4 of its 3.5 (an almost full swing); Tech Reborn's bronze chestplate
gives 5 armour (it had the iron template's 6) and its ruby helmet 2 more. A tool whose mining
speed is its own code (an energy drill's `getDestroySpeed`) mines as its tier says.

## Phase 5l status: the mods' villager trades (2026-11-14)

The trades the mods add (Fabric's TradeOfferHelper puts their listings into 1.20.1's
`VillagerTrades` tables, beside vanilla's) are 26.3 data: the content build asks each mod's
listing for its offer once (the facts' `trades`) and writes it as a `villager_trade` in the
carried mod of the namespace it trades in, added (not over vanilla's) to its level's tag or the
wandering trader's pool (`content120.rs`: `trades`; 1.20.1's rare wandering trades go to 26.3's
`uncommon`). Left out: a listing that needs a villager or a level to answer (Supplementaries'
adventurer maps), one that gives an item with data, and the trades of a mod's own profession
(Tech Reborn's), which has no villagers in Horizon. A random listing keeps the offer it gave.

Horizon's trade loader read only `data/minecraft/`, so a content mod's trades never loaded
though its tag named them: it now reads every namespace's `villager_trade` and `trade_set`, as
a data pack's are (`merchant.rs`, with a test; the trading harness passes).

Checked in a game with Farmer's Delight Refabricated: farmers offer its rice and cabbages for
emeralds beside vanilla's trades. With the Fabric Farmer's Delight beside Moonlight (mods1),
the runtime's own tables no longer hold FD's trades when the export reads them (Moonlight's
data trades are the likely cause): what 1.20.1 has is what goes over.

## Phase 5k status: the mods' effects and food (2026-11-14)

The mods' effects are Horizon's: the content build writes each (its category and colour) as a
content effect (`data/<ns>/horizon/mob_effect/`), so food gives them, `/effect` knows them, the
HUD shows the mod's icon and the save keeps them. Their foods are eaten as in 1.20.1: each food's
eating time, eaten or drunk, its effects with their chances and what it leaves (Farmer's
Delight's stews their bowl: its crafting remainder) become the item's `minecraft:consumable` and
`minecraft:use_remainder` (`content120.rs`: `mob_effects`, `foods`; the facts' `mob_effects`
and each food's `consume`). Before, a mod's food gave none of its effects, not even vanilla's.

What an effect does is the mod's code: as the player's effects tick, the 1.20.1 ones due are
collected (`potions.rs`), then run in the runtime on the player with Horizon's numbers
(`effectTicks`: `isDurationEffectTick`, then `applyEffectTick`, with the player's effects as
Horizon has them), and the health, absorption, food, saturation, exhaustion and air they leave
come back (`java_mods_120/effects.rs`). On mobs they don't act yet.

Checked: in the runtime test, Comfort heals a hurt player with no saturation on its 80th tick
only, and not with Regeneration on; Nourishment takes the exhaustion back. In a game, beef stew
leaves its bowl and gives 13 food and Comfort for 3600 ticks, whose icon is on the HUD; with the
saturation gone, Comfort heals one point every four seconds.

## Phase 5j status: the mods' commands and messages (2026-11-14)

The mods' commands (`/waystones`, `/supplementaries`, `/techreborn`...) are in every player's
command tree and run. The runtime's server has them in its dispatcher (Fabric API's command
callback registers them as its data loads); Horizon takes the names that aren't its own
(`commandNames`) and their tree in the commands report's form (`commandTree`: an argument of a
mod's own type goes as a greedy string), merged after vanilla's and the other mods'
(`game/java_mods_120/commands.rs`). A player's line for one runs in the runtime as that player,
at their permission level, in their level (`command`); what it changes lands in Horizon as any
call's changes do.

What the mods tell a player reaches them: `ServerPlayer.sendSystemMessage` (a command's answer,
its errors, a mod's notice, `displayClientMessage` over the hotbar) goes to Horizon as the
1.20.1 JSON text (`World120::tell`), and the server's next tick sends it to that player's chat
or action bar. Translated text stays translated: the mods' language files are in their content.

Checked: in the runtime test, `/supplementaries roll 6` answers its player with Supplementaries'
dice message, and a line the dispatcher can't parse gets vanilla's error; in a game, the mods'
commands are listed when the world opens, and `/supplementaries roll 20` answers in chat. (Its
English text, `Rolled D%1$d: %2$d`, shows as written: vanilla's translations take only `%s`, so
1.20.1 shows the same.)

## Phase 5i status: the mods' HUD (2026-11-14)

What the mods draw on the HUD is drawn over Horizon's: AppleSkin's saturation and exhaustion
over the food bar, Farmer's Delight's nourishment and comfort, any mod's `HudRenderCallback`.
In 1.20.1 most of it is drawn from mixins into vanilla's `Gui` parts, so the runtime runs the
parts mods inject into (`Hud120`: the hotbar, the hearts and food where the player can be hurt,
the experience bar, the effects), then every handler merged into `Gui.render` (Fabric's, which
fires the callback, and the mods' own), and keeps only what the mods draw: an operation is a
mod's when, walking out from the drawing call, a mod's class or a mod's merged handler comes
before the glue (`Graphics120.modsOnly`, as the 26.3 bridge's `HorizonGraphics`). Vanilla's
hearts and bars are dropped as drawn.

The player is Horizon's: health, max health, absorption, food, saturation, exhaustion, air and
experience go with each frame and are set on the runtime's player, whose state the client player
shares (its synced data, food, attributes, effects, abilities). The server draws each player's
HUD every tick and sends it on the 26.3 bridge's HUD channel when it changed
(`game/java_mods_120/hud.rs`); the game draws it over its HUD and says its GUI size, which the
next frame is laid out in. With both bridges' mods in one world, a player gets one frame with
both parts (`java_screen/hud_parts.rs`).

Checked: in the runtime test, a hungry survival player's frame holds AppleSkin's exhaustion
underlay (2.5 of 4: 50 of the bar's 81 pixels) and two saturation shanks over the food bar, and
none of vanilla's 43 operations; in a game, AppleSkin's gold saturation outlines over Horizon's
food bar. A frame costs about 2.2 ms of the server's tick per player (the runtime's call and the
stack walks), which suits a few players.

What the client needed: a frame timer (Jade asks the frame's delta), an empty client level while
a frame draws (the hearts ask whether the world is hardcore), the GUI made without its
constructor, and raw LWJGL calls answered (`RawGlMixin`): AppleSkin asks `GL11.glIsEnabled`
before it draws, and with no GL context LWJGL aborts the whole JVM.

Not yet: overlays about the world as the client sees it. Jade's tooltip needs what the player
looks at (`Minecraft.hitResult`) and a client level with the blocks; minimaps need the chunks.

## Phase 5h status: the mods' settings screens (2026-11-13)

A mod's settings screen opens from Horizon's Mods page (Options, Mods...), listed as
"<mod>: Settings", and is drawn and used as any screen of the mods' (phase 4). The runtime
finds them as Mod Menu does, from the mods' `modmenu` entrypoints (`Config120`, through
FabricLoader by reflection: it isn't on the glue's classpath), and lists every mod with a
factory (`configMods`); the server sends the list to its players with the keys
(`game/java_mods_120/config.rs`, channel `horizon:java120_config_mods`) and a click opens the
mod's screen for that player (`openConfig`). Checked with AppleSkin's (Cloth Config): its
options drawn with their labels, a tooltip on hover, a toggle clicked to "No", Save & Quit
writing the mod's `appleskin.json5`, and the screen closing back to Options.

What the client needed that its constructor would have made: the client's language with the
mods' translations (`Screens120.language`), a window the frame's size at GUI scale 1 and a mouse
handler where the frame's pointer is (Cloth asks the client, not `render`), the last input type,
and a sound manager that plays nothing (a button's click; Horizon plays its own). Clipping a
mod sets straight on GL (`GlStateManager._scissorBox`, Cloth's list) is recorded as a scissor
of the frame, and text drawn with no alpha is opaque, as `Font` makes it.

The settings files are the runtime's (`.horizon-cache/java-120/game/config/`), one per server:
a mod's client-only settings (AppleSkin's HUD) take effect on its HUD (phase 5i).

## Phase 3f status: energy and fluids between the mods' blocks (2026-11-13)

A tech mod's machines pass energy (Team Reborn Energy's `EnergyStorage.SIDED`) and fluids (the
Transfer API's `FluidStorage`) through Fabric's block lookups, which run in the runtime on
Horizon's blocks: the lookup finds the neighbour's block entity there, and both stay the
runtime's between ticks (as a chunk keeps them), what they become going to Horizon. Checked
with Tech Reborn: its creative solar panel powers its electric furnace, which smelts; a water
bucket emptied into its tank unit fills the tank and comes back empty.

What it needed: a mod's block placed by Horizon without the mod's code (`/setblock`, `/fill`, a
structure) has its block entity. The runtime lists, when it starts, the mods' blocks that have
one and its type (`modBlockEntityBlocks`: 260 with Tech Reborn's set), and Horizon makes it
however the block is placed (`new_block_entity`, and the edits `/fill` tracks), empty, for the
mod's code to fill as it reads it; the runtime also makes one where Horizon has none.

Checked in a game: Tech Reborn's creative solar panel and electric furnace placed by `/setblock`,
the furnace charged by the panel. (A hopper doesn't feed it there: Tech Reborn's slot config
takes no automated input until set in the machine's screen, as in 1.20.1.)

Not covered: energy or fluids between a mod's block and Horizon's own (Horizon has no energy;
vanilla's cauldron through the Transfer API is unverified), and `/data merge` on a mod's block
entity whose `Items` aren't vanilla's list (the command reads them as one).

## Phase 5f/5g status: sounds, particles, and vanilla mobs the mods make (2026-11-13)

What the mods' code plays and shows in the world reaches Horizon's players. A sound
(`ServerLevel.playSeededSound`, both overloads: where every `Level.playSound` and
`Entity.playSound` ends) goes to Horizon by name with its position, source, volume and pitch;
particles (`ServerLevel.sendParticles`) as vanilla's own string of them with where, how many,
spread and speed. They wait in `game/java_mods_120/effects.rs` and the server's next tick sends
them to the players near through the queues Horizon's native mods' sounds and particles use:
a mod's own sounds by name (they're in its content), vanilla's particle types as Horizon's
(block particles with their block's state, dust with its colour), a mod's own particle type by
name when it has no parameters. Every player hears a sound, the one whose action caused it too:
vanilla leaves that one out because their own client played it, and no client of theirs did.

A vanilla mob the mods' code adds to the world (the cow a sheared moobloom turns into) is
Horizon's, as vanilla's mobs are: handed over as 1.20.1 saves it, upgraded by vanilla's fixes
(`Ty::EntityTree`) and loaded as Horizon's own.

**5g, done: the blocks' client effects.** What a block's client code makes (`Block.animateTick`:
a stove's flames and smoke, a cooking pot's steam) comes too. The server half picks blocks
around each player as `ClientLevel.animateTick` does (667 random blocks within 16, 667 within
32, each `x + nextInt(r) - nextInt(r)`, with a random of its own so Horizon's world random is
untouched), keeps the mods' and has the runtime run their `animateTick` on the mirror
(`animateTicks`); what they add there (`Level.addParticle`, `addAlwaysVisibleParticle`, a
velocity being the packet's count 0) and play (`playLocalSound`) goes out as above.

Not covered: sounds and particles a mod makes elsewhere on its client half (ambient loops,
a screen's), and level events.

Checked in a game: a moobloom sheared turns into Horizon's cow and drops its buttercups; in the
runtime test, milking plays `minecraft:entity.cow.milk` where the player stands, and shearing
sends the explosion puff and plays `friendsandfoes:entity.moobloom.shear`. Lit stoves placed by `/setblock`
show their flames and smoke; in the runtime test, forty `animateTick`s of a lit stove send
smoke at its front.

## Phase 7b status: block entity renderers (2026-11-13)

What a mod draws on its block in code, through a `BlockEntityRenderer` (Farmer's Delight's
cutting board showing its item, a skillet its food), is drawn by that renderer. `Render120`
makes the mods' renderers from `BlockEntityRenderers`' providers with a context as the client's
(an `ItemRenderer` over the client's baked item models: a `ModelManager` given `Models120`'s
top-level bake; the client is handed it, and renderers that ask it, as FD's does, get it). The
server half (`game/java_mods_120/block_renders.rs`) asks the runtime, every other tick, to draw
the mods' block entities within 48 blocks of a player (`renderBlockEntities`: the meshes of each,
relative to the block's corner, as the mobs' are) and sends them on
`horizon:java120_block_renders` when they changed (and every five seconds for a player who joined);
the window draws them on their blocks with the mobs' mesh code, lit by the light there.

What it needed of the runtime:
- A mod's block placed without its block entity (`/setblock`, a structure) gets one, as a chunk
  makes it with the block, sent back to Horizon with the rest of the call.
- Work handed to the client or server "to do on its thread" while other work runs waits for it,
  bounded: Porting Lib's tool-tier sync answered itself between the two halves (the server sends
  them, the client half asks it to send them again) and recursed until the stack ran out.

Checked in a game: a carrot put on a cutting board placed by `/setblock` lies on it, drawn by the
board's own renderer; the runtime test draws it from the board's NBT, with the carrot's texture.

**Next:** LibMultipart's dynamic part renderers, then the phase-6 rest (structures with Java
pieces, the mods' biomes and dimensions) and the unverified rows of the support page.

## Phase 6d status: the mods' own mobs (2026-11-13, done)

Tested with Friends & Foes 5.0.4 (its moobloom), which uses plain models, no GeckoLib.

**6d.1, done: they live in the runtime.** A mod's entity (not vanilla's, which are Horizon's) put
into a mirror level lives there (`Entities120`, the level's `LevelEntityGetter`, so mobs see
each other), with ids of its own range (`0x7ffe0000` and up). `summon` makes one as `/summon`
does (`finalizeSpawn` with `COMMAND`). `tickEntities` ticks a level's entities with their own
code and says where each is after.

What a mob needed of a mirror level:
- A profiler: its goal selector profiles into the level's (the inactive one).
- Damage sources, for fall damage.
- Collisions: `Level.getChunkForCollisions` answers with the level itself, so its physics read
  Horizon's blocks rather than no chunk.
- Entity tracking: the chunk cache's broadcasts and its entity tracking do nothing, since
  Horizon draws from snapshots.

The runtime test summons a moobloom at 70 over Horizon's grass. It falls and stands at 65, with
vanilla's fall damage (10 health to 8), and its AI turns its head.

**6d.2, done: they're drawn by their own renderers.** Each tick, `Render120` runs each mod
mob's own `EntityRenderer` (made once from `EntityRenderers`' providers, with the client's entity
models and baked block models) into a capture of its quads: positions relative to the mob, the
texture each is drawn with (from its `RenderType`), the overlay (the hurt flash). Quads drawn on
the block atlas (the moobloom's flowers) are split into per-sprite meshes. The server half
(`game/java_mods_120/mobs.rs`) sends each player of the dimension the mobs and their meshes on
`horizon:java120_entities`; the window draws them where the mob is between the last two ticks,
with the GUI atlas' sprites, lit by the light where it stands.

What a renderer needed:
- `Minecraft.getBlockRenderer()`: Friends & Foes' flower layer asks the client for it when made.
- `ClientPacketListener.getPlayerInfo`: answers none while the headless client has no players.

Checked in a game: a moobloom from its spawn egg walks on Horizon's grass with its buttercups.

**6d.3, done: players hit and use them.** The window aims at the mobs it draws (their boxes,
among Horizon's entities, `java120_mob_hit`) and sends vanilla's attack and interact packets
with their ids. The server hands those in the mods' range (`0x7ffe0000` to `0x7fff0000`) to
the runtime, as 1.20.1's `handleInteract` would (nothing from more than six blocks away):
`Mirror120.entityAction` stands the player there with the held item and runs
`ServerPlayer.attack` (Fabric's `AttackEntityCallback` at its head) or Fabric's
`UseEntityCallback`, then `interactAt`, then `Player.interactOn`. A kill drops the mob's loot
in Horizon's world, the held item wears or changes (a bucket milked), and what the click put
into the player's inventory (the milk, in creative) is given to Horizon's player.

What a blow needed:
- The held item's attribute modifiers, which `LivingEntity.tick` would put on: the runtime
  player never ticks.
- How charged it is: Horizon's ticks since the player's last blow.

Checked in a game: a moobloom milked with a bucket, then killed with a netherite sword in two
blows (8 points each), dropping beef.

**6d.4, done: kept with the world.** A kill's experience comes out as Horizon's own orbs
(an `ExperienceOrb` the mods' code adds is a `spawnXp`, of its value). Each dimension's mods'
mobs are saved with it, as 1.20.1's `Entity.save` writes them, to
`dimensions/<ns>/<name>/data/skyline/entities120.json` (beside `raids.dat`); on the dimension's
first tick they go back into the runtime (`EntityType.loadEntityRecursive`). A dimension's file
is written only after its mobs came in, so an early save never writes over them; a different
world opened in the same game takes the last one's away. Mobs in chunks Horizon hasn't loaded
wait rather than tick (a batched `blockStates` read of each chunk: "" is unloaded), as vanilla's
do in chunks that don't tick, instead of falling through the air such a chunk reads as.

Checked in a game: a moobloom spawned, the game quit and opened again, and it stood where it
was with its health.

**6d.5, done: `/summon` and light.** Horizon's `/summon` makes the mods' types in the
runtime, with NBT as vanilla's SummonCommand takes it (no `finalizeSpawn` then): the runtime
lists its mods' entity types when it starts (`modEntityTypes`) and each game adds them to its
commands' registries (the export's `minecraft:entity_type` list, which `ids()` reads first, and
`summonable`), on the tick after, since the runtime may start after the world opened. A mirror
level's light is Horizon's (`Light120`, a `LevelLightEngine` over no chunks answering from
`World120::light`): mobs read it (a moobloom choosing where to grow), and without it their tick
failed on vanilla's engine, which a mirror level never made. The window's drawing of the mobs
had sky and block light the wrong way round in the lightmap; fixed.

Checked in a game: `/summon` of a moobloom (also with `{Health:3.0f,NoAI:1b}`) and of a copper
golem, each drawn by its own renderer, ticking with no failures.

**6d.6, done: natural spawning.** The spawns the mods add to vanilla's biomes in code
(`BiomeModifications.addSpawn`) come out of `Worldgen120.biomeChanges` with the rest and are
written as NeoForge `add_spawns` biome modifiers, each spawner saying its category
(`horizon:category`): Horizon has no row for a mod's own type, and a spawner that says its
category is let through for it (`mod_content_biomes`). Horizon's spawner, picking a type it
can't make itself, hands it to the runtime through `NaturalLevel::foreign_spawn` (the 400-tick
pass) and `foreign_generation_spawn` (`spawnMobsForChunkGeneration`, through vanilla's own
`getTopNonCollidingPos`): the type's placement and spawn rules and the mob's own checks are the
mod's code, then `finalizeSpawn` as a natural or chunk-generation spawn. Both are default no-ops,
so vanilla's path and its random's order are as they were. The mods' mobs count toward the
mob caps (their category and persistence come with each tick), and the players of a level stand
in its mirror each tick (`ServerLevel.players`), so vanilla's `checkDespawn` sends away a monster
far from all of them. What it needed of the runtime's level: Horizon's heightmaps (the surface a
spawn stands on) and no raids (an illager asking).

Checked in a game: maulers spawned with their savanna chunks, drawn by their own renderer; in
the runtime test a moobloom spawns on grass and not in the air, and an iceologer goes when every
player is more than 128 blocks away.

**Next:** the phase-6 rest (structures with Java pieces, the mods' biomes and dimensions), phase
7's block entity renderers, and the unverified rows of the support page.

## Phase 7 status: blocks drawn by their mod's code (2026-11-12)

A block whose look is code has no model Horizon can read: Wired Redstone's wires and cables
are LibMultipart containers whose model is built per position from the parts in the block
entity (the wire, its side, its connections, its power). They drew as stone.

- **The client's models, baked in the runtime** (`Models120`): ModelManager's loaders over
  vanilla's and the mods' resource packs, with Fabric's model loading (LibMultipart's container
  model is a plugin), on stand-in sprites (`Sprites120`: each its own slot of UVs, so a quad's
  UVs say its sprite). A mod's state whose model is a Fabric model, not a plain vanilla one, is
  drawn by code (3072 states with Wired Redstone).
- **A block drawn there.** Indigo's own block renderer draws it into a capture, in a view of the
  mirror level (Horizon's blocks and block entities). The quads come back with their sprite,
  corners in the block, UVs in the sprite and colour (`Runtime120::render_block`). The server
  half draws such blocks again when their state or block entity changed
  (`game/java_mods_120/drawn.rs`). The window puts the quads into the mesher as that
  position's model, on Horizon's sprite of the same name (the mods' block textures are in the
  block atlas), and meshes the section again.
- **LibMultipart's model keys.** The container keeps its parts' model keys and makes them again
  only in a client world, when the server tells it to redraw. The mirror is the server side,
  so a cable drew as it was when it loaded. They are made again from the parts before the block
  is drawn.
- **The wires link and carry power.** GraphLib links nodes in batches over ticks and tells the
  players' clients of each merge. Two things a mirror player lacked threw there, every tick,
  and the wires never linked:
  - Fabric's `PlayerLookup.tracking` asked the mirror level's chunk map, which was never built.
    It now answers with the mirror's players in view distance.
  - LibNetworkStack kept no connection for a handler whose channel isn't open, and a mirror
    player's never is (packets are handed over in-process). Now it is accepting.
- **Neighbour updates.** Horizon's neighbour updates beside a mod's block reach it each tick
  (`LevelAccess::observe_neighbours_updated`, `java_mods_120/neighbours.rs`, the runtime's
  `neighbours`): a wire takes the power of a redstone block placed beside it, and loses it when
  the block goes.
- **No more bake warnings.** Vanilla's item models read their sprite's pixels for their edges.
  A stand-in sprite has none, which logged ~590 warnings at every start; it now reads as clear.

Runtime test (Wired Redstone): a cable draws only Wired Redstone's sprites, inside its block.
A second cable beside it links within GraphLib's ticks (its block entity says it), and the
first then draws its arm to the shared edge. A red alloy wire beside a redstone block carries
15 and draws the powered sprite. All three mod sets pass.

In a game (Anthony's set):
- Bundled cables and red alloy wires laid in rows draw as one linked cable and one wire.
- A wire beside a redstone block draws powered.
- A redstone block put beside an unpowered wire powers all four blocks of it, and taking it
  away brings them to 0.

**Found in Anthony's game (2026-11-12, fixed):**
- **The wires could not be mined: the crosshair went through them.** The server syncs a
  block's shape as an attachment in the tick it places the block, and the window can get it
  before the block itself. When the block came, the window dropped the attachments it held
  for that position as the old block's. There was none (air), so they were the new one's. Now
  they are only dropped when a block entity is actually replaced (`remote.rs`,
  `sync_block_entity`).
- **A wire did not light a lamp.** Horizon's redstone asked the registry, which knows nothing
  of a mod block's power. Now, whenever a mod's block changes, the server asks the mods what it
  gives toward each side (`blockSignals`) and which of their states are sources
  (`signalSources`), keeps it (`java_mods_120/redstone.rs`), and answers Horizon's
  `adjust_signal` / `adjust_signal_source` from it. A change updates the block's neighbours and
  theirs, so the lamp looks again.
- **Collision is the mod's** (`collisionShapes`). It is kept per position with the state it was
  asked for (`collisions::set_mod_collision`), so the collision scan uses it, and synced as the
  `horizon:block_collision` attachment. Wired Redstone's ground wires and cables are walked
  through, as in 1.20.1 (their `getCollisionShape` is empty, like redstone dust); its standing
  wires are solid.
- **GraphLib's client half logged a stack trace on every graph merge**, because the headless
  client has no world. Its storage lookup answers none while there is no world, which GraphLib
  already handles.

Checked in a game: a redstone block, three wires, a lamp. The lamp lights. The crosshair finds
the middle wire, and in survival it breaks in a moment and drops. The lamp goes out, and the
wire past the gap loses its power. No GraphLib warnings.

Not yet: gates' and other parts' animated or dynamic renderers (LibMultipart's dynamic part
renderers, block entity renderers of 1.20.1 mods), and light from code-drawn blocks.

## Phase 6c status: the mods' structures (2026-11-12)

The content build already carries a 1.20.1 mod's data-driven structures: its structures,
structure sets, template pools, processor lists and `.nbt` templates, from 1.20.1's folders to
26.3's. Two things kept them from generating whole:

- **1.20.1's names in the data.** A processor rule naming `minecraft:grass` (`short_grass`
  since 1.20.3) failed its list, and with it every pool that named the list: Towns and Towers'
  forest ruins were left out. The carry-over renames the ids vanilla renamed (`grass`, `scute`,
  and `chain` as before) in a mod's data.
- **Templates read as saved.** Vanilla brings a template from its `DataVersion` up to its own
  (`StructureTemplateManager.readStructure`): the palettes' states, the blocks' block entities,
  the entities. Horizon read a template as it was, so a 1.20.1 template's renamed blocks were
  air. `worldgen::structure::upgrade_template` now runs vanilla's fixes on it first.

Checked in a game with Towns and Towers 1.12 and Cristel Lib:
- No warnings at load.
- `/locate` finds its villages and outposts.
- The forest village at -704 256 stands in mossy cobblestone and stone bricks, with its chests.

Structures written in Java (YUNG's, Repurposed Structures' own piece types) are not
placed yet.

## Phase 6b status: the mods' own feature types, placed by their own code (2026-11-12)

A feature of a mod's own type (Farmer's Delight's `wild_crop`, its mushroom colonies, a mod's
custom placement modifier) has no Rust implementation, so Horizon's generator skipped it (6a).

- **The hook.** Horizon's generator now has one for features it can't run
  (`horizon-world`'s `worldgen::foreign`).
  - The decoration step hands such a feature to whoever installed the hook, where vanilla's
    `applyBiomeDecoration` would place it.
  - It passes what seeds vanilla's random (the chunk's decoration seed, the feature's index and
    step), and the region being decorated as blocks, biomes and heightmaps by name.
  - Writes are allowed inside the region's write zone only.
- **The runtime places them.** With 1.20.1 mods, the game installs the hook as their runtime
  starts (`game/java_mods_120/worldgen.rs`). The runtime places the mods' features
  (`Worldgen120.place`):
  - Seeding: a `WorldgenRandom` seeded as vanilla seeds it.
  - The call: `PlacedFeature.placeWithBiomeCheck` on the mirror level, with a stand-in chunk
    generator for the overworld's shape and each biome's generation settings (`Generator120`).
  - Reads: the level's blocks, heights (`Level.getHeight`) and biomes (its `BiomeManager`,
    `Biomes120`) are the region's.
  - Writes: what the feature sets goes into the region at once, so its next reads, and the
    heightmaps, see it.
- **Block names.** States come back under 1.20.1's names and go through vanilla's own fixes to
  26.3's (`minecraft:grass` is `short_grass`). Blocks a mod's code sets in a running game now
  get the same fix.

The runtime test places FD's wild carrots on a flat meadow, in a biome Fabric gave them to,
until the rarity filter lets one through. Thirteen carrots land on the grass with their coarse
dirt and grass around them. All three mod sets pass.

In a game with Anthony's set, the generator hands the runtime FD's 9 features. A patch of wild
carrots was found in a new chunk: four carrots on coarse dirt, under birches. With
`HORIZON_120_TRACE=1` the runtime says each feature it placed.

## Phase 6a status: the mods' features placed in vanilla's biomes (2026-11-12)

A Fabric mod puts its ores and plants into vanilla's biomes in code, through Fabric API's
`BiomeModifications`, which a real server applies as it starts. Now:

- The runtime applies them to its worldgen registries (vanilla's and the mods' data, loaded as a
  server does) and compares each biome's feature lists with what they were
  (`Worldgen120.biomeChanges`).
- The content build writes what was added and taken away as NeoForge biome modifiers in the
  carried mods (`content120::biome_modifiers`). Horizon's content build already reads those into
  its biomes' feature lists, and its generator orders them as vanilla's `FeatureSorter` does.
- A feature a mod registers in code, so that no carried mod has its file, is written from the
  runtime's own encoding of it.

Features of vanilla's types (ores, most patches, trees from vanilla's parts) now generate in
new chunks. Features of a mod's own type (Farmer's Delight's `wild_crop`, its mushroom colonies)
are skipped by the generator ("placed features cannot run"); 6b runs those in the runtime. In a
game with Anthony's set: 78 modifications to 41 biomes, 11 modifiers, FD's 9 patches skipped.

## Phase 5e status: mods' key bindings and the client tick (2026-11-12)

- **The keys the 1.20.1 mods bind** (Fabric's `KeyBindingHelper` registry, read by
  `Keys120.modKeys`) go to each player's game once, on their own channel
  (`horizon:java120_keys`, `game/java_mods_120/keys.rs`), so the 26.3 bridge's list and this
  one don't replace each other. The game sends those keys while playing exactly as it sends
  the 26.3 mods' (`{"kind": "bind", ...}` on the screen input channel); the server hands each
  press to the runtime and still passes it on to the 26.3 bridge.
- **A press** sets the mod's `KeyMapping` and clicks it, as 1.20.1's keyboard handler does
  (`Keys120.press`). On the next server tick, for each player with a key down or a click
  waiting, the headless client stands in for that player and runs Fabric's
  `ClientTickEvents` START and END: the mod's `consumeClick()` loop sees the click and, as
  mods do, sends its server half a packet (handed over in-process, phase 5c). Keys are
  released when the player leaves.
- A mod listening to client ticks now counts as listening for the tick backoff (5b).

The runtime test registers a key binding on K through Fabric's own API, finds K in the list
sent to players, holds K over a tick, and counts exactly one click in the mod's tick listener.
All three mod sets pass. HUD overlays drawn by mods (`HudRenderCallback`) are not done yet.

## Phase 5d status: items used, mods' own screens, players leaving (2026-11-12)

- **A right-click with a 1.20.1 mod's item at nothing** runs 1.20.1's own
  `ServerPlayerGameMode.useItem` (`Mirror120.useItem`, `game/java_mods_120/items.rs`), from
  the point in Horizon's use where the 26.3 bridge asks too: the item's own use and Fabric's
  `UseItemCallback`. The stack comes back as the mods left it. A use held over many ticks
  (eating, drawing a bow, raising a shield) is let go and passed back: Horizon runs those for
  the content items it knows.
- **A screen the client half opens itself** (a guide book, a mod's own page) is now the
  player's screen on Horizon, through the same frames and input as a container's: laid out
  at the player's size, drawn each tick, taking clicks, keys and typing, closed by its own
  `onClose`. A click or a use that opened one says so (`screen:<class>`), as for a menu.
  Vanilla's written book opens this way (the server's `ClientboundOpenBookPacket` shows the
  client half's `BookViewScreen`), which is also the test's case.
- **A player leaving** (Horizon dropping a guest) closes their screen there, fires Fabric's
  `ServerPlayConnectionEvents.DISCONNECT`, and the mirror forgets them.
- **More of a server and a client for the mods' code**: the player list's operators, bans and
  whitelist (empty, as a new server's: nobody is an op), and text filtering off, as on an
  offline client.

The runtime test reads a written book with a right-click: its screen opens, draws its page,
and Escape closes it; the reader then leaves, once. All three mod sets pass.

## Phase 5c status: mods' packets, and players joining (2026-11-12)

- **A mod's two halves talk.** In 1.20.1 a mod's server half and client half exchange custom
  payloads: a machine's live numbers to its screen, config to a player who joins, a screen's
  button back to the server. Both halves run in this one runtime (the server half on the
  mirror, the client half in the headless client), so `Packets120` hands each packet to the
  other half's receiver, through Fabric API's own registries of them
  (`ServerNetworkingImpl.PLAY`, `ClientNetworkingImpl.PLAY`), with a copy of its bytes. A
  packet to a player runs the client receiver with the headless client standing in for that
  player; a packet the client half sends runs the server receiver as that player.
- **Where they're caught**: the mirror player's `ServerGamePacketListenerImpl.send` (anything
  the server half sends a player) and the headless client's `ClientPacketListener.send`
  (anything the client half sends); Fabric's `canSend` and `getSender` on both sides answer for
  connections Fabric never set up. Vanilla's own packets to a player stay Horizon's business.
- **Work handed on runs now.** Nearly every receiver hands its work to `Minecraft.execute` or
  `MinecraftServer.execute`; neither runs a loop here, so the work runs on the call that asked
  (`BlockableEventLoopMixin`).
- **Players joining**: Fabric's `ServerPlayConnectionEvents` INIT and JOIN fire when Horizon's
  player first reaches the mods' code, with a sender that reaches their client half: what a
  mod sends a joining player arrives. The mirror player's connection now names its player and
  has a (channel-less) network connection, which Porting Lib asks about on join.
- **A screen the client half opens itself** (`Minecraft.setScreen`, as Porting Lib's
  `open_screen` packet does) is not shown twice: a container's is the one `Screens120` makes
  from the server's menu; another kind is said once and not shown on Horizon yet.

The runtime test sends a player 41 through `ServerPlayNetworking.send`; the client receiver
answers 42 through `ClientPlayNetworking.send`; the server receiver gets it; both `canSend`s
say yes; each receiver hands its work on through `execute`, as mods do. No packet or join
listener fails on Anthony's mods (Farmer's Delight Refabricated, Wired Redstone). On the
15-mod set, TechReborn's ore-depth sync (it asks the world generator, phase 6) and one of
Supplementaries' join listeners still fail, each said once and skipped.

Not done: a mod's own non-container client screens (shown on Horizon), players leaving
(DISCONNECT), and packets the mods' client half sends from its own tick (there is no client
tick yet).

## Phase 5b status: the server starts and ticks for the mods (2026-11-11)

- **Fabric API's server events** fire in the runtime as a real server fires them
  (`Events120`): `ServerLifecycleEvents.SERVER_STARTING`, each level's
  `ServerWorldEvents.LOAD`, `SERVER_STARTED` (once, on the first tick), then every tick
  `ServerTickEvents` START/END of the server tick and of each level's, paced by Horizon's
  server (`game/java_mods_120/ticks.rs`). Wired Redstone's GraphLib ticks its graphs this way.
- **Each mod's listener on its own.** Not through the event's invoker: a listener that throws
  here is said once and skipped in that place, and the next mod's still runs.
- **A quiet mod set costs nothing**: when no mod listens to ticks, Horizon asks again only
  every 5 seconds, to notice a listener registered later.
- **More of a server for the listeners to find**: the server's level list (overworld(),
  getLevel, getAllLevels), its tick count (Horizon's game time: a mod doing something every
  20 ticks does it every second), its player list (Horizon's players, as they act), its folder
  (`getWorldPath`: the runtime's own, where Porting Lib keeps its server configs) and
  server.properties at vanilla's defaults (nothing written).

Checked on three mod sets: no listener fails on Anthony's (Farmer's Delight Refabricated,
Wired Redstone). On the 15-mod set, Supplementaries' server-started listener still fails: it
asks the level's world generator, which is phase 6.

## Phase 5a status: breaking blocks, and where the player stands (2026-11-11)

- **A 1.20.1 mod's block is broken by 1.20.1's own code**: `ServerPlayerGameMode.destroyBlock`
  on the mirror (`Mirror120.breakBlock`, `game/java_mods_120/breaking.rs`), from the same point
  in Horizon's break as the 26.3 bridge's hook. The block's `playerWillDestroy`, its own loot
  table with the real tool (enchantments go down: `stack_to_120`), the tool's wear and Fabric's
  `PlayerBlockBreakEvents` (fired by Fabric itself) all run as in 1.20.1.
- **Any other block** is still Horizon's to break, with `PlayerBlockBreakEvents.BEFORE` (a
  listener may stop it, and then hears `CANCELED`) and `AFTER` around it, so a 1.20.1 mod that
  listens for breaks hears vanilla blocks too.
- **A mod's block hears it is going.** The mirror's `setBlock` now runs the old block's
  `onRemove`, as vanilla's chunk does, for mods' blocks (Horizon runs vanilla blocks' removal
  itself). That is where a container block drops what it holds: breaking a cooking pot with
  five carrots in it drops the carrots and the pot, where before the carrots vanished.
- **The player stands where they stand.** A click put the mirror player's eyes at the point
  clicked, always looking south, so every 1.20.1 block placed faced the same way (all of
  Farmer's Delight's stoves faced north). Clicks and breaks now carry the player's feet, yaw
  and pitch (`Stance`): a stove faces whoever places it.

The runtime test covers both facings, and the pot broken with carrots in it, on both
Farmer's Delights. Not done: a block's `onPlace` on the mirror; the player's hunger from
breaking a mod's block (1.20.1's exhaustion lands on the mirror player).

## Phase 3e status: hoppers feed and empty mods' blocks (2026-11-11)

Horizon's hoppers and droppers move items into and out of a 1.20.1 mod's block the way Fabric
Transfer API's patch of vanilla's hopper does: the block's `ItemStorage.SIDED` on the face the
hopper touches, one item a move (platform120's `Hoppers120`, the 26.3 bridge's
`HorizonHoppers` on 1.20.1's names). The block entity is the mod's own, kept by Horizon as
opaque NBT; the item goes in or out through the mods' code and the NBT comes back changed, as
every call's changes do.

- Horizon's hopper code asks the 26.3 bridge first and then this one
  (`machines/java.rs`, `mod_hopper_insert`/`mod_hopper_extract`), only where a 1.20.1 mod's
  block is (`game/java_mods_120/hoppers.rs`). A face with no item storage (a stove's) is left
  alone for 40 ticks rather than asked every hopper cooldown.
- platform120 compiles against Fabric API's Transfer API now: `build120.sh` remaps the three
  modules it calls (api-base, lookup, transfer) from intermediary to Mojang's names into the
  cache, as Loom remaps a mod's dependencies. Compile-only; nothing of Fabric's is committed.
- Checked in a real game with Farmer's Delight Refabricated: five carrots in a hopper over a
  wooden basket go through the basket into the hopper under it. The runtime test pushes into
  and pulls out of a basket on both Farmer's Delights, and checks a stove takes nothing.

Not done here: the mods' own pipes and machines pushing into Horizon's containers are the
mirror's world view of vanilla block entities, which already crosses; it has not been checked
with a mod that does it. Fluids and energy (Team Reborn Energy) are the next transfer APIs.

## Phase 4 status: a mod's screen draws, shows and is used (2026-11-11)

Right-clicking a Farmer's Delight cooking pot opens its menu, and its own `CookingPotScreen`
draws a frame -- in the runtime, not yet in the game.

- **The menu opens on the mirror unpatched.** The mirror player is made with vanilla's
  constructor, so `ServerPlayer.openMenu` runs as on a server: the menu made, its slots
  listened to. Only the packet telling a client to show it goes nowhere; the title it carried
  is kept by a mixin at openMenu's return (`ServerPlayerMenuMixin`), and the click reports the
  menu's type and title (`Used::opened`).
- **The screen is the mod's own**, built on the server's menu itself through the constructor
  the mod registered (`MenuScreens.SCREENS`, found by shape because its interface is
  package-private and every name here is intermediary), then laid out with vanilla's
  `Screen.init`.
- **Its drawing is recorded, not drawn.** Mixins on `GuiGraphics` take each call at its head
  (`GuiGraphicsMixin`, `Graphics120`) and record it as the operations Horizon's screen drawing
  already reads (`game/java_screen.rs`, shared with the 26.3 bridge). The screen's pose is
  applied, so slots drawn relative to the screen's corner land where they go.

What a headless 1.20.1 client was missing, found one crash at a time:

| The screen asked for | Answer |
|---|---|
| The narrator, on layout | none: `shouldRunNarration()` is false (`ScreenMixin`) |
| The world behind, for the background | always vanilla's in-world darkening, never the title-screen dirt |
| The render thread | `RenderSystem`'s checks pass; its queue for that thread is dropped (`RenderSystemMixin`) |
| OpenGL | every void `GlStateManager` call stopped -- all 80, generated from the mappings (`GlStateManagerMixin`) |
| The blocks atlas, for an empty slot's outline | stand-in sprites that know only their name, drawn by Horizon from its own atlas (`Sprites120`) |
| A font, to centre a title | one that measures with 1.20.1's default ASCII widths and draws nothing (`Screens120.font`) |

The frame, checked by the test against real Farmer's Delight jars: the pot's texture at
(125, 37) on a 427x240 GUI -- exactly where vanilla centres a 176x166 container -- the bowl's
outline in the container slot, the title in vanilla's label grey, and behind it the darkened
world in vanilla's own colours.

### Part 5: on the player's screen, and used (2026-11-11)

The frame now reaches the player's game, and the player's clicks reach the screen. (Tested
in the runtime against real jars; the in-game path is built and compiles, and its first check
in a real game is pending.)

- **Shown with the 26.3 bridge's screen code.** A click that opens a mod's menu
  (`Used::opened`) registers the screen for that player's connection
  (`game/java_mods_120/screens.rs`); every server tick its frame is drawn and, when it changed,
  sent on `horizon:java_screen` as a client-only screen (window -1). The player's game shows it
  with `game/java_screen.rs` exactly as it shows a 26.3 mod's: nothing new on the client. The
  recorder's rectangles became corners (`x0 y0 x1 y1`) to match what that code reads.
- **Input runs through the screen's own handlers.** Clicks, releases, drags, scrolls, keys and
  typed characters come back on `horizon:java_screen_input`, converted to 1.20.1's terms (GLFW
  key codes from the SDL scancodes 26.3 sends, GLFW buttons and modifier bits), and go to
  `mouseClicked`, `keyPressed` and the rest (`Screens120.input`). A slot click, which a client
  sends to the server as a packet, runs the server's `AbstractContainerMenu.clicked` directly
  (`AbstractContainerScreenMixin`) -- the menu the screen draws *is* the server's.
- **The inventory travels with each call.** The player's 36 slots go to the runtime with every
  frame and input (by item, count and damage; a slot already holding that stack keeps its NBT),
  and the slots an input changed come back as 1.20.1 NBT, through vanilla's fixes, so an
  enchanted tool shift-clicked out of a machine keeps its enchantments. What the cursor
  carries lives in the runtime's menu, and goes back into the inventory when the menu closes,
  as a server's does.
- **Closing is the screen's.** Escape and the inventory key reach the screen, whose `onClose`
  asks to close (the mixin takes it, as a client's would send the close packet); the menu then
  closes as `ServerPlayer.doCloseContainer` closes it. A frame also checks `stillValid` with
  the player where Horizon has them: walk away, or break the pot, and it closes.
- **More of the headless client:** real `Options` (built from a file that isn't there: every
  key binding at its default), an allocated `Window` for the one question asked of it, and
  `InputConstants.isKeyDown` answered from the input's modifiers, so a shift-click is one.
- **Counts and tooltips.** `renderItemDecorations` is recorded as the `decorations` operation
  (the count, or the text the screen asked for), and every `renderTooltip` overload as
  `tooltip` lines (component JSON) or `item_tooltip` (the stack, for Horizon to write).
  A mod whose tooltip code fails here -- it reads the client's player, which this client
  hasn't got -- shows the item's name instead of losing the frame.
- **Empty-slot outlines** (`textures/item/empty_*.png`, drawn by 1.20.1 from the blocks atlas)
  are packed into Horizon's screen atlas, so the pot's bowl outline can be drawn.
- **A client player, for the recipe book.** Farmer's Delight Refabricated's pot screen (and
  every furnace-like machine screen) lays out vanilla's recipe book, which reads the client's
  own player: its recipe book, inventory and open menu. The headless client now has one,
  allocated once with an empty recipe book of its own and the server player's inventory and
  menus (`Screens120.clientPlayer`). The book shows no recipes: the player's unlocked recipes
  don't cross yet.
- **A title without `openMenu`.** Porting Lib's ports open their menus with their own extended
  openMenu, which never passes vanilla's, so the title comes from the block entity clicked
  when it names its menu (`MenuProvider.getDisplayName`).

The test, on real Farmer's Delight jars (1.4.3 and Refabricated 2.5.7; the test's jar lookup
missed Refabricated's file name at first and skipped it, so it ran on 1.4.3 only until the
lookup was fixed): the hotbar's carrots
and potatoes drawn in their slots 18 pixels apart, the count over the five carrots and their
tooltip with the mouse on them; a shift-click moves the carrots into the pot (the block entity comes
back holding them, the hotbar slot empty); a click picks the potatoes up and the next frame
draws them under the mouse; Escape closes the screen and puts them back in the inventory; and
a reopened pot closes once the player is out of reach.

**Not done here:** a tooltip's image (a bundle's contents, AppleSkin's hunger bar) is lost;
a shader colour set with `setColor` is lost; text is laid out with
approximate widths; a stack's components other than damage don't reach the runtime (an
enchanted tool put *into* a machine loses its enchantments); what the cursor holds is lost if
the server stops with the screen open. Those are the rest of phase 4.

## Phase 3d status: the shape only the mod knows (2026-11-10)

A block's shape is one per state in Horizon's registry, exported by asking each state with no
world and no block entity around it. That is the whole truth for a vanilla block and none of it
for a block whose shape is its block entity's: **LibMultiPart's container answers with no shape
at all** until there are parts inside it, so all 3072 of its states were recorded as shapeless.
A Wired Redstone cable placed in the world could not be looked at, aimed at or mined -- the ray
went straight through it -- while the block itself was drawn as its template, `minecraft:stone`.
An indestructible ghost stone cube.

So the shape travels from where it is known:

- `Mirror120.blockShapes(dimension, positions)` asks the mods, at the position, with the block
  entity in place: `state.getShape(level, pos)`. A bundled cable answers
  `[0.125, 0, 0.125, 0.875, 0.25, 0.875]` -- the plate it is. Batched like `blockStates`, and a
  mod that throws leaves the registry's shape in place (said once per position).
- The answer comes back over the wire as a flat `double[]` (a count, then that many boxes of
  six), which `Value::F64s` carries: the wire's first new value kind since it was written.
- Horizon keeps it on the block entity as the synced attachment `horizon:block_shape`
  (`game/mod_block_shape.rs`), set whenever a call of the mods' leaves a block entity changed
  (`refresh_java120_shapes`). Attachments already go to every client that can see the holder
  and are saved with the chunk, so a reloaded world has the shape before the mod has ticked.
- The aiming reads it first (`Game::outline_boxes`): a window reads what the server synced, a
  server its own block entities. No attachment means no mod has spoken for that block and the
  registry's shape is the whole answer, as for every vanilla block. An empty list is an answer:
  the mod says there is nothing to aim at.

**Not done here:** collision still comes from the registry. For a cable that is right (you walk
through one in vanilla too) and for a mod's machine the registry already has its real shape,
since those do not depend on a block entity -- but a mod block that *is* solid only when its
block entity says so will be walked through. And the block is still drawn as its template's
model: drawing a mod's own geometry is phase 7.

## Phase 3b status: block entities cross the mirror (2026-11-09)

A block entity a mod reads is Horizon's, and what it changes comes back:

- **Reads** (`Level.getBlockEntity`) ask Horizon (`blockEntity`, answered by `World120`) for the
  block entity as vanilla saves it (`saveWithFullMetadata`, SNBT) and load it as a chunk does
  (`BlockEntity.loadStatic`), so a mod's own type is made by the mod's own code from its own
  NBT. One position gives **one object for the whole call**: mods keep what `getBlockEntity`
  gave them, change it in place, and some compare it by identity.
- **Writes** go back as `setBlockEntity` events when the call ends, and only for one whose NBT
  the call actually changed (compared as this side writes it, so a difference is the mod's
  change and not another serializer's spelling). `Level.setBlockEntity` and `removeBlockEntity`
  land in the same set.
- A mod's block entity needs **no type in Horizon**: `decode_block_entity` keeps a non-vanilla
  id as `BlockEntity::Mod` (the compound opaque, with `Items` decoded into real stacks), so a
  cooking pot's contents save, reload and stay visible to Horizon's own hoppers.
- `setBlock` makes the block entity the new block has (vanilla's `LevelChunk.setBlockState`
  does, and a mirror has no chunks), and **keeps the one already there** when the new state is
  still its block: a mod sets a block from inside its own block entity's code (a jukebox turns
  itself `has_record=true` while holding the record it was just given), and a fresh one would
  lose the change.
- The mirror's **chunk source answers "no chunk"** rather than throwing
  (`ServerChunkCacheMixin`). It is made without its constructor, so its chunk map is null, and
  vanilla asks it questions on the way through code that has nothing to do with chunks:
  `BlockEntity.setChanged` alone asks `hasChunkAt` twice. Callers then take their "not loaded"
  path, which is the one that leaves the world to Horizon.

Tested (`tests/boot120.rs`): a jukebox is Horizon's block entity read by 1.20.1's own code and
changed by it, with no menu or mod in the way — `RecordItem.useOn` asks the level for it and
sets the disc on it, the disc is used up, and the block entity comes back holding it.

Phase 3b also made the mirror survive what a constructor-less level lacks, each found by its
own crash on the way to a block being placed:

- Its **chunk source answers "no chunk"** rather than throwing (`ServerChunkCacheMixin`): its
  chunk map is null, and vanilla asks it questions on paths that have nothing to do with
  chunks (`BlockEntity.setChanged` asks `hasChunkAt` twice).
- Its **profiler records nothing**, answered on `MinecraftServer` and not only on `Level`:
  mods merge an override of `ServerLevel.getProfiler` that delegates to
  `getServer().getProfiler()`, so a fix on `Level` is never consulted for a `ServerLevel`.
  It matters outside profiling because `Level.getEntities` opens with
  `getProfiler().incrementCounter("getEntities")`.
- Its **entities are an empty getter** (`NoEntities`), since the entity manager is never made.
  `ServerLevel.getEntities` needs its descriptor spelled out: the class has several overloads,
  and an inject by bare name binds to another one.

Both answers are ours rather than vanilla's `InactiveProfiler.INSTANCE`, which reads as null in
the child even though the bytecode asks for the right field.

**Why a click can miss the runtime**, which was invisible and cost a day of misreadings: it
passes when this side is a client (the server's side takes it), when neither the block nor the
held item is a 1.20.1 mod's, while the runtime is still starting, and — until now — whenever
another call held it. That last one is gone: a click waits up to 50 ms for a busy runtime and
says so if it gives up, because a click that quietly does nothing reads as a broken mod.
`HORIZON_120_TRACE=1` says where each click went.

**What a mod drops or ejects** now arrives: `ServerLevel.addFreshEntity` on a mirror level
sends an item back as a `spawnItem` event and Horizon spawns it, since Horizon owns this
world's entities (a chest spilling its contents, a machine putting its output out). Anything
that isn't an item is turned away, said once per kind, until entities are a phase of their own.
The stack crosses **as the runtime saved it** (`ItemStack.save`, in SNBT), not as a name and a
count, and Horizon reads it with vanilla's own fixes from 1.20.1's data version (3465, the
`world_version` of Mojang's jar) to its own: `datafix::update(Ty::ItemStack, ...)`. The step
that does the work is 1.20.5's `ItemStackComponentizationFix`, which is exactly the 1.20.1
`tag` → components conversion the downgrade direction calls for, and the chain already carries
every item fix after it. So an enchanted, renamed tool a machine ejects arrives enchanted and
renamed, with nothing hand-written: this direction was already in the tree.

The other direction -- Horizon's 26.3 items shown *to* 1.20.1 code -- still is not, so a held
stack crosses as its id, count and damage, and Horizon's own `extra` is kept across a call
rather than shown to the mods.

Tested: the record goes back out of the jukebox (`JukeboxBlock.use` → `dropRecording` →
`addFreshEntity`), and comes back as an item for Horizon to spawn.

**Their block entities tick** (phase 3c): every server tick, Horizon hands the runtime the
positions of the block entities it keeps as a mod's own (`BlockEntity::Mod`), and each one's
block ticker runs once on the mirror -- a cooking pot cooks, a machine runs. What the tick set,
changed and dropped comes back the same way a right-click's does (`apply_java120`).

They **stay alive between calls**, as the 26.3 bridge's do (`HorizonBlockEntities.LIVE`): one
position gives one object until the block there becomes something else
(`BlockEntityType.isValid` against the state Horizon has now, and the state is passed on when
it moves under it). Only a block entity whose NBT actually changed goes back, and the baseline
moves with it, so an idle machine costs nothing after its first tick.

What that settles, and what it costs. A machine ticking twenty times a second is no longer
read from Horizon and written back on each tick, and whatever a mod keeps in a field it never
saves survives between them. In exchange **the live object is the truth while it lives**:
Horizon's NBT is read when there is no live one (a fresh start, or the block changed), so
something else in Horizon editing a mod's block entity behind the runtime's back would be
overwritten. Nothing does that today -- Horizon keeps that NBT but never writes it except
from here -- and the 26.3 side holds the same shape.

A tick that throws is said once and that block entity type is not ticked again: a tick runs
twenty times a second, and a mod whose tick fails fails every one of them. A runtime busy with
another call skips the tick rather than waiting (nobody is waiting on a tick, unlike a click).

Tested: a lit furnace with fuel and something to smelt burns a tick of fuel and cooks for a
tick -- 1.20.1's own ticking code, needing the block entity loaded from Horizon, the recipes
bound, and the change sent back, with no mod in the way. Ticked again against the same
untouched NBT, it reaches its second tick rather than its first again, which is what tells a
live block entity from one read afresh each time.

**Reads are batched and kept for a call** (the chunk mirror's first half): a read costs a round
trip whatever it carries, and the code on the other side reads the same position more than once
and its neighbours besides -- a block's `use` asks for its state, `setBlock` asks again to see
whether it changed, and a pipe or a crop asks what is around it. The first read of a position
now fetches the 3x3x3 around it in one call (`blockStates`, many positions at once) and keeps
what came back for the length of the call, which is as long as it is good for: within one call
Horizon's world changes only where this side writes, and a write is read from `WRITES` first.

Measured on one stove click, with `Peer`'s own `calls_in`: **17 calls before, 7 with the cache
alone, 4 with the prefetch as well.** The test asserts the bound, so a future change that goes
back to a round trip per read fails it rather than quietly costing four times as much.

What it does not do yet: whole sections. A mod walking a large area still pays a call per 3x3x3
it touches, and the deltas that would let Horizon push changes instead of answering questions
are still to come.

**Telling the player, and telling the players watching**, both of which a mod does as a matter
of course and neither of which a mirror could survive:

- A mirror player now has a **packet listener that swallows** what is sent to it
  (`ServerGamePacketListenerMixin`). A mod telling "the player" about something sends through
  there, and vanilla's send threw on the connection a mirror player has not got. Horizon owns
  the real players and tells them about the world itself; mod packets reaching a player are
  phase 5.
- The mirror's chunk source now holds an **empty chunk storage** (`ChunkMapAccess`). Mods reach
  that field through their own mixins to ask who is watching a chunk before sending a block
  entity's network update -- LibNetworkStack does, so Wired Redstone does when it places a
  cable -- and a null one threw inside the mod, where nothing here could answer. Empty maps
  make the lookup find nobody, which is the truth.

Behind those came two more, each found by the one before it being fixed:

- Every position's lookup answers with **one chunk holder that holds no chunk**
  (`ChunkHolderMixin`, through the maps' default return value so either route finds it). An
  absent holder was not enough: LibNetworkStack asks for one and calls `getTickingChunk()`
  straight on the answer.
- `Level.neighborShapeChanged` and `updateNeighborsAtExceptFromFacing` are **quieted**: both go
  through `Level`'s own neighbour updater, which a mirror never made. Neighbour updates are
  Horizon's to run, as the rest of them already were; these two were simply missed, and
  LibMultipart reaches the first of them when it adds a part.

Found by placing a Wired Redstone cable, which needs all of them. Tested twice over. With vanilla's chest, which is the same shape with no mod in the way: its
`use` opens a menu (a packet through the listener) and angers nearby piglins (the level's
entities), and both threw before; checked by taking the fix out again and watching the test
reproduce the reported crash. And with the mod itself: a bundled cable placed on grass comes
back as a `libmultipart:container` block whose block entity holds the cable part, with one
taken from the stack.

A mod item Horizon names that this runtime hasn't got is now **said once per id**: the registry
answers an unknown id with air, so such a click used to arrive empty-handed with no sign of
why.

Next: menus (phase 4), which need the `GuiGraphics` recorder for a mod's own screen -- the 26.3
bridge's `java_open_menu` shows vanilla's menus only, and says so.

## How the bridge is laid out (Phase 0)

| Part | Size | What it is |
|---|---|---|
| `crates/horizon-jvm/java/dev/horizonlab/bridge/` | ~900 lines | Boot core: `Boot`, `Natives` (46 JNI natives), `fabric/FabricBoot`, `fabric/HorizonGameProvider`, `c112/Boot112`, `neo/NeoBoot` |
| `crates/horizon-jvm/java/platform/` | ~30,600 lines, ~75 Mixins | The 26.3 platform mod: mirror server/levels/client, block entities, entities, screens, GPU, worldgen |
| `crates/horizon-jvm/java/platform112/` | ~800 lines | Forge 1.12 platform (phase 1: boot + report) |
| `crates/horizon-jvm/src/` | ~7,500 lines Rust | JNI, events, content, `boot112.rs`, `neoforge.rs` |
| `crates/horizon-world/src/datafix/` | | Every vanilla server datafix from 1.8 to 26.3, ported to Rust |

### Reusable as is

- **The protocol's data.** `Natives.java` passes strings, SNBT and JSON, not Minecraft objects.
  The transport carries exactly these types, so the Rust handlers serve both runtimes once the
  data is converted.
- **Datafix upgrade 1.20.1 → 26.3.** `horizon-world/src/datafix` runs every fix from DataVersion 99
  up, including 1.20.5's item NBT → data components (`fixes_a.rs`, keys 3564–3833).
- **Horizon's drawing of screens** (`java_screen.rs`), content conversion (`horizon-port-pack`),
  entity snapshots and their client-side lerping.

### Needs changes

1. ~~Obfuscation.~~ **Done:** Fabric's own Minecraft provider remaps at boot; `Names120` gives
   the bridge's code Mojang's names.
2. ~~Reflection by Mojang names.~~ **Done:** `Boot120` via `Names120`.
3. ~~Fabric API.~~ **Done:** 0.92.12+1.20.1, pinned in `fabric120`.
4. **The platform mod (the bulk of the work).** The 26.3 platform's ~75 Mixins have to be redone
   against 1.20.1 as `platform120` (for example, 26.3's `GuiGraphicsExtractor` vs 1.20.1's
   `GuiGraphics`). It's written with Mojang's names and remapped to intermediary at build time,
   as Loom does.
5. **The downgrade direction (new code).** Datafix only *upgrades*. The mirror also has to show
   Horizon's 26.3 world to 1.20.1 code: placeholder blocks for blocks newer than 1.20.1; items
   from 26.3 components to 1.20.1 NBT, with the original components kept in a hidden key so a
   stack survives the round trip; vanilla block entities that mods touch, to 1.20.1 SNBT and back.
6. **In-world rendering.** Capture at `VertexConsumer` / `MultiBufferSource`. Mods with custom GL
   shaders need a GL recorder (like the 1.12 plan's phase 7), or stay unsupported.
7. ~~Java version.~~ **Measured:** 1.20.1 and the 14 mods run on Java 25 with launcher options.

## Mods that need an older Fabric Loader (e.g. stuck on 0.16.4)

The 1.20.1 child runs one Fabric Loader for all its mods (the newest, 0.20.1; Fabric API
0.92.12+1.20.1 needs ≥ 0.16.10). The report already lists each mod's declared `fabricloader`
range. There are four reasons a mod only runs on an older loader, each with its own fix,
cheapest first:

1. **Only the declared range blocks it** (`"fabricloader": "<=0.16.4"`), and the code works.
   Fix: Fabric Loader's own *dependency overrides* (`config/fabric_loader_dependencies.json`),
   which change a mod's declared dependencies without touching its jar. The bridge writes them
   from a curated table plus the player's own entries, and the report lists each override.
2. **Mixin behaves differently on the newer loader.** The loader already gives each mod the
   Mixin behaviour of the loader version it declares. What's left gets measured in the tracker.
3. **The mod calls loader internals that moved** (`net.fabricmc.loader.impl.*`). Fix: a
   `loader-compat` layer in the child that brings back the old classes as thin forwards. Where
   names clash, the mod's references are redirected in memory as it loads (a Java agent:
   Horizon owns the child's command line). The jar on disk stays unmodified.
4. **Last resort: an older loader for that pack.** Each runtime is its own process, so a pack
   that truly needs 0.16.4 runs its child on 0.16.4. `Boot120` keeps to loader entry points
   both versions share (Knot's constructor and `init`, `prepareModInit`, the public
   `FabricLoader` API), all by reflection, so this stays possible.

**Status:** reason 1 is fixed automatically (Phase 1b status). `scan120.py` sorts each stuck
mod into reasons 1–4 before any fix is written, and found a real reason-3 case (Rhino).

## Upstream

Nothing about 1.20.1 is in `docs/`, `roadmap/` or `ROADMAP.md`. Before the platform work, open
an issue with the author. `horizon-jvm-remote` is useful beyond 1.20.1: it's how Forge 1.12 or
NeoForge could run beside Fabric 26.3 rather than in its place.

## Phases

0. **Study.** Done.
1. **boot120 + transport.** Done (above).
   - 1b: done: loader-range overrides, `scan120.py`, the first tracker (above). Left from it:
     the `loader-compat` shim (start: Rhino's two calls), and the child's Java chosen by the
     scan (`java<=21` → a Java 21 child) instead of by hand.
   - 1c: Horizon's game starts the child when `mods-java-1.20.1/` holds jars (the gate,
     `game/java_*.rs`), and the report feeds `horizon-port-pack`.
2. **placed120.** Done for the 14-mod set (Phase 2 status): blocks, items, models, textures,
   recipes, loot and tags. **Milestone met:** 1.20.1 mod blocks placed, drawn, saved and
   reloaded; they drop themselves and their recipes craft. Next here: more mods through it
   (the top-150 list), and Horizon's recipe book showing content mods' recipes.
3. **machine120.** 3a done: right-clicks run the mods' code on a mirror `ServerLevel`. 3b done:
   block entities cross both ways, a mod's own kept as opaque NBT, and what a mod drops or
   ejects reaches Horizon, and their block entities tick and stay alive between ticks (all
   above). 3e done: Horizon's hoppers and droppers feed and empty mods' blocks through the
   Transfer API (above). Next: the chunk mirror and batched deltas ("Measured"), the
   downgrade tables, fluids and energy.
4. **gui120.** Started (Phase 4 status): menus open, a mod's own screen is shown in the game and
   takes clicks and keys, with counts and tooltips. Next: the first check in a real game,
   tooltip images, `setColor` tints, real text widths, a stack's components into the runtime.
5. **Events and networking.** 5a done: block breaks. 5b done: lifecycle and tick events. 5c
   done: mods' packets between their halves, players joining. 5d done: items used, mods' own
   screens, players leaving. 5e done: mods' key bindings and the client tick (above). 5f/5g done:
   sounds and particles. 5h done: the mods' settings screens. 5i done: the mods' HUD (above). 5j done: their
   commands and messages. 5k done: their effects and food. 5l done: their villager trades. 5m done: their tools, weapons and armour. 5n done: their tooltip lines. 6e done: their fluids flow and are drawn. Left: the HUD overlays that read the client's world (Jade, minimaps).
6. **Entities and worldgen.** 6a done: the mods' Fabric biome modifications reach Horizon's
   generator as biome modifiers. 6b done: a mod's own feature types run in the runtime on the
   chunks being decorated. 6c done: data-driven structures (above). 6d done: the mods' mobs
   (above). 6g done: mods that replace vanilla's world generation as data (Terralith, above).
   6f done: the mods' data-driven dimensions as levels (above; their own carvers and portals
   not yet). Next: structures with their own Java pieces. Biomes added by code
   (TerraBlender) are left to the 26.3 bridge (above).
7. **In-world rendering.** Blocks drawn by their code done (above: Wired Redstone's wires and
   cables); block entity renderers done (7b: Farmer's Delight's cutting board). Next:
   LibMultipart's dynamic part renderers, then a GL recorder if needed.
8. **Tracker.** Started with phase 1b (boots: 149 of the top 150). It grows into "works in a
   world" as the phases land: works / partial / broken / needs Java 21 / needs an older loader.
