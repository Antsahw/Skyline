# Java bridge: notes for the boss chat

The boss chat merges the worker chats' branches, keeps `java-bridge` green, plans the next tasks
and talks with the user. Read this, then [java-bridge.md](java-bridge.md) and
[java-bridge-tasks.md](java-bridge-tasks.md). Update this file when something here changes.

## The people

- **The user: iaxivers.** Owns the fork `iaxivers/horizon`. Fedora, RX 9070 XT, Ryzen 9600X,
  32 GB DDR5, Europe/Oslo. Often on mobile: short replies, plain words, casual is fine. Has
  Claude Max; mind usage but don't be stingy when it matters. Wants honest answers ("is this a
  tragedy?" gets a straight yes/no and why). Tests on his PC in the evenings.
- **Ludovic (ludovic111):** owns upstream `horizon-rust/horizon`. Decides what goes upstream and
  all releases (agents never add `release-now`). Upstream's docs say Fabric/NeoForge jars don't
  run; upstream is building its own "native mods" (Rust patches the launcher compiles). The
  bridge needs his buy-in before any upstream PR. Not contacted yet (2026-11-01).
- **ChinoCappuchino (NeuHorizon fork, PR #5053):** NeoForge 1.21.1 mods compiled to native code
  with Cranelift. As of 2026-10-31 his own docs say mod behaviour code doesn't run and mixins
  don't apply; his claims of "deep mixin support" weren't true then. Friendly; worth sharing a
  hook-table format and the Prism launch idea (we built a Fabric version: `prism.sh`). Don't
  merge his code into ours.

## The chats

Six workers, one area each (files in java-bridge-tasks.md): entities, worldgen, pack, client,
mixins, gpu. Plus, when needed, one-off chats (the upstream sync). Workers report by the user
pasting their final message here; patches come as attachments.

- The **client** chat often can't push (403): its work comes as `git am` patches. Apply with
  `git am -3`. If its patch series includes a commit you already merged (it builds on its own
  older branch), cherry-pick only the new commit.
- Workers often branch from their own previous, not yet merged branch. That's fine: merged
  commits are ancestors; only new work conflicts.

## Merging a round

1. `git checkout java-bridge && git pull --ff-only`, then `git checkout -B merge-batchN`.
2. Merge each pushed branch (`git merge --no-edit origin/bridge-…`), apply patches. Stop at each
   conflict and resolve it before the next merge.
3. Conflicts seen so far, and how they were settled:
   - `docs/java-bridge.md`, `regress.py`: keep both sides. In regress.py the joined `dict(`
     blocks need a `),` between them; check `python3 -m py_compile` and `--list`.
     Watch for two chats using the same MODS key or case name for different mods.
   - Two chats adding the same method (`players()`, `origin()`, `tickCount`/`currentTick`):
     keep one if they're the same, both if the names differ; fix callers' types (Rust
     `worldgen_origin` returns a struct; callers take `.x`, `.z`).
   - **Model loading (`HorizonClient.models`)** was rewritten by four chats. Settled design:
     pack3's async reload (prepared on background workers, applied on the client thread via
     `tasks()`, `finishModels()` each tick, given up after 2 minutes), plus client2's
     `ClientBootstrap.bootstrap()`, gpu2's colour-map listeners and `fonts(atlases, skins)`.
   - **Client join (`joined`)**: gpu's per-player version (`joined(ServerPlayer)`, own
     connection, login + spawn packets) plus client3's Fabric INIT/JOIN events at its end and
     the `dev.horizonlab.platform.` prefix filter.
   - `HorizonChunks`: entities' revision cache (`kept`) for server views + gpu's held client
     chunks (`held`); `Copy` has both `made` and `server`.
   - **Batch 4 (on top of the upstream sync):** upstream's design wins where both built the
     same thing: loot loading (shared tables, pack overlays, `native_loot_table_load`; pack-5's
     lazy loot was dropped and requeued), the parallel table loading in `load_resources` (pack-5's
     memory marks kept), `player_fall_damage` (mixins' routed falls go inside it). Branches made
     before the sync that touched `server_mods.rs` merge into `java_server.rs` by git's rename
     detection; check where a hunk landed when HEAD's side of a conflict is empty (it can split
     a doc comment from its item). `mem_log` lives in horizon-client-render.
   - Merge messages: write them with `git commit -F`, not `git merge -m … --no-edit` after a
     conflict (that leaves the `# Conflicts` lines). Never `filter-branch` this branch: it
     re-hashes upstream's signed commits.
