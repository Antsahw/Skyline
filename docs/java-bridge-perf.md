# Java bridge performance: the 81-jar test pack (2026-11-05, bridge-perf-1)

What a big Fabric pack costs Horizon: start time, memory, server tick time, and where they go.
Measured headless with `tools/java_bridge/perf_measure.sh`. Start with this page before tuning.

**Machine:** cloud container, 2 shared cores, 7.6 GB RAM, no GPU: Horizon's renderer and the mods'
GPU device (the headless client's) both run on **llvmpipe**, on the same 2 cores. A desktop is
faster everywhere, and much faster for the mods' GPU work (see "HUD" below). Compare runs on one
machine, not across machines.

**Pack:** `tools/java_bridge/get_test_pack.py` → 81 jars (72 run, 166 mods with nested jars).
Seed 1234, creative. Each run: join, stand 60 s, sprint forward 30 s, stand 20 s.

```sh
MODS=~/hz-test/mods-java CACHE=/tmp/pack-cache tools/java_bridge/perf_measure.sh pack   # the pack
MODS=~/hz-test/fapi tools/java_bridge/perf_measure.sh fapi                               # Fabric API alone
tools/java_bridge/perf_measure.sh vanilla                                                # no Java (JAVA_MODS=0)
```

It writes `/tmp/hz-perf/<name>/`: `/tick query` at each phase (`*.tick`), RSS (`*.status`), the
JVM's heap (`*.heap`) and native memory by area (`*.nmt`, `-XX:NativeMemoryTracking=summary`),
the live heap's classes after a full GC (`histo.txt`), the GC log, a JFR recording (`perf.jfr`:
CPU and native samples, allocations), the game's log with `HORIZON_JAVA_PROFILE=1`, screenshots.
jcmd attaches through `-XX:+StartAttachListener` (the bridge's JVM runs with `-Xrs`, so SIGQUIT,
which attach uses otherwise, is ignored). `launch.sh` now takes `JAVA_MODS=0` (the Java side off).

## The numbers

| | Vanilla Horizon | Fabric API alone | Pack, before | Pack, with the tap fix | Pack, fix + `-Xmx3g` |
|---|---|---|---|---|---|
| Start to joined | 10 s | 43–47 s | 182 s cold (content made: 52 s), 196 s warm | 209 s | 199 s |
| RSS standing / after walking | 1.49 / 1.63 GB | 2.26 / 2.45 GB | 4.42 / 4.24 GB | 4.06 / 4.18 GB | 3.93 / 4.95 GB |
| JVM committed (NMT) | — | 0.71 GB | 2.34–2.57 GB | 2.11–2.42 GB | — |
| Java heap: max / live after full GC | — | 1.5 GB / ~0.28 GB | 1.5 GB / 0.64–0.67 GB | 1.5 GB / 0.72–0.91 GB | 3 GB / 0.91–0.96 GB |
| Native "Other" (direct buffers) | — | ~0 | 250 → **795 MB** | **5 MB** | 5 MB |
| Tick avg / P95 / P99, standing (60 s) | 11.5 / 18 / 29 ms | 16.6 / 29 / 86 ms | 169 / 499 / 701 ms | **129** / 249 / 354 ms | 114 / 222 / 363 ms |
| Tick avg / P95 / P99, sprinting (30 s) | 12.4 / 22 / 25 ms | 19.5 / 33 / 49 ms | 243 / 654 / **3060** ms | **155** / 293 / **2080** ms | 114 / 212 / 288 ms |
| Tick avg / P95 / P99, standing after | 12.9 / 20 / 24 ms | 16.8 / 26 / 34 ms | 192 / 554 / 1019 ms | **108** / 211 / 293 ms | 100 / 215 / 237 ms |
| GC pauses (JFR window, ~2.5 min) | — | — | 158, 9.2 s, max 1.98 s | 198, 11.0 s, max 1.85 s | 108, 6.7 s, max 0.23 s |

