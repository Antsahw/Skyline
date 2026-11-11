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
5. **Events and networking.** Fabric API 1.20.1 events at the bridge's hook points, mod packets.
6. **Entities and worldgen.** Data-driven registries from a real server load (mods' biomes
   through Fabric's biome API show up only there).
7. **In-world rendering.** `VertexConsumer` capture, then a GL recorder if needed.
8. **Tracker.** Started with phase 1b (boots: 149 of the top 150). It grows into "works in a
   world" as the phases land: works / partial / broken / needs Java 21 / needs an older loader.
