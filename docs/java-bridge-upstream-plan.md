# Upstream PR plan (2026-11-06, read-only analysis)

Nothing created or opened yet. Helpers: `tools/java_bridge/upstream_plan/` (`spec.tsv`: every changed file/hunk →
PR; `pick.py FILE 1,4,7`: those hunks as a patch; `mk.sh ID`: applies one PR's rows to a worktree, `CHECK=1` dry run;
`hunks.py`/`sizes2.py` regenerate the spec — re-run after P0/P1 change the fork tip; `groups.py`: class-level
dependency cycles in the Java platform). Baseline: upstream main `2b74332fbe`, fork `f90ab51b2c`, merge base
`41d5649ed5` (upstream 609 commits ahead).

## Summary

- **~70 PRs.** Without the crate and gameplay bodies upstream sees ~20k lines; with them ~66k (reviewer said ~64k).
- **Two fork-side refactors come first**, or the planned order doesn't compile:
  - **P0** (`bridge-split-prep`, no behaviour change): split `game/java_server.rs` (lines 1–807 plain-build, 808–4514
    feature) into shared types + `off.rs` + per-area files; same for `java_screen.rs` (1,361 lines compiled in the
    plain build — decision: gate it, or keep plain clients able to draw a bridge server's screens); off-stubs per area
    (`game/java_hooks/{blocks,players,screens,entities,worldgen,lan,remote}.rs`); rename generic lower-crate
    `*/java.rs` to `mod_*`; move every `#[cfg(feature="java-mods")]` call site out of the C areas into G; launcher
    fork fallback into `java_variant/fork.rs`.
  - **P1** (before J1): the Java platform has one **37-class cycle across 12 J groups** (HorizonServer refers to 28
    classes in J6–J16, HorizonClient to 21 in J10–J16; shared mixins LevelMixin, ServerLevelMixin,
    ServerChunkCacheMixin, GameModeMixin, ChunkMapMixin). Fix: core + per-subsystem parts via a `HorizonParts`
    registry (`horizon-parts.txt`, `Class.forName`), a J2 `HorizonSide` class, one mixin per subsystem, registered
    callbacks for the rest, natives split into `src/natives/<area>.rs`, prototypes removed. New order:
    J1 J2 J3 J4 J5 J7 J6 J8 J9 J11 J14 J12 J10 J13 J15 J16.
- **Order:** A + B (independent, open together) → P0 (fork) → C0–C7 → P1 (fork) → J1 (+CI1a) … J16 → G0–G7 →
  L1–L4, CI1b, E2E, D1.
- **Over 500 lines:** A8 (750) and every J and G PR: ask for the exception (Ludovic allowed larger PRs for new files
  under crates/horizon-jvm) or split on the fork.
- build.rs downloads Fabric and compiles Java: keep horizon-jvm out of `default-members`, or skip the Java step
  without a JDK, so plain `cargo test --workspace` stays offline.

## Fork-only (never upstream)

`.github/workflows/java-mods-release.yml`; the README rewrite; the AGENTS.md paragraph; `launcher/java-mods-keys.txt`;
`launcher/src/java_release.rs`; in `java_variant.rs` `install_from_fork`, `fork_commit`, `refresh_from_fork` and the
fork fallbacks (`java_bug_report.rs` uses `marked_commit`); backend.rs `option_env!("HORIZON_LAUNCHER_NO_SELF_UPDATE")`;
research-lock.json `cleanroom`; docs `java-bridge-{112,neoforge,tasks,boss,survey,pitch,perf,upstream}.md`; tools
`*112*`, `neoforge/`, `survey/`, `setup_cloud.sh`; in the crate `src/{neoforge,boot112}.rs`, `tests/boot112.rs`,
`java/neoforge`, `java/platform112`, `dev/neo/NeoBoot`, `dev/c112/Boot112`.

## Conflicts with upstream main (16 files, mostly additive: keep both)

| File | PR | Resolution |
|---|---|---|
| game.rs ×6 | C0/G0, C3 | `--java-mods` beside upstream's `--pregen` block; `Screen::JavaMod` beside `HudEditor` (×3); re-insert `java_screen_scroll` / `java_screen_mouse` in upstream's new scroll/mouse code |
| agent.rs, control.rs, startup_menu.rs | C3 | JavaMod arms beside HudEditor |
| options_ui.rs ×2 | C3 | Java rows after upstream's Browse button and settings column |
| client_lib.rs | A2 | add `mem_log` to upstream's list |
| mods.rs | C1 | `java` field next to `jobs` |
| mounts.rs | C4 | keep upstream's `use …Action;` |
| remote.rs ×2 | C4 | `mob_hit()` returns 3 values |
| remote_vehicles.rs | G3 | `remote_ride(keys, sprint_window) -> Result<bool>` → `return Ok(true)` |
| lan.rs | C6a | upstream wraps the call in `spans::time` |
| lan_actions.rs ×2 | C6b/C6c | keep `\|\| g.handover.is_some()` in `invulnerable()`; keep `lan_building_boxes` |
| lan/conn.rs ×3 | C6a | `tapped` beside upstream's `wire` counters |
| launcher backend.rs | L1 | move upstream's nightly-digest body into `release_digest.rs` |
| launcher main.rs | L1 | keep upstream's auto-update block |
| tools/lan_e2e.py | E2E | append |

Strict `git apply --check` also drifts in mesher.rs, fluid.rs, advancements.rs, options.rs, screens.rs; 3-way merges
them cleanly.

## PRs outside the crate

Gate: none = always on (general fix); default no-op = trait default/empty table; stubs = plain build answers
none/false. Labels: A none (fix) or `feature` (★); B, C `skip-release`.

| ID | Title | ≈ lines | Gate | Depends | Tests |
|---|---|---|---|---|---|
| A1 | Protocol: in-memory connection carries packets past 2 MiB | 113 | none | – | `connection::wide::tests` |
| A2 | Big packs: streamed gameplay-facts.json, heap trim, HORIZON_MEM_LOG | 324 | none | – | **add** GameplayRow vs Value table test |
| A3 | Block models: smaller BakedQuad | 95 | none | – | model/mesher tests |
| A4 | Worldgen: data packs' structure templates | 183 | none | – | `pack_templates::tests` |
| A5 | Worldgen: noise seeded under another id | 101 | none (non-vanilla `horizon_seed` key) | – | `noise_seed` |
| A6 | Content mods: blocks with their own states | 273 | none | – | state_table/content tests |
| A7 | Collision: content blocks follow their template's rule | 355 | none | – | `tests/mod_block_collision.rs` |
| A8★ | Content root: own states and collision templates | 750 (split) | none | A6, A7 | mod_content tests + 5 slow |
| A9a | Port pack: lenient fabric.mod.json, ids, legacy .lang, namespaces | 473 | none | A6 | carry tests |
| A9c | Port pack: blocks' own states and state tables | 456 | none | A6 | **add** own_states test |
| A9d | Port pack: carried item components, invisible blockstates | 136 | none | A9c | **add** tests |
| A10 | Models: OBJ `map_Kd` naming a texture slot | 29 | none | – | obj_slots |
| A11★ | Levels a server or data pack adds (`Dim::Extra`) | 488 | none | – | **add** Dim round-trip, dimension_type sync; lan_e2e |
| A12★ | Replay viewer: `--replay` plays `.mcpr` | 480 | none | – | replay.rs tests |
| A13 | Streaming: no proto over a loaded column; forced chunks first | 70 | none | – | **add** guard test |
| A14 | Small fixes (ctl timeout, offscreen fps, `/locate`, dropper, mount jump…) | 329 | none | – | title tests, agent_control_e2e |
| B1 | Level hooks for fluids/blocks another engine runs | 129 | default no-op | – | observe test |
| B2 | Worldgen hooks: noise-only protos, unsupported structures, foreign biome/noise | 419 | default no-op | – | unsupported test |
| B3 | Mesher: content fluids' sprites/tints, BE-drawn blocks | 260 | empty | A3 | **add** mesher test |
| B4a | HUD atlas, particles, biomes: content mods' images | 215 | empty | – | hud expectation |
| B4b | Items from given quads; chunk block revision | 236 | empty | A3 | item_render test |
| B5 | Jars as data packs, structures-only packs, ticketed chunks, block-set hook | 170 | default no-op | A4, A13 | datapack/java test |
| C0 | Java mods' state, plain-build stubs | 94+ | stubs | P0 | plain build, clippy |
| C1 | Hooks: random ticks, fluids, breaks/places, hoppers, crafted takes | 240 | stubs | C0, B1, B5 | parity --changed |
| C2 | Hooks: effects, advancements, omen, falls, teleports, commands | 295 | stubs | C0 | omen tests |
| C3 | Hooks: a mod's screen, HUD and keys | 149 | stubs | C0, B4a | agent_control_e2e |
| C4 | Hooks: mod entities' draw, hits, mounts | 163 | stubs | C0, A14 | mounts parity |
| C5a | World presets from mods | 303 | stubs | C0 | create_world; dedicated_server |
| C5b | Hooks: worldgen as a world loads, spawn search, command registries | 322 | stubs | C0, A11, B2, B5 | generation_saves |
| C6a | Hooks: held joins, connection tap, unplaced menus | 425 | stubs | C0, A1 | packet_tap; lan_e2e |
| C6c | Hooks: mod abilities, tab names, mod menus' windows | 231 | stubs | C0 | guest/java test |
| C6b | Hooks: guests' actions | 356 | stubs | C6a, C6c | lan_e2e; server_player_damage |
| C7 | Hooks: a server's mod payloads on the client | 84 | stubs | C0, C3 | – |

Gameplay bodies (feature-only): G0 bridge start (799, needs J2, J3, J8); G1a–f java_server (4,604, J4–J7); G2a/b
java_screen (1,361, J12); G3a/b java_entities (1,127, J13); G4a/b java_worldgen (1,129, J14); G7 java_device + shared
Vulkan instance (612, J15, J16). Launcher: L1 switch + java_variant without fork code + release_digest (~550), L2a/b
Java mods tab (~900), L3 CLI/docs/MCP (~300), L4 bug report (476); each with `tr()` keys in all 10 languages. CI1a
(Java 25 in CI) lands with J1; CI1b (game-build.yml `java_mods` input + packaging) after G. E2E with G-LAN. D1 docs.

## crates/horizon-jvm → J1–J16

Rust 12,010 lines (637 fork-only), Java platform 30,628 in 141 files, java/dev 886, fixtures 2,634.

| J | Contents | Tests |
|---|---|---|
| J1 | Cargo.toml, build.rs, *.tsv, src/{libs,runtime,gate,platform}, dev/{Boot,FabricBoot,HorizonGameProvider,Natives}, platform json; workspace member, prepare `pub real_path`, check_public_tree, check_no_fabric | tests/boot.rs, test_check_no_fabric |
| J2 | src/lib.rs (without boot112/neoforge; split its natives), Log, Services, Failures, Profile, Json, plugins (878) | tests/fabric.rs; loadermod, failmod, hellomod |
| J3 | events.rs 2,824, world.rs 260, facade.rs 708 | facade, facade_stress; testmod |
| J4 | HorizonServer 3,597 + 7 Access (3,842) | real_game.rs |
| J5 | 18 server mixins (1,158) | eventsmod |
| J6 | 16 world-state classes + 7 mixins (3,996) | fixture dev |
| J7 | Hooks, MixinTable, MixinInit (2,317) | mixinmod |
| J8 | content.rs, renderers.rs, HorizonContent (869) | content.rs, modrinth.rs |
| J9 | HorizonClient (4,144) | client_side.rs; keysmod |
| J10 | 3 classes + 16 client mixins (1,089) | configmod |
| J11 | Loop, PacketTap, Configuration + 9 network mixins (1,793) | joinmod |
| J12 | 13 GUI/texture classes + 14 mixins (3,499) | – |
| J13 | Entities, EntityRender, Spawner, Packets, Particles, Sounds (3,300) | crittermod, cartmod, geckomod |
| J14 | 6 worldgen classes + 3 mixins (2,470) | geomod |
| J15 | src/gpu, HorizonGpu, dev Gpu (3,294) | kineticmod |
| J16 | device.rs, DeviceMods, DeviceClient (1,008) | voice_test.sh |

## Executing

```
cd <repo> && git fetch https://github.com/horizon-rust/horizon main
export UP=$(git rev-parse FETCH_HEAD) FB=$(git rev-parse origin/java-bridge); export MB=$(git merge-base $UP $FB)
git worktree add --detach ../wt-up $UP && cd ../wt-up && git switch -c up/<branch> $UP
tools/java_bridge/upstream_plan/mk.sh <ID>      # then fix conflicts per the table above
cargo fmt --all && python3 tools/pr_ready.py
gh pr create -R horizon-rust/horizon --head iaxivers:up/<branch> --base main [--draft] --body-file body.md
```

Before opening: a tracking issue ("Filed by …"), `/claim` it. Dependent PRs go up as drafts "Depends on #N" and are
rebased after the parent merges (upstream PRs can't target a fork branch). PR body opens with
`> 🤖 Opened by iaxivers's AI agent (Claude Code, Claude Opus 5.5), not by iaxivers personally.`, then
"Part of the Fabric mods bridge (#<tracking>; design docs/design/java-bridge.md, #10216): PR <ID>." and, per kind:
A "Useful without Java: …"; B/C "No behaviour change: the hooks answer none until the bridge (`java-mods`, off by
default) is built and turned on."; J/G "New code behind the `java-mods` feature, never in the plain build.";
"Depends on: …"; the pr_ready block; the "Generated with" line.
