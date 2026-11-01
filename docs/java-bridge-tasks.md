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

- **entities 1 (mob cost in the 137-jar pack)**, `bridge-entities-1`. Agent: Claude (iaxivers' chats).
  Measured (`HORIZON_JAVA_PROFILE=1`, 50–60 mod mobs around a walking player): Naturalist's
  whale 0.7–0.9 ms each (8 of them: 6–7 ms a tick), Promenade's lush creeper ~0.3 ms, Naturalist's
  jellyfish 0.15–0.36 ms; all mobs 16–20 ms of ticks, ~2.5 ms of snapshot. JFR (in a small pack
  of Naturalist and Promenade, 6 of each summoned: the 137-jar pack runs out of the sandbox's
  8 GB with JFR attached): Promenade's `tickMovement` mixin asks every living entity's biome each
  tick, and the mirror's `BiomeManager` sampled the climate noise for each. Now the mirrored
  levels read Horizon's own chunk biomes, kept per chunk (`HorizonBiomes`, noise only where
  Horizon has no chunk), and block reads skip the thread-local lookup when the same thread reads
  again. Whale 0.75 → ~0.58 ms, jellyfish 0.25 → ~0.16 ms, lush creeper ~0.2 ms (noisy: the mods'
  worldgen shares the 2 cores). The whale's rest is its own body parts' collisions and fluid
  checks. No regress case (a cost); `regress.py --java-only`: all pass (`attack` failed once and
  passed on rerun: flaky). Test: `HORIZON_JAVA_PROFILE=1`, `grep "entity ticks" latest.log`.