`/tick query` covers the last 100 ticks of each phase. On 2 cores everything competes: the noise between two runs of the same jars is ±15% on tick
averages. RSS minus the JVM's committed memory is Horizon's own side (Rust, llvmpipe, the mods'
content, LWJGL's untracked mallocs): ~1.5 GB vanilla, ~1.7–1.8 GB with the pack.

## Where the server tick goes (pack, standing, after the fix)

`HORIZON_JAVA_PROFILE=1`, per server tick (`Java profile (mirror tick)`):

- **The headless client: 110–115 ms.** Its HUD (`client: HUD`) 50–78 ms, its tick
  (`client: tick`, the mods' client ticks) 19–26 ms, frame end ~6 ms.
- Mod entities 7.5 ms, their snapshot 4.3 ms, spawner 1.8 ms, the rest <1 ms each.
- Walking adds the mods' worldgen, called on the server thread: TERRAIN 75–118 ms a tick
  (145–675 ms a chunk), structure starts and references 34–100 ms, features 9–42 ms.

JFR on the server thread (Java and native samples):

- **HUD:** Java side ~40% of the client's samples. `HorizonGraphics.shrink` (downsizing a
  window-sized GPU target before sending it) is the biggest leaf. Then gson writing the frame,
  Base64 of image pixels, Xaero's minimap, JourneyMap, MiniHUD and Jade. Native side: the mods'
  GPU passes (`Gpu.endPass`, `clear`) and readbacks (`Gpu.readTexture`), here on llvmpipe, i.e.
  CPU. **This needs measuring on a real GPU** before anyone optimises it.
- **VoxelMap: ~16% of the server thread's Java time.** Its `MapChunk.isSurroundedByLoaded` asks
  for client chunks. Each answer is a `HorizonChunks` copy filled whole (98k blocks, lithium's
  state tracking on each set), refilled every time Horizon's chunk revision changes (any block
  set: random ticks, grass).
- Allocation: 37% of the pack's allocation pressure is vanilla's own `WorldGenRegion` constructor
  (`EnvironmentAttributeSystem` baked per step, all biomes' layers). Vanilla does the same per
  step. Its biome layer holds the region's `BiomeManager`, so it can't be shared per level.

## Where the memory goes (pack)

- **The JVM: 2.1–2.6 GB committed.** Java heap 1.2–1.5 GB committed (the cap is 25% of RAM:
  1.5 GB here), metaspace 212 MB, the CDS archive 228 MB (mapped from its file), code 86 MB,
  GC structures 66 MB, symbols 44 MB, class space 51 MB. JFR's own buffers (53 MB) are
  measurement overhead.
- **Live heap: 0.64–0.96 GB.** By package, shallow sizes after a full GC (the arrays belong to
  all of these):
  - The data fixer: **36–220 MB**, the biggest swing. Up to 6 rule chains for mods' old
    structure files are kept (`HorizonDataFixer.MAX_RULES`), ~35 MB each.
  - Client models: ~37 MB (BakedQuads ~26 MB: 357k quads, ModelPart vertices). The model
    bake keeps them for the mods' renderers and item models.
  - Structure templates: ~42 MB (StructureBlockInfo 20 MB + their BlockPos). The template
    manager's cache of every template loaded.
  - Mixin's class nodes: ASM 28 MB + Mixin 8 MB. Mixin keeps them, as in any Fabric game.
  - Block states and shapes: ~36 MB. Strings: ~25 MB (deduplicated).
  - Fabric API alone: ~0.28 GB live, of which client models 19 MB.
- **The mirror's chunks: small when idle** (PalettedContainers ~10 MB live). They churn: chunk
  copies are refilled whole on every revision.
- The JVM's heap uses a ceiling, not a target. Churn (worldgen regions, chunk copies, HUD frames
  as JSON strings) fills the 1.5 GB cap within minutes, and G1 then runs a **full compaction:
  1.85–1.98 s, in every run of the pack**. That is the P99 above.

## Where the start goes (pack, warm content cache, 196 s)

