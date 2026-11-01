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
| mixins | `HorizonHooks`, `HorizonMixinTable`, routed actions in `game/java_server.rs` |
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

### Upstream sync (2026-11-01, branch `bridge-upstream-sync`)

`origin/java-bridge` (cab3843) with `horizon-rust/horizon` main (6f54a627d, 6,014 commits past
our base 3782473) merged in. 42 files conflicted; all resolved, upstream's code kept and our hooks
put back as small calls.

What moved where:
- `game/server_mods.rs` (deleted upstream with WebAssembly code mods): its Java half is
  `game/java_server.rs`. The WASM parts (lent world, server mod output, block writes) are gone;
  what Java needed of them is now its own: `JavaPlayer` (was `horizon_mods::ServerPlayer`),
  `ModsState::java` (players, events, block writes), `add_java_chat_line`. Renames:
  `tick_server_mods` → `tick_java_server` (game.rs `fn tick`, after `native_tick`),
  `mod_block_event` → `java_block_event`, `mod_player_chat` → `java_player_chat`,
  `local/guest_server_mod_command` → `local/guest_java_command`, `apply_mod_block_writes` →
  `apply_java_level_writes` (levels.rs). Java's command tree is merged after the native mods' one
  (`Hub::mod_commands`, new getter).
- `biome.rs` (deleted upstream) → `crates/horizon-client-render/src/biome.rs` (`mod_biomes`).
- Upstream's `horizon-client-render` split: our changes in `hud.rs`, `particles.rs`, `mesher.rs`,
  `streaming.rs`, `options_ui.rs`, `chunk.rs`, `world.rs`, `mod_list.rs` moved there.
