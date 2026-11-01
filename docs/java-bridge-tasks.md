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

- **entities 2 (hostile mod mobs attack players)**, `bridge-entities-2`. Agent: Claude (iaxivers' chats).
  Works: a mod's mob hurting a mirrored player (`ServerPlayer.hurtServer` on the mirror) goes to
  Horizon (`ServerPlayerHurtMixin` → `Natives.hurtPlayer` → `Action::HurtPlayer` →
  `java_hurt`): Horizon hurts its player as its own mobs do (armour, blocking, knockback from
  where it came, invulnerable ticks, death), as that LAN guest or the window's player, and the
  death message names the mob ("was slain by Biter"). A mod's damage type Horizon doesn't know
  counts as a mob's attack. New fixture mob `horizon_critters:biter` (hunts players, 3 a bite).
  Also: `regress.py` rebuilds a fixture whose sources changed (it reused stale jars), and can
  build one against the case's jars (`FIXTURE_CP`). Touched `server_mods.rs` (mixins area: one
  routed action) and `lan_e2e.py`. Test: `regress.py --jars … hostile` (the player loses health
  to a biter); `lan_e2e.py --bin-dir target/fast --java-mods <critter jar folder> --check
  javahostile` (a guest 20 → 17). Not done: mod mobs vs Horizon's own mobs (they don't see each
  other), a mod's projectile is untested (same path).
- **entities 3 (GeckoLib fixture animates)**, `bridge-entities-3`. Agent: Claude (iaxivers' chats).
  Works: new fixture `geckomod` (`horizon_gecko:bobber`, a GeckoLib 5.5 entity with a looping
  idle animation: its top turns and bobs) draws and animates. The fix: the bridge rendered at
  partial tick 1.0, which GeckoLib reads as "game paused" (`AnimationController` stops); it now
  renders at 0.999 (`HorizonEntityRender.PARTIAL_TICK`). New regress case `gecko` takes two
  screenshots 0.7 s apart and checks they differ (`tools/java_bridge/changed.py`). Branch merges
  `bridge-entities-2` (for `FIXTURE_CP`); touched tools only outside the area. Test:
  `regress.py --jars <folder with geckolib-fabric-26.3-*.jar> gecko`.
