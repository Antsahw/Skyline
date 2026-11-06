# The Java bridge upstream: hooks in Horizon's files, and the PR series

Ludovic asked for the bridge in horizon-rust/horizon (2026-11-04). Decisions so far: **off by
default**, the launcher's "Fabric 26.3 mods (beta)" switch turns it on (docs/java-bridge.md,
"The Fabric switch"); the `java-mods` cargo feature builds it. This page is what a reviewer of
Horizon's own files sees, and the order to send it in. No PR is opened until the user and
Ludovic agree on the plan below.

## Size (vs upstream main's merge base `73d3ee3d2`, 2026-11-04)

| | before (bridge-upstream-prep) | after (bridge-upstream-prep-2) |
|---|---|---|
| Horizon's files changed | 143 | 141 |
| lines added / removed in them | +6,345 / −585 | **+1,835 / −438** |
| of which Rust | +5,833 / −558 in 120 files | **+1,476 / −412** in 118 files |

The rest left the shared files for files that are ours (all new): `<file>/java.rs` child
modules next to the file they hook (a child sees its parent's private items, so nothing had
to become `pub`), `game/java_*.rs`, `tools/java_bridge/`. General Horizon improvements the
bridge needed (kind **G** below) moved into topic-named child modules (`oracle/state_table.rs`,
`models/catalog/baked_quad.rs`...): each is a PR of its own that stands without Java.

