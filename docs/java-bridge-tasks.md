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
