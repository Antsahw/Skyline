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

- **client 1, mod config per player on LAN** (`bridge-client-1`). Works: a guest's settings
  are theirs (AutoConfig mods, Jade, AppleSkin, REI): in memory per player, and files a guest
  saves go to `config/horizon-players/<uuid>/` while the host's stay (LAN-tested with
  AppleSkin: guest off, host on, host's file on). Also fixed: Architectury's join NPE came back
  with the per-player connections (they lacked `minecraft`/`connection`). Doesn't: a guest's
  files aren't loaded at their next join; Farmer's Delight-style ConfigValues stay shared.
  Test: `tools/java_bridge/lan_test.sh '$G command "modconfig appleskin"'`, toggle, Save, then
  `$H command "modconfig appleskin"`; regress `join modsettings` (new) pass, `rei hud keys` too.

- **client 2, a clear in-game message when a mod fails** (`bridge-client-2`, on client 1).
  Works: every player gets "[Java mods] <mod> failed in <where>: <exception>: <message> (the
  game goes on; the log has more)" in chat, once per mod and place (8 a session max), from
  events, hooks, screens, HUD, ticks, joins, settings and the recipe viewer; startup failures
  wait for the first join. Touched HorizonHooks/HorizonServer.fire with one call each.
  Doesn't: failures outside those catch sites (worldgen, entity drawing) only log. Test:
  regress `failmsg` (failmod fixture), or any pack: open chat after a failure.

- **client 3, dedicated server: mod screens, HUD and REI for its players** (`bridge-client-3`,
  on client 2). Works (screenshots): on `--dedicated`, a joined player gets Tech Reborn's
  machine screens, Jade with TR energy, REI over the inventory and over TR's screens. Fixed on
  the way (single player too): REI's list vanished from mod screens a few seconds after they
  opened. REI reloads on its own at the client's join, which emptied the synced displays and
  removed its overlay renderer; both are put back now. REI's settings are shared again (not
  per player, unlike client 1 said): its copied config stopped the overlay. Test:
  `MODS=<REI+TR+Jade...> tools/java_bridge/server_test.sh '$G key e tap; sleep 6'` (new);
  regress `reiscreen` (new), `rei hud join modsettings failmsg` pass.
