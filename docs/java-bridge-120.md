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

**First step (phase 1b):** `scan120.py`, like `scan112.py`: per mod, its `fabricloader` /
`fabric-api` ranges and every reference to loader internals in its class files. Together with
the report, that sorts each stuck mod into reason 1–4 before any fix is written.

## Upstream

Nothing about 1.20.1 is in `docs/`, `roadmap/` or `ROADMAP.md`. Before the platform work, open
an issue with the author. `horizon-jvm-remote` is useful beyond 1.20.1: it's how Forge 1.12 or
NeoForge could run beside Fabric 26.3 rather than in its place.

## Phases

0. **Study.** Done.
1. **boot120 + transport.** Done (above).
   - 1b: `scan120.py`, the dependency-override writer, a first loader-compat table.
   - 1c: Horizon's game starts the child when `mods-java-1.20.1/` holds jars (the gate,
     `game/java_*.rs`), and the report feeds `horizon-port-pack`.
2. **placed120.** Content conversion for blocks, items, models, sprites. **Milestone:** a 1.20.1
   mod block is placed, drawn, saved and reloaded.
3. **machine120.** Mirror 1.20.1 `ServerLevel` with the chunk mirror and batched deltas
   ("Measured"), block entities, the downgrade tables, the Fabric Transfer API on Horizon's
   containers.
4. **gui120.** `GuiGraphics` recorder → `screenFrame` JSON, menus, input, slot clicks (its own lane).
5. **Events and networking.** Fabric API 1.20.1 events at the bridge's hook points, mod packets.
6. **Entities and worldgen.** Data-driven registries from a real server load (mods' biomes
   through Fabric's biome API show up only there).
7. **In-world rendering.** `VertexConsumer` capture, then a GL recorder if needed.
8. **Tracker.** The top 60 Fabric 1.20.1 mods: works / partial / broken / needs Java 17 /
   needs an older loader.