| Phase | Time |
|---|---|
| Horizon's content and first frame (mods' content from the cache) | 19 s |
| JVM + Knot + mixins (`Minecraft bootstrap done in 19.7 s`) | ~28 s |
| Mods' entrypoints (Fabric loader started: 166 mods) | ~26 s |
| Server data (5346 recipes, 4738 advancements) | 10 s |
| **Spawn area generated through Java** (TerraBlender, BOP: the overworld is Java's) | ~85 s |
| Headless client: atlases and model bake before the join | ~22 s |

Cold adds the content conversion (52 s), but here it overlapped. Cold and warm joined within
the noise (182 vs 196 s). The CDS archive is written at the first run's end and used after.

## Fixed in this branch

1. **The packet tap kept a broken packet's bytes for good** (`HorizonPacketTap.dropLeftover`).
   The tap runs when a mod mixes into `Connection`. In this pack that is FrozenLib's QSL
   `ConnectionMixin`, a registry-sync patch, not a recorder. A packet the decoder couldn't read
   whole (here a >1 MiB payload: "Payload may not be larger than 1048576 bytes") left its 2.3 MB
   rest in the `ByteToMessageDecoder`. Every later packet was appended to it: a new zeroed direct
   buffer of megabytes per packet. That was 31% of the server thread's Java time
   (`SegmentFactories.initNativeMemory`), up to 795 MB of native memory, and every later packet
   misread for the mods ("found 2325608 bytes extra", identifiers made of pixel data). Now each
   body's leftover is skipped (each body is one whole packet). Native "Other" 795 → 5 MB,
   average ticks 169/243/192 → 129/155/108 ms (standing, sprinting, after). ReplayMod's recording still passes (`regress.py replaymod`).
2. **HUD sprite lookups searched every jar each frame** (`HorizonSprites.texture`). A "directory"
   atlas source asks the class loader, which walks all 81 jars' zip indexes. This ran per sprite
   per frame: ~5% of the HUD's Java time. Now each sprite's answer is kept (the jars don't
   change while the game runs).
3. `HORIZON_JAVA_TAP=0` turns the tap off, for a pack whose connection mixins aren't recorders.
   The tap stays on by default (no change). After fix 1 the tap itself costs little here
   (tap off: 145/143/117 ms against 129/155/108, within the noise).

## The 3 biggest cheap wins (next)

1. **Raise the JVM's heap ceiling for big packs** (not done: it trades memory). With `-Xmx3g`
   the 1.9 s full compaction never happened: sprinting P99 2080 → 288 ms, GC pauses 11.0 → 6.7 s, max
   1.85 s → 0.23 s. The cost is +0.5–0.8 GB RSS at peak, given back as the heap shrinks
   (`MaxHeapFreeRatio=30`). A floor of 3 GB (or `-XX:MaxRAMPercentage=40`) when RAM ≥ 8 GB
   would do it. A player can set `HORIZON_JVM_OPTIONS=-Xmx3g` today. Pair it with win 2, so the
   extra room goes to churn, not caches.
2. **Keep fewer data-fixer chains** (`HorizonDataFixer.MAX_RULES` 6 → 1 or 2). Its own comment
   says a chain is rarely used twice. The live heap swung 36 → 220 MB from this alone. One
   constant. Check first how long rebuilding a chain takes during worldgen (the spawn area's
   85 s).
3. **Refill VoxelMap-style client chunk copies less.** `HorizonChunks.held` refills a whole copy
   (98k blocks) on any revision change. Following the change (as `kept` does with `written`), or
   keying revisions per section, would cut most of the 16%. Also cheap: answer a chunk's
   loaded-ness with no copy where a mod only asks that (`has` exists; VoxelMap's
   `isSurroundedByLoaded` goes through `getChunk`).

Worth a look after those, not cheap: the HUD on a real GPU first. If it's still big, draw the
HUD's GPU images at 10 Hz like big targets (`PIP_INTERVAL`), send image pixels as binary, not
Base64 in JSON, and downsize in Rust's readback. Also: generate the spawn area while the client
bakes its models (85 + 22 s, one after the other now), and drop the template manager's cache
once mods' templates are exported.

## Traps found

- jcmd can't attach to the bridge's JVM without `-XX:+StartAttachListener` (`-Xrs`).
- `GC.class_histogram -all` counts dead objects too (it skips the full GC): use it without `-all`.
- The heap's `used` right after a run is mostly garbage: read the live set from the GC log's
  `Pause Full (Heap Inspection Initiated GC) A->B`, B.
- `jfr print --json` of a few minutes is ~700 MB; parse the text form (`jfr print --events …`).
- JFR's Java samples miss the mods' GPU work: read `jdk.NativeMethodSample` too (`Gpu.*`).
- `lab.py decompile` was OOM-killed on 7.6 GB; `javap -c` on `.local/minecraft/26.3/jars/*.jar`
  answers single questions.