4. Build Java (`java.sh`), then the game; `cargo fmt`; clippy (`-A
   clippy::manual_range_contains`: pre-existing in horizon-world).
5. `regress.py`. Don't edit `screen_test.sh` while a run is going (bash reads it as it runs).
   A failure right after a merge is often the test scene, not the code (the pot scene once
   failed because the player picked up seeds first: it now clears the inventory).
6. Update README's status table if something big changed; `check_public_tree.py`; commit as
   iaxivers; fast-forward `java-bridge`; push.
7. Give the user one short message per chat (or "continue with your queue"), and update
   java-bridge-tasks.md's queue.

## A fresh boss container (what batch 4 needed)

- JDK 25 from Adoptium into `~/jdk25` (`JAVA_HOME`, `HORIZON_JAVA_HOME`); `libasound2-dev
  libudev-dev`, `mesa-vulkan-drivers xvfb` for headless runs.
- Fabric's libraries: the build's own download fails on the proxy's certificate: fetch the
  `fabric-libs.tsv`/`fabric-api.tsv` files with curl into `.local/bridge/lib` (and
  `HORIZON_BRIDGE_LIBS`).
- Vanilla: a launcher-style folder from Mojang's servers (version JSON pinned in
  `research-lock.json`, client and server jars, libraries, assets), then `horizon-prepare
  --prepare --minecraft-dir <it> --server-jar <it>/server-26.3.jar` (~1 min). Never commit it.
- Test jars from Modrinth's API (`/v2/project/<slug>/version?game_versions=["26.3"]&loaders=
  ["fabric"]`). Slugs that differ from the names: theoneblock, gwg (Grid World),
  buildcraftrefabricated, trinkets-updated, voxelmap-updated, diorama-map.

## Test jars

Not in the repo. `regress.py --jars <folder>` picks jars by name (`MODS` in regress.py). A boss
chat's container starts empty: download the Modrinth jars the cases need (list in
java-bridge.md), or run only the fixture cases (keys, entity, attack, ride, slot, mixins2).
Some sandboxes refuse to run downloaded mod code: then say so; the user can run them.

## Upstream

Synced with upstream main 6f54a627d (2026-11-01, `bridge-upstream-sync`, merged in batch 4).
Upstream removed WebAssembly code mods (our `server_mods.rs` is `game/java_server.rs` now) and
moved rendering into `crates/horizon-client-render`. Sync every day or two (a one-off chat, or
the boss when it's small); keep hooks in shared Horizon files to one-line calls into our files.

## Status (2026-11-01, merge-batch4)

`java-bridge` has upstream main (6f54a627d) and every chat's round 4. Works headless: everything
of before, plus (round 4) hostile mod mobs, riding mod mobs, GeckoLib animation, map mods with
Trinkets and per player, Diorama, 17 tech mods (BuildCraft, EE, RFTools Power...), mod settings
per LAN player, in-game mod failure messages, dedicated servers' mod screens/REI/Jade, Better
Combat sweeps, mods judging fall damage and effects, faster startup (110 jars: 193 → 120 s) and
content builds (173 → 107 s), much less memory while loading.

Not yet: anything on real hardware; 157 jars in 8 GB (memory); dedicated servers' other client
mods; running from a normal install; Windows.

## Next big steps

1. Daily upstream syncs.
2. The user's PC results → fix what's really slow or broken.
3. Beta readiness: run from a normal install (bridge jars and a Java runtime shipped/fetched),
   Windows, clear in-game mod error messages, a known-working pack list. Then a closed beta
   (10–20 testers) — only after asking Ludovic.
4. Draft the message to Ludovic (what the bridge does, how it could fit: opt-in feature,
   separate crate, or on top of native mods).
5. NeoForge/Forge for 26.3 (same mirror; a different loader and events). 1.12 is a separate
   research project (Cleanroom 1.12.2 + a 1.12↔26.3 translation), last.
