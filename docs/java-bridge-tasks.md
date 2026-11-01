# Java bridge: the task queue

The chats working on the bridge take their next task from here, so they keep going without
waiting for a person. The boss chat merges the branches into `java-bridge` and updates this file.

## The loop, for each task

1. `git fetch origin && git merge origin/java-bridge` into your branch (start each task from the
   newest `java-bridge`, not from your last branch).
2. Do the task. Stay in your area's files (below); if you must touch another area's, keep the
   change small and say so in your report.
3. `python3 tools/java_bridge/regress.py --jars <your jars>` (`--java-only` after a Java-only
   change). Add a case for what you made work.
4. Commit, push your branch (`bridge-<area>-<n>`). If the push is refused, make a patch.
5. Add a short entry under "Reports" at the bottom: what works, what doesn't, how to test.
6. Take the next task of your area. After 4 tasks, or about 6 hours, stop and report.

Never write as the user; mark what you post (CLAUDE.md). Never run mod jars the sandbox blocks.

## Areas and their files

| Area | Owns |
|---|---|
| entities | `HorizonEntities`, `HorizonEntityRender`, `HorizonSpawner`, `game/java_entities.rs` |
| worldgen | `HorizonWorldgen`, `HorizonChunks`' generation, `game/java_worldgen.rs`, LOD |
| pack | `HorizonData`, `HorizonFunctions`, `HorizonContent`, `content.rs`, the big pack |
| client | `HorizonClient` (screens, HUD, join), `HorizonRecipeViewer`, `game/java_screen.rs` |
| mixins | `HorizonHooks`, `HorizonMixinTable`, routed actions in `game/server_mods.rs` |
| gpu | `HorizonGpu`, `horizon-jvm/src/gpu/`, map mods |

## Queue (in order)

### entities
1. Measure the per-type mob cost in the 137-jar pack; cut the top three.
2. Hostile mod mobs attacking players (single player and a LAN guest); add a regress case.
3. GeckoLib: a fixture mod with an animated GeckoLib model (no download needed) drawing with
   its animation; real GeckoLib mods are checked on the user's PC.
4. Dismounting (`stopRiding`) and riding mod mobs.

### worldgen
1. Startup time: why the 137-jar pack takes ~255 s to spawn; cut it (profile the start).
2. Grid World's far terrain (its holes).
3. Content preparation time for a new mod set (4–7 min): find the slow part.
4. Lost blocks: keep the repro running in the regress set (a case), fix if it shows.

### pack
1. Memory: block states (577k with 157 jars) — find where Horizon's memory grows with them
   (Rust side) and make it lean enough for the full pack in 8 GB.
2. The remaining errors: Lithostitched setup, Cardinal Components' scoreboard client tick,
   vehicle hearts on the HUD.
3. Grow toward SkyFactory: more 26.3 Fabric tech and automation mods; fix what breaks.

### client
1. Mod config files per player on LAN (a guest changing Jade's settings changes the host's).
2. A clear in-game message when a mod fails (which mod, why), instead of only the log.
3. Dedicated server: mod screens, HUD mods and REI for its players; add a lan_test-style check.

### mixins
1. Getting off vehicles (`stopRiding`), and Better Combat's sweep (several mobs a swing).
2. The next "Rust does" lines from `mixin-table.tsv` in the big pack, most-patched first.
3. A regress case per routed action not covered yet.

### gpu
1. Xaero, JourneyMap and VoxelMap with Trinkets installed (models never load: find why).
2. JourneyMap and VoxelMap per player (their state is one per game).
3. Other 26.3 Fabric mods that draw with the GPU: find and get them working.

## Reports

(Newest last. One short entry per task: area, task, branch, works / doesn't / how to test.)

- **pack 1, memory** (`bridge-pack-5`, 74191149b; Claude for iaxivers). Works: where the
  memory goes is measured per load stage (`HORIZON_MEM_LOG=1`; table in java-bridge.md,
  "Where the memory goes"); `level::Behaviours`' parse of `gameplay-facts.json` no longer
  peaks at 3.4 GB (208k states: 453 MB; GeneratorData's load 3.95 → 0.73 GB); loot tables are
  read on use (577k pack: −270 MB); the heap is trimmed after the big loads. Doesn't: the
  577k-state pack still doesn't fit this 8 GB sandbox: the world opens at 5.2 GB (1 GB of it
  llvmpipe's textures) and the JVM's start passes the limit. Biggest left: baked models
  (+665 MB), block facts (+512 MB, the behaviour JSON kept until the world rest), `BlockState`
  (~1 KB a state). Test: `HORIZON_MEM_LOG=1` with any pack, read `memory after` in the log;
  `cargo run --profile fast -p horizon-world --example state_memory_probe -- <content root>`.
- **pack 2, errors** (`bridge-pack-5`, 6f5856370). Works: Lithostitched's noise router
  modifiers apply (its setup error is gone), Cardinal Components' scoreboard client tick,
  Enderscape's vehicle hearts; Puzzles Lib's and Architectury's handleLogin on a player's own
  connection (a small change in `HorizonClient.connection(player)`, client area); Skyblock
  Infinite's first player no longer falls (joins wait for the load functions' first
  schedules; regress `skyblock` failed on java-bridge cab3843). Doesn't: Xaero's world map's
  frame hook (models, the Trinkets issue), Wunderlib's and Shulker Box Tooltip's receivers at
  ClientPlayConnectionEvents.INIT, Compressed Blocks' settings screen. Test: `regress.py
  --pack <137-jar pack> bigpack` and `--jars … skyblock oneblock pot hud rei` (all pass).