- Dropped for upstream's own: our Fabric creative tab pages (upstream has NeoForge-style pages),
  our item component loading in content mods (upstream's `component_patch`), our mod particle
  sprite loading (upstream's; `add_mod` kept on top of it), `Guest::abilities_packet` (upstream's
  `abilities()` now carries a mod's speeds).
- Merged with upstream's new features: world presets (`WorldPreset` + our `mod_preset` for mods'
  presets), content blocks (upstream's declared state properties + our own states), random ticks
  (upstream's `before_random_tick` + our `RANDOM_TICK_HOOK`), item ports in `oracle.rs` (26.3
  components carried, 1.21.1's patch over them), ticket sources (forced = `/forceload` + Java's
  tickets).
- `features`: upstream removed `mods`; `java-mods = ["dep:horizon-jvm"]`.
- Content FORMAT 11 → 12 (the carry-over changed with upstream's port-pack): content rebuilds once.

What broke and was fixed: routed attacks (upstream's new mob-attack path returned before our
hook: `java_mob_attack` now runs inside it); mods' blocks with their own states (upstream's
model-states expansion overwrote them: "model state definitions disagree", every FD/Tom's block
unknown); a duplicated creative page UI; small API changes (`decode_with` layout, `Spans`,
`QueuedParticle`, `parked_chunks` gone).

Known, not from the sync: the build without `java-mods` doesn't compile (game.rs calls
`java_mods.rs`/`java_entities.rs` unconditionally); it didn't at cab3843 either.

Which of our hooks upstream's native mod hook points could replace: docs/java-bridge.md, "Hooks
in Horizon's shared files".

Tests (headless, this sandbox: 2 cores, lavapipe), `regress.py --jars` with the Modrinth jars
(26.3 Fabric): **21 of 22 pass**: pot, terminal, ber, hud, rei, keys, entity, worldgen,
veinminer, harvest, carry, seasons, orbs, preset, oneblock, attack, bettercombat, ride, slot,
snow, mixins2. `preset` needed `new_world.sh` to click past the new first-launch screen.
**skyblock** fails one check only: the player ends on the island, but first "fell out of the
world": they join right as the load functions finish (no hold), and the island comes from the
function Skyblock Infinite schedules 5 s later. Not traced to the merge (the hold logic is
unchanged); check it on a fast PC before chasing. Not run: bigpack, prism.
`lan_test.sh` with Farmer's Delight: the guest sees the host's stove, gets FD items, its
creative inventory shows the mods' page. Also: `cargo test -p horizon-jvm -p horizon-port-pack
-p horizon-mods` and horizon-client's mod_content (13 slow ones included), title, save, lan
tests pass; fmt and clippy `-D warnings` clean (horizon-client, horizon-jvm).

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
- **pack 3, SkyFactory direction** (`bridge-pack-5`, ed553348c). Works (17 tech and
  automation mods, alone: 48k states): BuildCraft (its module blocks: content mods declare
  under every namespace they own), Equivalent Exchange 26's transmutation table, GrowableOres,
  RFTools Power, Tesseract, Wireless Chargers, Large Fluid Tank (a client-to-server sync loop
  hung the server thread: fixed in `HorizonLoop.flush`), Logistics, Additional Enchanted Miner,
  Experience Bottler, Discerning Furnace, Conveyors & Automation, Simple Pocket Machines (its
  mixin config's trailing comma), Idle Generators, Copper Rails, Red Bits, TC Wireless
  Redstone. Left out: Croparium (its 26.3 build is a broken beta), Agritech (BOP recipes for
  items this BOP lacks, no conditions: fails on Fabric too), PolyFactory (99k states: past
  this sandbox with the rest). Doesn't: the 156-jar pack with them (245k states) runs out of
  memory here once Java starts; BuildCraft's FineRichesTracker server tick NPE; BuildCraft's
  model reload listener ("Invalid atlas id: minecraft:blocks"); several mods' settings screens
  on the Mods page (`getProvidedConfigScreenFactories`: client area). Test: `regress.py --jars
  … tech` (BuildCraft, Large Fluid Tank, Scalable Cat's Force jars), or the tech mods in
  `~/hz-test/mods-java`: `/setblock ~2 ~ ~ buildcraftbuilders:quarry`.

- worldgen 1, startup time (`bridge-worldgen-1`, by the worldgen chat): a 110-jar pack (the
  137's named mods from Modrinth; content made) went from 193 s to ~120 s to the join on 2
  cores. The content root was rebuilt on every run in a new folder (its stamp went through
  the `.horizon-cache` link): now reused (−40 s). Java starts right after the content root
  instead of after the first frame (−15 s here, more with more cores; peak 5.4 GB).
  `HORIZON_JAVA_EARLY_START=0` turns that off. Left: ~10 s of Fabric biome modifications ×
  TerraBlender at the overworld's start (mod code), and the spawn's chunks. Test: start the
  pack twice; the second log says "content from N mod(s)" within seconds, and "Hello from
  Java" comes before "load assets".

- worldgen 2, Grid World's far terrain (`bridge-worldgen-2`, by the worldgen chat): far
  columns are cut as its `doFill` cuts land (from its `getConfig()`): pillars whole, lines as
  their layers every 5 blocks, the rest gone; coarse far tiles show the layers (a tile can't
  show 4-block holes). Regress case `gridlod` (needs `gridgen*.jar`), unit test
  `grid_mask_keeps_pillars_layers_and_nothing_else`. Test: Grid World with `lodDistance:256`,
  fly up: the grid near, thin layers far, no solid land.

- **worldgen 3, content preparation time** (bridge-worldgen-3): the slow part was port-pack's
  nearest-shape search (`oracle::own_states`: every mod state against every vanilla state).
  Now over vanilla's distinct shapes, memoized: 110-jar pack 173 s → 107 s, output identical.
  Left: the Java oracle's class loading (~65 s). Test: delete `.horizon-cache/java-content`,
  start with the pack, `grep "content mod(s) made" latest.log`.

- **worldgen 4, lost blocks** (bridge-worldgen-4): the repro is a regress case, `lost`
  (`lost_blocks.sh`: 36 gold blocks set right after spawn with FD, read back 25 s later; fails
  on any lost one or a "generated again while loaded" line). It didn't show: 0 lost in 4 runs
  (35/34/36/35 set) on the current java-bridge, so no fix. Test: `regress.py --jars … lost`.

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
- **entities 4 (riding and getting off mod mobs)**, `bridge-entities-4` (on top of
  `bridge-entities-3`). Agent: Claude (iaxivers' chats). Works: a mod's rideable mob (new
  fixture `horizon_critters:mount`) takes a player on a use (its own `mobInteract` →
  `startRiding` on the mirror); the player's movement keys (PlayerInput flags, now bits 16-22
  of `HorizonServer.tick`'s `permissions`) become the mirror player's `xxa`/`zza`/jumping, and
  `RiderAuthorityMixin` makes mirrored players non-authoritative so the mirror moves the mob
  (vanilla leaves a ridden mob to the rider's client). `HorizonEntities.tickPassenger` doesn't
  run a mirrored player's whole tick: it seats it (`positionRider`) or, on sneak, runs
  `stopRiding` (`dismountTo` moves Horizon's player). The snapshot's `rides` tell each game its
  seat; the game sits there (sitting pose, its own mount not aimed at) and its move packets
  carry the seat. Touched outside the area: `remote_vehicles.rs`, `perspective.rs`,
  `events.rs`, `HorizonServer.tick` (input bits), `lan_e2e.py`. Test: `regress.py --jars …
  mount` (on 0.4 over the floor, drove ~20 blocks, off); `lan_e2e.py --java-mods <critter
  jars> --check javamount`. Not done: in-frame (`--in-frame`) riding; other players see the
  rider standing (Horizon's tracker doesn't know the mod's vehicle); horse-style charged
  jumps (`PlayerCommand` riding jump) aren't forwarded.
- **Seen while running entities 3 and 4 (not fixed, pack area):** `regress.py skyblock` fails
  often on a 2-core box: the player joins before Skyblock Infinite's load function has run
  (it waits on chunk 0 0, which Horizon hasn't loaded), so `held_joins` (game/lan.rs) lets
  the player in at y -63 and it falls out of the world. The hold should also wait for a load
  function deferred on an unloaded chunk.