How to see what's left: `git diff 73d3ee3d2 -- <file>`; the moved code: `git ls-files '*/java.rs'
'*/java/*'` and `game/java_*.rs`.

Kinds: **H** a hook (a call into our code, a field holding our state, a `mod`/`pub use` line, an
enum variant every `match` needs); **G** general (a fix or feature that works without Java).

## The plain build

`cargo build -p horizon-client` (no `java-mods`) didn't compile on `java-bridge` (60 errors:
`game/java_mods.rs` used `horizon_jvm` unconditionally; hooks called `java_entities.rs`
methods, which only exist with the feature). Fixed here: `java_mods.rs` keeps the `JavaGame`
state, the bridge's start is `java_mods/bridge.rs` (feature only), `java_mods/off.rs` answers
"none" for the entity and click hooks. Clippy `-D warnings` is clean with and without the
feature. **Upstream's CI must keep building both**; a plain-build check belongs in the first
hooks PR.

## Hooks that remain, by crate

### horizon-world, horizon-core, horizon-protocol, horizon-threads (no Java in them)

| file | what stays | kind | why it stays in Horizon's file |
|---|---|---|---|
| level/mod.rs | `LevelAccess::foreign_fluid(state)` / `foreign_fluid_changed(pos)` (default methods) | H | only the trait can let a level own a fluid (a Java mod's); the game answers from `java_server` |
| level/fluids.rs | `foreign_fluid` checks in `fluid_tick`, `should_spread_liquid`, `liquid_block_changed`, `lava_random_tick` | H | the exact places vanilla's FlowingFluid/LiquidBlock/LavaFluid would act on the mod's fluid |
| level/update.rs | `foreign_fluid` check in the fluid-tick scheduler | H | every fluid tick is scheduled there |
| level/mod.rs | `mod caller_precipitation;` | G | `precipitation_tick_with` (a caller's own weather answer) |
| level/data.rs | `streaming::read_gameplay` (2 lines) | G | the gameplay file streamed, not parsed whole (big packs' 110k states) |
| block_collision.rs | `fallback` map, `StateRules::new`, 3 `extend`, `warn_fallback`, a 3-line fallback check in `collision_shape` | G | mods' blocks with a template's collision: the registry's own build loop and query |
| collision.rs | `templates` field + one parse line | G | parsed where the cached registry is built |
| worldgen/density.rs | `register_noise_seeded(.., noise_seed(json), ..)` | H | a noise's own seed from its JSON |
| worldgen/generator.rs | `noise_only` flag on `generate_with_context`, structures filtered by it, early `ProtoChunk::at_noise` return; `SeededPipeline::generate_noise`/`unsupported_structures` decls + impls | H | noise-only chunks (far terrain from the mods' noise) inside the existing stage fn; a private trait's methods live in its blocks. Biggest left (+50) |
| worldgen/generator.rs | `.with_packs(..)`, `from_hash_of(&noise_seed::id(..))`, `OverworldGenerator::unsupported_structures` | G | small inline |
| worldgen/generator_structures.rs | `StructureState::unsupported` (5 lines) | G | reads the private `possible_sets` |
| worldgen/overrides.rs | `"structure"` in `PACK_KINDS`, `templates` field, `is_empty`, `if self.templates.add(..)` | G | data packs' structure templates (The Sift's overrides) in the pack walk |
| worldgen/structure.rs | `packs` field, `template()` reads it first | G | the template lookup itself |
| worldgen/structure/start.rs, worldgen.rs | `mod` lines | G | |
| core density/graph | `seed` on `NoiseEntry`, `from_hash_of(entry.seed())` | G | where the noise is hashed |
| protocol connection.rs, frame.rs | `wide` flag on reader/writer, `self.header()`, `self.contents(body)`, `contents`/`unbounded_contents` split | G | wide frames for the in-memory connection (a 2.3 MB packet ended the window's connection) |
| threads lib.rs | `mod heap_trim; pub use heap_trim::trim_heap;` | G | |

### horizon-client-render (no Java in it)

| file | what stays | kind | why |
|---|---|---|---|
| world.rs | `SET_HOOK` static + its call in `set` | H | every block write (the mirror hears Horizon's changes); `native_block_changed` sees only `Level.setBlock` |
| mesher.rs | `mod java;` + `pub use`; tint fallback arm `java::mod_tint`; `fluid: java::mod_liquid(..)`; one `SectionJava::new` per section, `java.fluid(id)`, `java.drawn_by_block_entity(id)` (`#[inline]`) | H | mods' tints, fluids and block-entity-drawn blocks in the hot loop |
| mesher/fluid.rs | `mod_look` param + 3 branches | H | the mod fluid's own sprites/tint inside `tesselate` |
| streaming.rs | `ticketed` field (+1 init) | H | the Java mods' share of upstream's `mod_forced` (native mods' tickets, upstream sync 6): `set_ticketed` (streaming/mod_tickets.rs) puts their chunks there, so upstream's own ranges and ticket sources keep them; nothing else left in streaming.rs (merge batch 12) |
| streaming.rs | column guards (3), `near_forced` rank, `Ranges::priority` | G | a proto finished/saved over a loaded column (bug fix); forced chunks' priority |
| datapack.rs | `structures_only` field (+5 inits), `set_reader`, `pack.skips(..)`, `Box<Zip>`→`Arc<Zip>` | H/G | mods' structure-only packs; the reader shares the zip |
| hud.rs | `mod java;` + `pub use`, `java_slots`, `walk_java_textures`, one test expectation | H | mods' GUI textures and dynamic images in the HUD atlas |
| options_ui.rs | `Act/Event::JavaModSettings`, `&& s.mods.java.is_empty()`, `.chain(java::mod_rows(s))`, 2 match arms | H | the Mods page's Java rows |
| models/catalog.rs | `BakedQuad`, `force_translucent`, baked-model count log | G | quad memory (pack-6) |
| biome.rs, particles.rs, assets.rs, chunk.rs, mod_list.rs | `mod java;`, one call each (`push_mod_biomes`, `insert_mod_fluid_sprites`), `BlockRevision` field, `java` field | H | |
| fusion.rs, cube.rs, breaking.rs | follow `BakedQuad` | G | |
| assets.rs, lib.rs | `mem_log::mark` ×5, `pub mod mem_log` | G | memory log for big packs |

### horizon-client lib (not gameplay)

| file | what stays | kind | why |
|---|---|---|---|
| Cargo.toml | optional `horizon-jvm`, feature `java-mods` | H | |
| entry.rs | `#[cfg] horizon_jvm::exit(..)` | H | Java's exit is the process's |
| mod_content.rs | `NewBlock.own`, `own_states`/`own_count` in `Plan::new`, `b.own_row`/`own_gameplay`/`own_names`, `rows_at` table dispatch, `collision_templates`, `if !b.own.is_empty() { ..; continue }` ×2 in nbt() | H | mods' blocks with their own states, in the content root's tables (the row loops are Horizon's) |
| mod_content.rs | `J::Raw` (+ its arm), streaming table writes (`Overlay::create`), a template without an item warns, string-state rename | G | big packs' tables as JSON text |
| vanillagen.rs | `oracle: java::Oracle`, `warn_unsupported_structures`, 2 early returns (`find_closest_biome`, `noise_biomes`), `trim_heap`, decorate's non-VanillaProto return, `pub fn layout` | H | the mods' biomes answer biome queries, `/locate` |
| item_render.rs | `let layers = self.java_layers(..)` | H | mods' item model types |
| lan/guest.rs, lan/hub.rs | `mod_abilities`, `tab_name`, `java_window_state` fields; `speeds()`/`may_fly()`/`invulnerable()`; tab-name-first `display_name` | H | mods' abilities (flight, speed) and tab names |
| lan/conn.rs | `tapped` field, `self.tap(&body)` ×2, `send_login_finished`, `wide_frame` | H | the packet tap (ReplayMod, Flashback) sees the window's player's bytes |
| lan/containers.rs, lan/data.rs, lan/mod.rs, lan/commands.rs | `pos: impl Into<Option<..>>`, `push_mod_biomes`, `pub mod packet_tap; replay;`, 2 consts `pub` | H | |
| landing.rs, motion.rs | `FallEvent::Routed` (+ describe arm), `route_fall` field and check | H | a fall the mods route (their own fall damage) |
| menu.rs | `crafted_takes` + `CraftedTake::of` | H | Fabric's crafted events |
| save.rs, title*.rs | `mod_preset` (WorldGen not `Copy`), `PRESET_KEY`, `write_world_gen_settings_preset`, `.or_mod_preset`, `load_mod_presets`, `cycle_mod_preset` | H | mods' world presets in Create World and level.dat |
| mobs.rs | `Part::parse` pub | H | |
| en_us.json | `horizon.replay.*` | H | |
| lan/mod.rs, bin/horizon-ctl/launch.rs | local connection error always logged; `HORIZON_CTL_LAUNCH_TIMEOUT` | G | |

### horizon-client gameplay (game.rs, game/)

Already documented one by one, with the native hook point that could replace each: docs/java-bridge.md,
"Hooks in Horizon's shared files". What remains after the move:

| file | what stays | kind | why |
|---|---|---|---|
| game.rs | `mod java_*` (7, 4 behind cfg), `java: java_mods::JavaGame` (was 8 fields) | H | |
| game.rs | `--java-mods` arm + help line | H | the opt-in flag |
| game.rs | `Screen::JavaMod`: id, close, draw, mouse, drag arms | H | every `match` on `Screen` |
| game.rs | `foreign_fluid*`, `java_random_tick`, precipitation filter, `kept_chunks()` | H | LevelAccess, chunk tickets |
| game.rs | `tick_java_server`, `java_random_ticks`, `java_break(..).horizon()` let-else, `java_block_event` ×2, `java_after_break`, `java_crafted`, `save_java_mods`, `start_java_mods` | H | one line each |
| game.rs | HUD, screen, overlay, entity geometry, shadow, hand; mouse, scroll, key arms | H | draw order, input routing |
| game.rs | `--replay` + `open_replay`; no pause on lost focus during level loading; `Loading.drawn_at`; `mem_mark`/`trim_heap`; early `behaviour_json` drop | G | |
| lan_actions.rs | guest swap (`mod_window`, `java_actor`), abilities, teleport refusal, attack/use block, use item, slot, ride, mob attack, command, death; packet arms (Attack, Interact, container click/close, screen input, device); jumps refused/routed falls; item and orb touch | H | the guest packet handlers (+91: the most hook points of any file) |
| machines.rs | `java_eject`, `java_suck`, dropper `java_drop` | H | hoppers and droppers with mods' containers |
| machines.rs | dropper into a container entity (`machines/dropper.rs`) | G | |
| lan.rs, chat.rs, remote.rs | held joins, `java_player_chat`, `java_event_except`, mod biomes/structures in registries, commands, `remote_java_payload`, client ticks, entity use/attack | H | |
| world_loading.rs, initial_spawn.rs, dedicated.rs, levels.rs | `java::streaming(..)` (JavaTerrain), mod presets, spawn origin, `set_ticketed`, `apply_java_level_writes` | H | |
| creatures, mounts, remote_vehicles, trial_spawner, names, server_travel, advancements, lan_crafting, combat, control, agent, integrated, mods, and 1-5 line ones | one call or field each | H | |
| integrated.rs, mods.rs, control.rs, agent.rs, commands.rs | `thread_end::Ends`, `mods/cache.rs`, `near_loading`, probe JSON fields, `locate_structure` key | G | |

### launcher

| file | what stays | kind | why |
|---|---|---|---|
| cli.rs | `Cmd::JavaMods`, its dispatch, Play's `java::ensure_build`, `"java_mods"` in instance JSON | H | clap subcommands are `Cmd` variants |
| main.rs | `java_mods: JavaModsTab`, `Msg::JavaBuild`, `stale()` on game exit, Play's `java_build_at_play`/`ensure_at_play` and the owner-gate condition | H | |
| main.rs | `HORIZON_LAUNCHER_PAGE=launch` | G | screenshot hook |
| backend.rs | `unpack` `pub(crate)`, `java_variant::follow`, `Msg::JavaBuild`; `release_digest` moved out | H/G | |
| home_tabs.rs, glass_home_tabs.rs, launch_page.rs, owner_page.rs | `HomeTab::JavaMods` + arms, the switch, owner gate | H | |
| instances.rs, mcp.rs, native_mods.rs, lib.rs, tests | `java_mods` fields, `java_variant::apply`/`ensure_for`, `own.push(DIR)`, mod lines | H | |
| lang/*.json | 7 keys each | H | `tr()` text |

### Tools, CI, docs

| file | what stays | kind | why |
|---|---|---|---|
| tools/lan_e2e.py | `--java-mods`, `JAVA_MODS`, `java_lan.game_setup`, `java_lan.register`, 2 longer waits | H | checks themselves in `tools/java_bridge/lan_checks.py` |
| tools/check_public_tree.py | `crates/horizon-jvm/java/` sources allowed, fixture stubs | H | |
| .github/workflows/game-build.yml | `java_mods` input, matrix `java_*`, Java 25, one step calling `tools/java_bridge/package_java_variant.sh`, upload | H | |
| Cargo.toml / Cargo.lock, .gitignore | member `horizon-jvm`; `mods-java/`, `config/`, bridge test dirs | H | |
| README.md, AGENTS.md | the fork's section and pointer | fork only | drop from upstream PRs; a short "Fabric mods (beta)" section instead |
| docs/launcher-commands.md, docs/native-mods.md | `java-mods` commands; lazy loot tables note | H/G | |

## Found on the way (not changed: this branch is a pure move)

- **lan_actions.rs**: `motion.flying_speed = g.speeds().0;` is overwritten by upstream's
  `motion.flying_speed = g.flying_speed;` on the next line, so a mod's flying speed is lost on
  the server (a merge leftover; likely fix: drop upstream's line there).
- The launcher's source-scan test (`mojang_downloads_only_for_an_owner`) treats files in
  subfolders (`cli/java.rs`, `backend/release_digest.rs`) as test files and doesn't scan them.
- Fixed: the plain build (above); 3 `mod_content` tests reading raw rows; the launcher's bin
  test `Instance` literal; `startup_menu`'s test `Loading`.

## The PR series (draft; nothing opened until agreed)

Rules from CLAUDE.md: < ~500 lines where possible, labels (`feature` / `skip-release`), each PR
self-checked with `tools/pr_ready.py`, opened from the user's account with the agent line.
Every PR builds and passes **without** `java-mods`; from PR 10 on, CI also builds with it.

**Phase 1, general (G): useful without Java, any order, ~100-300 lines each.**
1. Streaming fixes: a proto finished/saved over a loaded column; forced chunks' priority
   (`streaming.rs` inline, `streaming/busy.rs`).
2. Content format for big mods: state tables (`oracle/state_table.rs`,
   `content/states.rs`), mods' own states (`oracle/own_states.rs`, `carry/declare.rs`).
   Two PRs: format + reader; port-pack writer.
3. Port-pack fixes: lenient `fabric.mod.json`, hyphenated ids, legacy `.lang`, texture-only
   namespaces, invisible blockstates, carried item components.
4. Memory for big packs: `BakedQuad`, raw JSON rows (`J::Raw`), streamed gameplay file,
   `trim_heap`, `mem_log`, lazy loot tables (doc).
5. Worldgen for data packs: structure templates from packs (`overrides/pack_templates.rs`,
   `structure/packs.rs`), noise seeds, `unsupported` structures listed.
6. Protocol: wide frames on the in-memory connection.
7. Small fixes: no pause while the level loads, dropper into container entities, OBJ
   `map_Kd` slots, `HORIZON_CTL_LAUNCH_TIMEOUT`, the server thread's end log.
8. The replay viewer (`--replay`, `lan/replay.rs`, ~460 lines): standalone, Horizon plays
   ReplayMod `.mcpr` files.

**Phase 2, hook points, no Java yet (`skip-release`).**
9. Generic hook points in the lower crates: `LevelAccess::foreign_fluid*`, `SET_HOOK`,
   chunk tickets (`streaming/mod_tickets.rs` → rename `tickets.rs`), mod tints/fluids in the mesher,
   mod biomes, block revisions. Names without "java" where the hook is generic. ~400 lines.

**Phase 3, the bridge, behind `java-mods` (off by default).**
10. `horizon-jvm` Rust core (`src/lib.rs`, `events.rs`, `content.rs`, `libs.rs`: ~9,100
    lines) + Cargo feature + a CI job that builds with it. Too big for 500: proposed as
    one crate PR reviewed by file, or split lib/JNI · events · content · GPU · device.
11. The Java platform (`java/platform`, 137 files, ~28,000 lines) and boot (`java/dev`):
    new code only, never linked into the plain build. Proposal: CODEOWNERS
    `crates/horizon-jvm/ @iaxivers`, reviewed by subsystem (server mirror · client ·
    screens/graphics · entities · worldgen · mixins table · GPU), ~8 PRs.
12. Fixtures and tests (`java/fixtures`, `tests/`: ~4,100 lines).
13. Gameplay: `game/java_*.rs` (~8,700) + the `<file>/java.rs` children (~3,400) and their
    hooks, by area: server tick and actions · screens/HUD · entities · worldgen ·
    LAN/guests · client (remote) · device mods. ~7 PRs, each with its own hooks.
14. Launcher: the switch, the Java mods tab, the `-java` variant (~1,000 + hooks + 10 lang
    files).
15. CI: Java 25, the `java_mods` game-build input, a few headless cases
    (`tools/java_bridge/regress.py` with the fixture mods, `lan_e2e --java-mods javamobs`).
16. Docs: `docs/java-bridge.md` (trimmed of the fork's task history), the testers' guide,
    a README section. `java-bridge-tasks.md`, `-boss.md`, `-survey.md`, `-112.md` stay in
    the fork.

**To agree with Ludovic first:** the review shape for 10-13 (crate-sized PRs vs ~25 small
ones), CODEOWNERS for `horizon-jvm`, whether Phase 1's G items are wanted at all, and
renaming generic hooks (`java_*` → `mod_*`) in Phase 2.
