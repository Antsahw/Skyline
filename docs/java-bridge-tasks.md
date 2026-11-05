# Java bridge: the task queue

The chats working on the bridge take their next task from here, so they keep going without
waiting for a person. The boss chat merges the branches into `java-bridge` and updates this file.

## The loop, for each task

Fresh cloud container: `tools/java_bridge/setup_cloud.sh` first (one command, ~20 min, mostly the
build). Java-only change: `JAVA_ONLY=1` (seconds, no game rebuild). Run only your own cases.

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

Round 8 (after merge-batch7, 2026-11-03). Start from the newest `java-bridge` (it has upstream
sync 4). A case that fails only in a full regress run is a bug in the case: make it stable when
you meet one (on a slow box: `jump2` saw an extra refused jump, `worldmap-lazy`'s World Map
count stayed under its 10k mark while the map was still filling in). Worldgen is still on round 6.

### Before anything else: Ludovic's two conditions, and launcher only (2026-11-04)

Ludovic's agent read the branch: Horizon will link to it as an unofficial community project once
both are fixed. The user's decision: **players play only through Horizon's launcher; Prism and
`horizon-play` go.**
1. ~~**Ownership (one chat, `bridge-launcher-only`).**~~ Done 2026-11-04 (docs/java-bridge.md,
   "Launcher only"). Left: lang/ (ko, hr, ar, hi, id, pt...) lack the fork's Java mods strings,
   so the launcher's i18n test fails on java-bridge already. Was: Remove `horizon-play` (crate, docs, CI,
   packaging, regress `prism`) and Prism's wrapper path; the testers' guide becomes launcher
   only. No offline play anywhere a player can reach: the game with Java mods starts only from
   the launcher with its Microsoft sign-in (the switch's `--java-mods`), and nothing is
   downloaded (Java runtime, server jar) before that sign-in; the server jar comes from the
   player's own install where possible. Developers' headless runs (`horizon-ctl`, regress) stay
   as Ludovic's own harnesses run.
2. ~~**No code tied to other mods' internals (one or two chats, `bridge-generic-N`).**~~ Done
   2026-11-04 (`bridge-generic-maps`, `-ui`, `-entities`; merge-batch11). `regress.py` now fails
   when a mod's package is in the bridge's code (`nomodcode`, java-bridge.md "The regression
   run"); Mod Menu's API is the one allowed (ask Ludovic if that's fine). Was: Every
   mixin and reflection into a specific mod (JourneyMap's four mixins, Xaero's, VoxelMap, Better
   Combat, REI, JEI, Jade, Architectury, GeckoLib, Create...: `git grep -l` the mod ids in
   `crates/horizon-jvm`, `crates/horizon-client`) is replaced by a generic fix in the bridge (do
   what vanilla does there, so every mod gets it) or removed. Check each mod's licence and say
   it in the report. A mod that stops working is listed in java-bridge.md's status, not patched.
   Its regress cases stay and show what's broken.
   Maps (JourneyMap, Xaero's, XaeroPlus, VoxelMap, MiniHUD/MaLiLib): done in
   `bridge-generic-maps` (see Reports). Left for the other chat: REI, JEI, Jade, AppleSkin,
   Better Combat, Architectury, GeckoLib, Create...

### Testers' Trello (2026-11-04, https://trello.com/b/2uhi4jkj)

**CRASH, first of all (2026-11-04, tester, batch 10, Create + Fabric API):** placing a
scaffolding stops the server thread: "Horizon: unsupported block collision state 36126"
(`horizon-world/src/block_collision.rs`: `collision_shape` has no cached shape and no `dynamic`
rule for the state, returns `Unsupported`, and a caller treats it as fatal). 36126 is past
vanilla's states: a mod block whose collision depends on context (scaffolding-like). Fix: a
state Horizon has no rule for must never end the game: ask Java for the mod block's shape
(`getCollisionShape` in the mirror) or fall back to its outline/full cube, and log once. Find
every caller that turns `Unsupported` into a stop. Own small chat (`bridge-crash-1`): regress
case placing it, then walking into/on it.
The tester had to **remake the world** after it: also check that a world saved at such a stop
opens again.
**Done (bridge-crash-1):** the block is Create's `andesite_scaffolding` (template
`minecraft:scaffolding`). The stop was the window's own motion (Render thread) and every other
collision query: fixed at the source, `BlockCollisionRegistry` never answers `Unsupported` for a
registry state (the template's rule, else a full cube). Regress `scaffold`; the tester's kind of
save (made at the crash) opens and plays.

Chats on it (2026-11-04): `bridge-client-13` (Cloth Config, Chat Heads, waypoint NPE,
Litematica), `bridge-replay-1` (ReplayMod, Flashback), `bridge-mixins-10` (double break sound,
Chunky), `bridge-voice-1` (Simple Voice Chat), `bridge-entities-10` (TACZ), `bridge-trello-1`
(reads the board, retests every card on the newest java-bridge, queues new ones here). The board
is public: read it with a web fetch of `https://trello.com/b/2uhi4jkj.json`.

bridge-client-13 (see Reports): Cloth Config's screens fixed (icons, scrolling, lag); the
waypoint NPE was already fixed (now regressed on host, LAN guest, server); Litematica's
in-world overlay and Chat Heads' heads researched, not done (java-bridge.md says why and how).

Works: JEI. Kinda: Farmer's Delight. Doesn't work (testers' builds may predate batch 10: ask
them to pull first): Create Fly, TACZ Refabricated, Simple Voice Chat, Chunky, Better Combat,
Chat Heads, Cloth Config, Flashback, ReplayMod. Also seen: "A Java mod failed in the HUD:
NullPointerException ... ClientWaypointManager.hasWaypoints()" (client-12/gpu-12 gave the
connection a waypoint manager: check it's set on every client connection, LAN guest and
server too). Litematica untested by them.
By area:
- **client (UI mods, `bridge-client-13`)**: Cloth Config's screens (a library many mods use for
  settings: first), Chat Heads (draws heads in chat: Horizon's own chat), ReplayMod and
  Flashback (record packets and replay them with vanilla's renderer: research what's possible;
  likely a "not supported" line like Sodium's if not), Litematica's overlay, the waypoint NPE.
- **server/mixins**: Chunky (pregeneration through the server's chunk system). Simple Voice
  Chat: done in `bridge-voice-1` (java-bridge.md, "Voice chat"); left: icons over heads,
  a dedicated-server run on a real network.
- **entities/gpu**: TACZ (guns: models, animations, projectiles).
- Better Combat and Create: in the generic-entities and gpu-13 chats.
- **Create, tester on batch 10 (2026-11-04, for gpu-13):** Create's "fancy 3D" items (the
  ones drawn by a custom item renderer: wrench, held tools) don't draw, in the hand or the
  creative tab; and a placed machine (press/mixer-like, over a basin or belt) draws with wrong
  parts: grey cylinders and a pale blue-white flat square where its moving parts' textures
  should be (a sprite or atlas mixed up for its block-entity parts, or a partial model). Not an
  entity problem: item renderers and block-entity parts.
- **FIRST for `bridge-mixins-10`: lava on water loops** (tester, batch 10, Create Fly + Fabric
  API, videos): lava flowing onto water makes obsidian, which turns back into flowing lava,
  again and again (constant smoke). Vanilla behaviour broken: a regression, likely batch 10's
  fluids (mixins-9's `foreign_fluid`) or Create's mixin on `LiquidBlock.shouldSpreadLiquid`/
  `FlowingFluid` interactions being routed to Java while Horizon's own fluid tick also runs, the
  two writing over each other. Reproduce: Create + a lava source flowing onto still water;
  vanilla gives obsidian/cobblestone once. Without Create too. Then **Create's own fluid
  interactions** (honey + lava makes limestone, chocolate + lava makes scoria): what a mod adds
  to lava's interactions must happen once, through the mod's code.
- **Items placed on stations don't show** (tester, batch 10): right-click with an item on
  Create's depot puts nothing visible on top; "like Farmer's Delight" (its cutting board,
  skillet, stove show items on top through their block-entity renderers). First find which
  half fails: is the item in the block entity (Java's `LIVE`, `HORIZON_JAVA_DEBUG=ber`) and
  gone from the hand? Then it's drawing: block-entity renderers that draw an item stack
  (`ItemModelResolver`/`submitItem` inside a BER) need the item path in the bridge's recorder.
  If it never got in, it's the use (mixins). gpu-13 checks first; FD's cutting board passed
  `pot`-style cases, so compare with it.
  Update: right-click does nothing **and thrown items don't go in either** (the depot takes
  item entities that land on it): so the item never gets in, not only a drawing gap. Mixins:
  the use on a mod block with an item in hand, and a mod block entity picking up item entities.
  Same root, seen with Create's fluid tank: a water bucket used on the tank places a water
  block beside it instead of filling the tank. Horizon's own item use (bucket, also any vanilla
  item with its own use) runs before the mod block gets the click; vanilla asks the block first
  (`useItemOn` on the block, then the item's `useOn` only if the block passed).
  Clear evidence (tester, train tracks): placing a curved track by clicking a track, then the end,
  says Create's own "Right-click the targeted train track first": the first click never reached
  the track item's `useOn` (the item's remembered start is never set). A good regress case: track
  item on a placed track, then the action bar must not show that line. Same for metal brackets.
- **Create's motor speed can't be set** (tester, batch 10: "motors lack GUI"). Create has no
  screen for it: you look at the motor and **scroll the mouse wheel** (its value box, drawn as
  a HUD overlay on the looked-at block). So: mods' injections on `MouseHandler.onScroll` while
  playing (not in a screen) must run and may cancel the hotbar scroll, and the value box overlay
  must draw. Same bucket: **Create's goggles** show nothing (worn, looking at a machine draws a stress/
  speed tooltip overlay: a HUD layer of the looked-at block). Client chat (`bridge-client-13`): generic, every mod with scroll-to-adjust gets it.
- **Fluids drawn black in screens** (tester, batch 10): in JEI's list (and a mod's fluid slots)
  the fluid entries (water, lava, Create's honey/chocolate) are black or dark squares. Mods draw
  a fluid in a GUI from its still sprite in the block atlas, tinted (Fabric's
  `FluidRenderHandler`/`FluidVariantRendering`, `GuiGraphics` blit of a `TextureAtlasSprite`):
  the screen path needs those sprites and the tint. gpu-13 (it did fluids in the world).
- **Create machines, tester batch 10 (gpu-13 checks logic first, then drawing):** a water wheel
  in flowing water doesn't spin (Create reads the neighbours' `FluidState` flow vectors in the
  mirror: are Horizon's flowing water levels and falling flags there, and does the kinetic
  network get a speed?); the mechanical mixer over a basin is "just bugged" (ask what: no
  mixing, no animation, or a drawing gap; his screenshot has a block between mixer and basin,
  check his setup). Confirmed working on batch 10: eating FD meals.
  Also: **crushing wheels** do nothing (they process item entities thrown between them: the
  same item-entity path as the depot, mixins); **fluid pipes draw broken** in water (a pipe
  through a water block: its middle segment missing, the outline bigger than the model;
  check the pipe's connection states after neighbour updates in the mirror, and multipart
  models of waterlogged mod blocks).
  And **mechanical pumps** "non functional" (a pump moves fluid between pipes and takes water
  source blocks from the world: Java removing and reading Horizon's fluids through the mirror;
  check with pump + pipes into a tank, his screenshot's setup may lack pipes).
  **Mechanical saw** works (it fells the tree) but its blade doesn't draw (the saw's moving part,
  a block-entity renderer part like the shafts; a black hole where it should be). **Ponder**
  (Create's tutorial: hold W over an item's tooltip) is missing: its keybind on tooltips, then
  a screen that draws small 3D scenes of a fake level through vanilla's renderer. Research
  after the rest (gpu): likely the hardest Create piece.
  **Belts don't assemble and chain conveyors don't connect**: both are made by using an item
  (belt, chain) on one block then another, so likely the same use-order bug (mixins: the block
  and the item's `useOn` first, as vanilla). And a client tick error: `ParticleEngine.
  createParticle` on a null (Horizon's client has no vanilla `ParticleEngine`: a mod making a
  particle client-side must get one that hands the particle to Horizon's particles; client chat).
  **Blaze burners** work but draw wrong: the blaze head looks doubled/overlapping and a pale
  blue-white flat square under it (the same pale square as the press/mixer report: likely one
  sprite or render type wrong for animated/emissive block-entity parts; gpu-13).
  **New kind (client chat): mods' client sounds.** Using Create's frogport: "failed in using a
  block: NullPointerException ... FrogportAudioBehaviour.open". Mods play client sounds through
  vanilla's `SoundManager` (`Minecraft.getSoundManager().play(SoundInstance)`, looping and moving
  sounds); Horizon's client has no vanilla sound engine: give mods one that hands their sounds
  to Horizon's audio. Casings work (tester).
  **Encased fans** do nothing: a fan's air current pushes entities and processes item entities
  in front of it (Java moving Horizon's entities: players, mobs, items; the same item-entity
  path as the depot and crushing wheels, plus entity motion set from a mod).
  **Mechanical arms** draw but do nothing: their pick-up/drop-off points are chosen by
  right-clicking blocks with the arm item before placing it (the use-order bug). "Anything
  variable can't be set" (filters, speeds, thresholds): Create's value boxes (scroll) and
  filter slots (right-click with an item on the box): the scroll and use buckets above.
  **Drill works** (tester).
  **Windmill bearing and super glue** do nothing: a bearing assembles its sails into a
  contraption (an entity carrying blocks) on right-click, glue is applied by clicking two
  corners: the use-order bug, then contraptions (not done: gpu-13 has them listed).
  **Toolbox: the block opens (lid animates) but no screen shows**, "again": a mod menu that
  needs its opening data on the client (Fabric's `ExtendedScreenHandlerType`/menu data
  sent with the open packet) or a screen that fails to make: check the log for the screen's
  error; client chat.
  **Cardboard armor disguise** doesn't work (full set + sneaking: you look like a box and mobs
  ignore you). Two halves: drawing a player as something else (a mod's player-renderer hook;
  Horizon draws players itself: gpu) and mobs ignoring a disguised player (Horizon's mobs are
  Rust: they'd have to ask the mods whether a player can be targeted, a hook on target
  selection: entities). Lower priority.
  **Sandpaper** does nothing (hold right-click with it, the off-hand item gets polished): a mod
  item used over time that isn't food. Mixins-9 made food finish; the same must hold for any
  mod item with a use duration (`getUseDuration`, `onUseTick`, `finishUsingItem`,
  `releaseUsing`): mixins.
- **A broken block's moving parts stay** (tester, batch 10): break a cogwheel and the hand
  crank's handle on it keeps floating, drawn. A removed mod block entity must drop its
  renderer's instance (gpu-13: the recorder's per-block-entity draws on removal).
- ~~**Breaking sounds twice**~~ and ~~**Chunky**~~: done in `bridge-mixins-10` (report below).
  Was: **Breaking sounds twice** (a tester, 2026-11-04: "dirt goes brrbrr"). Mods: Biomes O' Plenty,
  TerraBlender, GlitchCore, Terralith, Geophilic, Lithostitched, FallingTree, WorldEdit, Create
  Fly, Fabric API. Likely a routed break (FallingTree and WorldEdit patch breaking) where Java's
  `destroyBlock` sends its level event 2001 (break sound and particles) and Horizon's own break
  plays it too. Check: break dirt with each mod alone; the client must get one 2001 per break.
  mixins chat (`bridge-mixins-10`), first.

#### Retest on java-bridge 3bae5c528 (bridge-trello-1, 2026-11-04)

Every card's mod, newest 26.3 Fabric file from Modrinth (with its required deps and Fabric API
0.165.2), alone, headless (`screen_test.sh SCENE=none` + steps). Create Fly not retested:
CurseForge (site and CDN) answers 403 here, run it on a PC. The board itself wasn't read (the
fetch tool refuses trello.com): cards added after the list above are missing.

| Mod (version) | Result | Area |
|---|---|---|
| JEI 31.17.0.108 | works (regress `jei`, `pot-jei` pass) | – |
| Farmer's Delight 3.6.30 | works (regress `pot`, `pot-jei`, `eat` pass) | – |
| Litematica 0.29.1 + MaLiLib 0.30.2 | menu on M works (regress `litematica` passes) | – |
| Cloth Config 26.3.159 | works: Better Combat's Cloth screen opens by `/modconfig bettercombat`, draws right | – |
| Better Combat 3.2.2 | hits land and go through the mod ("Players' attacks go to the mods"); no errors. Its swing animations (PlayerAnimationLib) not seen on Horizon's player | entities/gpu |
| Chat Heads 1.3.2 | loads, no error, no heads: Horizon draws its own chat | client |
| Chunky 1.5.66 | fails: task starts, never generates | mixins |
| Simple Voice Chat 2.6.27 | fails: client entrypoint dies; server starts | client + mixins |
| TACZ Refabricated 0.8.6-beta1 | fails: gun invisible, client ticks throw | entities/gpu + client |
| Flashback 0.43.6 | loads, does nothing visible: no recording, no UI | client (`bridge-replay-1`) |
| ReplayMod 2.6.28 | loads, records nothing (no `replay_recordings/` after a walk) | client (`bridge-replay-1`) |
| Double break (BOP, Terralith, Geophilic, Lithostitched, FallingTree, WorldEdit, each alone) | not reproduced (details below) | mixins |

By area (log lines from `<scratch>/home/logs/latest.log`):
- **mixins**
  - **Chunky**: `/chunky radius 48`, `/chunky start`, 30 s later `/chunky progress`:
    `[Chunky] Task running for minecraft:overworld. Processed: 0 chunks (0.00%) ... Rate: 0.0 cps`.
    No error. Its chunk loads (`ServerChunkCache` futures/tickets) never complete: the mirror's
    chunk source holds nothing (`ServerChunkCacheMixin`). Needs those loads routed to Horizon's
    generation (or the Java worldgen path) and completed.
  - **Simple Voice Chat, server half**: `Voice chat server started at port 43908`, then
    `Mixin census: voicechat's onPlayerConnect at RETURN of PlayerList.placeNewPlayer takes
    locals; Horizon can't give them`: the voice server never learns the player.
  - **Lithostitched** (alone): every level tick `Lithostitched failed in
    ServerTickEvents.START_LEVEL_TICK: NullPointerException: Cannot invoke
    "net.minecraft.world.level.StructureManager.getStructureWithPieceAt(...)" because "manager"
    is null` (`StructureAttributeHandler.tick:41`): the mirrored level's `structureManager()` is
    null. (Not seen with Terralith beside it.)
  - **Double break**: with a probe logging each `LevelEvent` 2001 the window's client gets
    (`remote.rs`, not committed), a survival `break` of dirt gave **0** extra 2001 with each mod
    alone (right: vanilla's `levelEvent(player, 2001, …)` skips the breaker, whose client plays
    it itself). Not yet checked: sounds that come as sound packets (`HorizonPackets`), creative,
    other blocks, FallingTree on a log, WorldEdit's wand, all the tester's mods together. Seen
    instead: **with BOP (+TerraBlender, GlitchCore) the client got 43 2001s within 2 s of
    joining, 25–35 blocks from the player** (y 56–62; WorldEdit's run: 1). Something in the
    world is being broken near spawn (plants popping off?): may be the tester's "brrbrr".
- **client**
  - **Simple Voice Chat, client half**: `Could not execute entrypoint stage 'client' ...
    'voicechat' at 'de.maxhenkel.voicechat.FabricVoicechatClientMod'!, caused by
    java.lang.NullPointerException: Cannot invoke "...IPackRepository.voicechat$addSource(...)"
    because "repository" is null` (`FabricClientCompatibilityManager.addResourcePackSource:136`):
    the headless `Minecraft` has no resource pack repository. V opens nothing after that.
    Audio (mic, OpenAL) is a research item after it.
  - **Chat Heads**: 22 client mixins into `ChatComponent`/`GuiMessage` rendering; Horizon's chat
    (`chat.rs`) never runs them. Nothing in the log.
  - **TACZ, client half**: `A mod failed in ClientLifecycleEvents.CLIENT_STARTED
    [cn.sh1rocu.tacz.client.TaCZFabricClient]: ... "ModelManager.entityModels()" because
    "this.modelManager" is null`; then every client tick `... "LocalPlayerDraw.draw(...)" because
    "this.tac$draw" is null` and `"LocalPlayerAim.isAim()" because "this.tac$aim" is null`
    (TACZ's `LocalPlayer` mixin fields, set in its constructor injection: the bridge's
    `LocalPlayer` is allocated without it). The gun's name shows as
    `block.tacz.modern_kinetic_gun` (its translation key not found).
  - **Flashback / ReplayMod**: both load with no error. Horizon's pause screen has neither's
    buttons; neither writes a recording (they record from the client `Connection`'s packets,
    which Horizon's window doesn't have). For `bridge-replay-1`'s research.
  - Every run: `[Options/ERROR] Failed to load options ... "Options.forceUnicodeFont()" because
    "this.options" is null` (`Minecraft.isEnforceUnicode` during `Options.<init>`): harmless so
    far, but noise in every testers' log.
- **entities/gpu**
  - **TACZ**: `/give @s tacz:modern_kinetic_gun[custom_data={GunId:"tacz:glock_17",...}]`: held,
    nothing drawn in hand or hotbar. 126× `[tacz/WARN] tacz:<gun> index file read fail!
    java.lang.IllegalArgumentException: there is no corresponding display file` (its gun packs'
    display files are client resources, loaded after the CLIENT_STARTED that failed above).
    Fix the client half first, then its item renderer and animations.
  - **Better Combat**: check its swing animations on the player (PlayerAnimationLib draws
    through the player model's parts).

Screenshots: `/tmp/trello/<mod>/none-N.png` of that run (cloth's screen, the invisible gun,
chat with no head, Chunky's 0 chunks). The driver: one `screen_test.sh` per mod with steps
(`/modconfig bettercombat`; `attack` a NoAI zombie; `chat`; `chunky start`; `key v`; give and
use a gun; pause; `break` a dirt block in survival).

### A tester's log (2026-11-05, ~60 mods, mostly client-side: GTX 1060, Linux)

No crash: the world played ~4 minutes and closed normally. Mods: Continuity, Entity Model/Texture
Features, Async Particles, FancyMenu, Immersive Hotbar, ShulkerBoxTooltip, Legendary Tooltips,
Zoomify, Falling Leaves, Particle Effects, JEI, Jade, Xaero's, 3D Skin Layers, Lithium, Mouse
Tweaks, The Sift... Failures, by what Horizon's headless client lacks (client chat; all generic):
- **`mc.particleEngine` null** (Async Particles every tick; also Create's): a vanilla
  `ParticleEngine` that hands particles to Horizon's.
- **`ModelManager` / `ItemModelResolver` null** (EMF's entity models, the hands' tick, Immersive
  Hotbar's `stack` null, the HUD's hotbar and item name): stand-ins the HUD and hands use.
- **Options load fails: "Can't getDevice() before it was initialized"** and "the GPU device
  failed to start" early: a mod reading options before the bridge's GPU device is up.
- **Continuity (connected textures): "Invalid atlas id: minecraft:blocks"** in its reload: the
  block atlas under its 26.3 id; then its CTM loader NPE.
- **mezz_config gui atlas: "Unknown element id: minecraft:directory"** (an atlas sprite source
  type) and "Fallback pipeline cache not yet set".
- **ShulkerBoxTooltip: "Cannot register receiver while not in game"** at
  `ClientPlayConnectionEvents.INIT` (the event fires before the client counts as in game).
- **FancyMenu: packet send NPE** (`pendingActions` null: a connection field not made).
- `Entity.vicinityCache` missing; "a client level has no chunk source: value 0 not in 1..32"
  (a view distance of 0 given to the client level).

### Later: upstream's launcher redesign (noted 2026-11-04)

Upstream redesigned the launcher (the "glass" views). Batch 10 only added the Java mods tab to
its home view. Later: check the Fabric switch, the Java mods tab and its failures list look and
work right in the new design (screenshots), after the launcher-only chat.

### Upstream (2026-11-04: Ludovic asked for the bridge in the main repo)

The user's decisions: **off by default; the launcher has a switch to turn on Fabric 26.3 mods.**
One chat, branch `bridge-upstream-prep`, in this order:
1. ~~**The switch.**~~ Done 2026-11-04 (bridge-upstream-prep; docs/java-bridge.md, "The Fabric
   switch"). Left: `-java` assets in the signed releases (the maintainer's). A launcher setting (per instance, `tr()` text, design-system look) "Fabric
   26.3 mods (beta)", off. On: the instance installs/updates the `-java` game variant
   (game-build.yml already packages it), shows the Java mods tab (today it shows whenever
   `mods-java/` exists), and starts the game with an explicit opt-in (e.g. `--java-mods`).
   Off: the plain build, no tab, no Java started, even with jars in `mods-java/`. The game
   with the `java-mods` feature also starts Java only when opted in. Fetching Java 25 happens
   only after the switch. Test headless: the launcher CLI, both states.
2. **Shrink the shared-file edits** (5,300 lines in 122 of Horizon's files vs upstream main):
   move logic into our files, leave one-line hooks; list each remaining hook and why
   (`docs/java-bridge-upstream.md`).
3. **Plan the PR series** (< ~500 lines each where possible): hooks, crates, gameplay files,
   launcher, CI (Java 25, a few headless cases), docs. Don't open PRs until the user and Ludovic agree.

### The user's PC run (2026-11-03, 81-jar test pack from `tools/java_bridge/get_test_pack.py`)

Plays (RX 9070 XT, Fedora): JourneyMap, Xaero's Minimap/World Map, Jade, REI, mod mobs ("very
well"), structures, the mods' terrain, most mod blocks and their logic; performance about the
same as without mods. Fixed on the way (java-bridge, boss): worldgen workers racing on the
structure ring positions, a Horizon-made chunk beside Java ones panicking, the window's
in-memory connection dropping a 2.3 MB packet ("End of stream"), a pause on lost focus during
"Loading terrain" that held the chunks back. Run it yourself: in `~/hz-play` (the game folder is
the working directory: `mods-java` is read from there, not from `HORIZON_HOME`).
Broken, first in every area's queue below:
- **Invisible blocks** in a swamp biome (a few), and **mod blocks invisible when placed** until
  a relaunch (gpu/client: a state's model not there for the mesher until the next start).
- ~~**Hoppers don't work**~~ (bridge-mixins-8: not Lithium; hoppers didn't see Java mods'
  containers. Fixed, see Reports.)
- **Litematica's UI** doesn't show (client; MaLiLib's MiniHUD does).
- **Sodium** doesn't work: expected, it replaces vanilla's renderer, which Horizon doesn't use.
  Say so in-game (a "not needed in Horizon" line) rather than a failure (client).
- ~~The test's offscreen window renders uncapped (99% GPU, coil whine)~~ (pack-9: capped).

- ~~**Litematica's UI** doesn't show~~ (client-12: MaLiLib's hotkeys get every key; M opens
  its menu. See Reports.)
- ~~**Sodium** doesn't work~~ (client-12: renderer mods are left out, with a gray "isn't needed
  in Horizon" line.)
- The test's offscreen window renders uncapped (99% GPU, coil whine): cap it (tools).

### First beta testers (2026-11-03, Discord)

- ~~**(2026-11-04, Windows 11 tester, video) Farmer's Delight's cooking pot sometimes opens as
  Horizon's stand-in screen**~~ (client-12: JEI's screens over the pot lost it; see Reports) (one row of 9 slots titled "Cooking Pot", `mod_menu_type` in
  `remote.rs`) instead of FD's own Java screen (grid, arrow, bowl slot, JEI beside it). The
  same pot opens right on other clicks: a race between Horizon's stand-in and the Java screen
  taking over. Client chat, first in its queue. Reproduce: open and close the pot 20 times
  (regress `pot` opens it once); make the case open it repeatedly.
- ~~**(2026-11-04, same tester, video) A Farmer's Delight bowl meal never gets eaten**~~
  (bridge-mixins-9: fixed, regress `eat`; see Reports): hunger
  9/10, he holds right-click with the meal (a bowl dish, count 1), the eating animation and
  particles run on and on, the item stays and hunger doesn't change. Check plain FD foods
  (cooked bacon) too. Likely the use's finish (`finishUsingItem`: FD's `ConsumableItem` gives
  the bowl back and applies its effects) isn't reached or applied for a Java mod's food
  (`consume.rs`, the mirror's use). Entities/mixins chat. New regress case: eat an FD meal and
  a plain FD food; check hunger, the stack and the bowl.
- **(2026-11-04, same tester, Windows, screenshot) Mods' fluids don't work** (flow: done in
  bridge-mixins-9, regress `fluids`; drawing and entities in them still open): a honey bucket

- **(2026-11-04, same tester, Windows, screenshot) Mods' fluids don't work** (drawing done in
  gpu-12: their own sprites; flow is still lava's: mixins): a honey bucket
  (Create) places a fluid drawn as lava. Mods' fluids need their textures/tint (Fabric's
  `FluidRenderHandlerRegistry`) in Horizon's fluid mesher, and their flow (Horizon's fluid
  ticks are Rust: a mod `FlowingFluid` must tick in the mirror or be ported per fluid). gpu
  chat (drawing) + mixins (flow). Test with Create's honey and chocolate, and a plain fluid mod.
- **(same) Create ("Create Fly", unofficial 26.3 port) is broken** (gpu-12: Flywheel readied,
  kinetic parts draw and turn, OBJ items draw, both NPEs fixed; gpu-13: press, millstone,
  deployer, belts, contraptions, the wrench and the tab's blank items; left: item entities in
  the mirror): kinetic machines don't
  work. Create draws its shafts, cogs and contraptions with **Flywheel** (instanced rendering
  on top of vanilla's renderer, which Horizon doesn't run) and its contraptions are entities.
  First a research round (gpu chat): does the kinetic network turn and process in Java (logic)
  and only not draw, or is the logic broken too? Then Flywheel's instances drawn through the
  GPU path, as the bridge does for block-entity renderers. Big: Create is the most-asked mod.
  Second video (Windows): Create loads (its creative tab, items, advancements); a Creative
  Motor places and draws; **many items in the tab are blank** (Create's items drawn by custom
  item renderers: shafts, cogs, kinetic blocks); **honey places and flows but looks like
  lava; chocolate places nothing visible**. Kinetics not shown working.
  Boss probe (2026-11-04, the jar from CurseForge `create-fly-unofficial-port`, file 9321329,
  public domain; Flywheel and Ponder are inside it): Create starts; a motor, shafts and a
  cogwheel place. Three concrete failures in the log:
  1. **Flywheel never initialised**: Create's block-entity renderers already reach our BER path
     (`CreativeMotorRenderer`, `BracketedKineticBlockEntityRenderer`) and throw because
     `flywheel.lib.model.baked.ModelRenderHelper.INSTANCE` is null. Flywheel sets it in its
     client init, hooked on vanilla's renderer start, which Horizon doesn't run: call it (or set
     the helper) from the bridge. Likely fixes most invisible kinetic parts and blank items.
  2. ~~`LocalPlayer.firstPersonHandsAndItems()` returns null~~ (client-12)
  3. ~~`SynchedEntityData.get` with a null accessor in Create's `AllSynchedDatas`~~ (client-12;
     two more HUD failures behind it fixed too: the Hud's contextual bar, the waypoints)
  Not checked: whether the kinetic network turns (`/data get block` reads Horizon's world, which
  has no Java block entity: read Java's `LIVE` instead). Order: 1 (gpu), then 2–3 (client),
  then a regress case `create` (motor → shaft → cogwheel; speed read in Java; a screenshot).
  **1–3 fixed (gpu 12, see Reports); the `create` case is written but not yet run with the
  real jar** (CurseForge's CDN is blocked from the agents' machines): run it on a PC.

Works for them: Bedrock Reloaded (small pack), Terralith ("runs better than normal Fabric"),
Biomes O' Plenty; 800 FPS stable on a GTX 1060 3 GB / i5-9400F / 16 GB. Reported:
- ~~**A pack's resource packs aren't picked up**~~ (pack-9: Prism instances and a pack's
  folder beside `mods-java`; see Reports).
- **Mods' overrides of vanilla structure templates are ignored** when Horizon generates the
  structure itself: Mielon's The Sift replaces `data/minecraft/structure/ancient_city/city_center/
  city_center_{1,2,3}.nbt` (8 sonorous deepslate before the frame), new worlds get vanilla's
  centres. Horizon's structure code must read templates (and template pools, processors) from
  the mods' data the way a data pack overrides them (worldgen; the same for any data pack).
- ~~**JEI doesn't appear**~~ done (client-11, 2026-11-04): its list, search and recipe screens
  over Horizon's inventory, per player.
- ~~**3D Skin Layers** doesn't draw~~ (gpu-12: its meshes drawn; Horizon's flat layers stay
  under them).
- Asked: macOS (untested); native Rust mods beside Java mods (untested; the launcher's native build has no
  `java-mods` feature: add it in the fork, then test one native mod with the test pack).

### Survey 1: failures of the 60 most-downloaded Fabric 26.3 mods (2026-11-04, `bridge-survey-1`)

Details and log lines: [java-bridge-survey.md](java-bridge-survey.md). Rerun one mod:
`SURVEY_DIR=… python3 tools/java_bridge/survey/survey.py <slug>` (after `fetch.py`, `inspect.py`).

**client** (`HorizonClient`'s allocated `Minecraft` and join)
1. **Essential stops the game** (exit 78): its coroutine dispatcher on Minecraft's executor has none
   (`ExecutorCoroutineDispatcherImpl.getExecutor()` null, from `PackRepository.openAllSelected` in
   `HorizonData.prepare`). Give the headless `Minecraft` its executor; and a Java mod that fails to
   start should be left out (and named), not stop the game.
2. Null fields of the headless `Minecraft`, one pass: `resourcePackRepository` (Simple Voice Chat's
   client entrypoint), `profileKeyPairManager` (No Chat Reports), `launchedVersion` (Veinminer
   Client), `userPropertiesFuture` (Language Reload's screen close), the window's SDL handle for
   `SDL_GetWindowFlags`/`SDL_GetWindowPosition` and `getPreferredFullscreenVideoMode` (Dynamic FPS,
   CreativeCore's `resizeGui`, Language Reload's options save).
3. Client join order: `ClientPlayConnectionEvents.INIT` fires before receivers may be registered
   (ShulkerBoxTooltip: "Cannot register receiver while not in game!"); `handleLogin` injections run
   with no client level (Sound Physics: `clientLevel` null).
4. FancyMenu's client→server packets: the headless client's `Connection.pendingActions` is null.
5. `/modconfig` is silent when a mod's factory gives no screen (Architectury, Balm, Forge Config API
   Port, Fzzy Config, BadOptimizations): say so.
6. `config/` in the game folder before mods initialise (Entity Culling, Presence Footsteps write
   `config/x` relative to it on a fresh folder).

**gpu** (`HorizonGpu`, textures, models)
1. Continuity: its reload listeners fail (`Invalid atlas id`, CTM reloader NPE), "the models couldn't
   be loaded", and its settings screen draws half ("Texture view does not exist").
2. Entity Model Features: `ModelManager` null on the headless client (`entityModels()`), so every
   EMF model falls back.
3. ImmediatelyFast: `onResourceLoadFinished` hook, `VanillaPackResources.fullResources()` null.

**entities** (`HorizonEntityRender`)
1. 3D Skin Layers' settings preview: a preview player's `ClientAvatarState` is null
   (`AvatarRenderer.extractFlightData`).
2. BadOptimizations: "a player's mod layers couldn't be drawn": `mannequinRenderers` null.

**worldgen** (the mirror's `ChunkMap`)
1. C2ME's `ChunkMap` constructor wraps: the light executor's name and scheduler are null in the mirror.
2. Lithium: "a client level has no chunk source" (view distance 0 outside 1..32) and its
   `Entity.vicinityCache` mixin field not made.

### entities
1. The rest of Horizon's arrows on mod mobs: Punch, tipped arrows' effects, piercing.
2. Iceberg/Legendary Tooltips' armour on its armour stand: `HorizonEntityRender.context` has no
   equipment renderer (gpu r7 found it).
3. Mod mob tick cost, next part: `Natives.chunkRevision` (a JNI call and a dimension string per
   chunk per view), then vanilla movement and collisions.

### worldgen (round 6 queue, still running)
1. Far terrain for packs that change the land's shape (Tectonic): Horizon's own noise reading
   the pack's worldgen files, not a Java call per column.
2. The quit through Java: quitting from a world, the title, a dedicated server's stop and a
   crash all end the process; how long each takes.

### pack
1. (round 7's 2) The tech mods that open no screen on use (More Barrel Variants, Heater, Chunk
   Loaders) and the block drawn with the missing texture.
2. (round 7's 3) The full pack on a real GPU: the user's PC run (bigpack) → fix what it shows.
   ALA Industrial's pipes there (no "over 16 MiB" in the log, pipes place).

### client
1. ~~**Dedicated servers' `logs/latest.log` is written by two writers.**~~ (client-12) Java's log4j uses
   Mojang's `log4j2.xml` (in `com/mojang/logging`'s jar: a `RollingRandomAccessFile` at
   `logs/latest.log`, relative to the working folder), which on a dedicated server is the
   server's folder, where Horizon writes its own `latest.log`: lines are overwritten (batch 7's
   `maps-server` lost "Done (" there; server.log had it). Give log4j a config of the bridge's
   (no file of its own, or `logs/java.log`) and keep mods' log lines in Horizon's log. Check
   `server`, `maps-server` and a client case.
2. BuildCraft's settings screen from the pack's log (r7 couldn't reproduce it from its jar).
3. Toasts from Java mods (Pufferfish's Skills; gpu r7 saw none drawn).

### mixins
1. The next "Rust does" lines from `mixin-table.tsv`, most-patched first (needs the pack's
   jars: ask the user for the table from his PC run if the sandbox has none).
2. The host's own in-frame player routed like guests (jump/teleport cancels); `revoke` for
   advancements.

### gpu
1. Pufferfish's Skills' category background.
2. Two more from the r7 scan: Cape Provider (player picture in picture), Puzzles Lib's
   pictures in picture (TSL Entity Status, Spell Engine after).

### upstream sync (one-off chat)
1. Sync 5 with upstream main (`bridge-upstream-sync-5`), regress, report.
2. `punch`'s lost first click after `/gamemode` or a teleport (sync 4 saw it; the case retries).

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

- **mixins 1** (`bridge-mixins-1`), by Claude for iaxivers: getting off vehicles and Better
  Combat's sweep. Works: sneaking in a boat runs the heads of `stopRiding`/`removeVehicle`
  (mixinmod keeps a player holding a stick seated, lets go with empty hands); one Better
  Combat swing hurts all three zombies in a row (the mirror's box queries didn't list the
  copies of Horizon's mobs). Doesn't: passive mobs aren't swept (Better Combat's own rule);
  each tick of a held sneak asks Java once. Outside the area: `HorizonServer.entities`'
  `candidates` (3 lines). Test: `regress.py --jars <jars> sweep dismount` (Better Combat,
  Cloth Config, playerAnimator; the mixinmod fixture).

- **mixins 2** (`bridge-mixins-2`, on top of `bridge-mixins-1`), by Claude for iaxivers: the
  most-patched "Rust does" lines of the 107-jar table, after `Player.attack` (done):
  `LivingEntity.addEffect` (6 mods) and `causeFallDamage` (5). Works: a player's fall damage
  goes through vanilla's `causeFallDamage` on the mirror when a mod patches it (mixinmod: no
  damage holding a feather, half the distance with a slime ball, 7 for 10 blocks with empty
  hands); potions, food, totems and `/effect` ask the heads of `addEffect` (mixinmod's poison
  immunity holds). Doesn't: beacons, conduits, raids and the Warden add effects without
  asking; mace/wind-charge falls and in-frame single player stay Rust's. Outside the area:
  the client lib's `landing.rs`/`motion.rs` (a `route_fall` flag and `FallEvent::Routed`),
  `lan_actions.rs`, `potions.rs` (a few lines each). Test: `regress.py --jars <jars> fall
  effects`.

- **mixins 3** (`bridge-mixins-3`, on top of `bridge-mixins-2`), by Claude for iaxivers: a
  regress case per routed action. Works: the new fixture case `mixins1` covers breaking,
  pickup, crop random ticks and entity use (new `MilkMixin`); with it every routed action has
  a case (list in docs/java-bridge.md), the fixture ones without downloaded jars. All 16
  mixin cases pass headless. Doesn't: use-on, orbs and precipitation are still only checked
  with real mods (Right Click Harvest, Clumps, Serene Seasons). Test: `regress.py --jars
  <jars> mixins1` (or the whole list).

- **gpu 1, map mods with Trinkets** (`bridge-gpu-1`). Works: Xaero's minimap, JourneyMap and
  VoxelMap with Trinkets Updated 4.2.1: the models load in ~9 s. Cause: Trinkets' client init is
  a Yumi entrypoint run from `Minecraft.<init>` at its options, which never ran; now fired after
  the registries' freeze. Also: client reload listeners after the models now run with them;
  JourneyMap's map was empty on `java-bridge` (the client player's `levelCallback` was `NULL`:
  no mapping task), fixed. Regress cases `maps`, `journeymap`, `voxelmap` (jars: Xaero's
  Minimap, Trinkets, JourneyMap, VoxelMap). Test: those four in `~/hz-test/mods-java`, join,
  wait ~1 minute: Xaero's map top left, JourneyMap's/VoxelMap's top right.
- **gpu 2, JourneyMap and VoxelMap per player** (`bridge-gpu-2`, on top of `bridge-gpu-1`).
  Works: host and LAN guest each see their own JourneyMap (map centred on them, own
  coordinates and biome) and VoxelMap (own map image, no black gaps). How: `HorizonClientState`
  slots can now make a fresh value per player (`field=Class(arg)`) and reach instances through
  static fields; a new optional mixin config (`horizon-platform-mods.mixins.json`, plugin
  `ModsPlugin`) makes JourneyMap's static final state non-final. Small change outside gpu:
  `regress.py` runs `lan=True` cases with `lan_test.sh` (which now honours `NO_BUILD`). Doesn't:
  a leaving player's VoxelMap map keeps its thread; JourneyMap's waypoints and settings are still
  shared. Test: `MODS=<JourneyMap + VoxelMap> tools/java_bridge/lan_test.sh '$H command "tp Guest ~40 ~ ~"; sleep 100'`,
  or `regress.py --jars … maps-lan`.
- **gpu 3, other GPU mods** (`bridge-gpu-3`, on top of `bridge-gpu-2`). Scanned 12 popular 26.3
  Fabric client mods; three draw with the GPU directly: Diorama, XaeroPlus, MiniHUD. Works:
  Diorama's minimap (its baked map tiles, compass, coordinates). It needed (useful beyond it):
  Fabric's `ClientChunkEvents` fired per player as Horizon's chunks come into view, the client
  chunk cache answering with Horizon's chunks, `GameRenderer.render()` HEAD injections run,
  GPU textures written from memory sent to the HUD, and players' client ticks held until the
  models load. Fixed on the way: `hasChunk` made a full copy per call (VoxelMap stalled the
  server thread with two players). Doesn't / not tried: Diorama's water colours unchecked
  against the real game; XaeroPlus and MiniHUD (needs MaLiLib) not tried. Regress case
  `diorama`. Test: Diorama alone in `~/hz-test/mods-java`, wait ~1 minute: the square map top
  right.

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

- **worldgen 5.1, startup** (bridge-worldgen-5): Java's classes are kept between runs in a
  class archive (CDS, `.horizon-cache/java-classes-<stamp>.jsa`, written as the game quits
  through Java's exit): 110 jars, Fabric start 89–92 s → ~80 s, join ~166 → ~150 s (2 cores);
  the first quit of a mod set takes ~25 s more. TerraBlender's unmemoized `possibleBiomes`
  memoized (server start hooks 3.3 → 2.2 s; the "10 s" was mostly elsewhere). `has_java_mods`
  cached (it listed every jar every 2 ms during the spawn search). Not done: the mixin
  transforms (~18% of Fabric's start) and shape caches (~27%); see java-bridge.md "Java's start,
  cached between runs". Also fixed: java_worldgen's test didn't compile (upstream's `Spans`).
  Regress: all pass but `preset` once (an oak log at the One Block's spot, 1 of 3 runs; passes
  alone twice). Test: start a pack twice; the log says "classes mapped from …" the second time.

- **worldgen 5.2, lost blocks** (bridge-worldgen-5): still none. `lost` passed in the full
  regress run and 3 more times (0 of 33–36 lost), and the repro on the 110-jar pack
  (`lost_blocks.sh`, 40 s wait): 0 of 27, no "generated again while loaded". No change.

- **worldgen 5.3, big pack far terrain** (bridge-worldgen-5): two findings. (1) Here (2 cores)
  far terrain draws nothing for minutes at render distance 12, mods or not: upstream's `Lod`
  holds meshing on ≤4-core machines while near chunks load; `horizon-ctl status` now shows
  `lod.near_loading(_parts)`. (2) The real gap: Tectonic's density functions change the land,
  far terrain still draws vanilla noise (ocean where the land is). A Java-side fix works but is
  far too slow (0.4 s–80 ms a column), parked on `bridge-worldgen-5-farnoise-wip`; the fix
  worth doing is Horizon's noise reading the pack's density-function JSON. Screens:
  `lod-bop5.png` (BOP far terrain fine), `lod-big5c.png` (big pack seam).

- **pack round 5, task 1: memory** (`bridge-pack-6`, 4f287e0cc, ff8506aca; Claude for
  iaxivers). Works: the behaviour JSON's tree (~380 MB at 577k states) is freed after the facts
  load; baked faces drop the sprite name and the rest of the `ModelQuad` (model catalog at 577k:
  579 → 385 MB); the registry was already done upstream (~350 B a state). The 155-jar pack
  with no JVM opens its world at 3.76 GB (was 5.2) and the player joins. Doesn't: with the JVM
  (~2.5 GB) it's still killed by this sandbox's ~6.2 GB limit; ~1.2 GB of that is llvmpipe's
  textures, so on a real GPU it may fit in 8 GB (untested). Left: render assets 683 MB. Test:
  `HORIZON_MEM_LOG=1` with the pack, read `memory after` lines; `cargo run --profile fast -p
  horizon-client-render --example texture_memory_probe -- <resources>`; regress `pot hud tech`
  pass.
- **pack round 5, task 2: lazy loot tables** (`bridge-pack-6`, 6d9e98480). Works: upstream's
  shared tables, overlays and key stay; tables are listed, then parsed (and the
  `native_loot_table_load` hook run) on first use. 208k-state pack: the loot stage +1 MB.
  Doesn't: nothing known. Test: horizon-world's loot tests; regress `pot` (Farmer's Delight
  loot), `skyblock` and `oneblock` pass.
- **pack round 5, task 3: skyblock join** (`bridge-pack-6`, 9ba115a97). Works: a new player
  also waits until the run a forced chunk stopped has gone through (no retry queued, two quiet
  ticks). Doesn't: the island still lands ~1 s after the join, because Skyblock Infinite's
  function waits for a player on purpose (`data get entity @p`, rescheduled each tick), as
  in vanilla. Not a bridge bug. Test: regress `skyblock oneblock` (pass, screenshot shows
  the island).

- **mixins 5-1** (`bridge-mixins-5-1`), by Claude for iaxivers: our hooks onto upstream's
  native hook points. Works: random ticks (`java_random_tick` in `before_random_tick`, before
  `native_random_tick`; horizon-world's `RANDOM_TICK_HOOK` removed), hurts (`java_hurt` first in
  native_hooks' `hurt_server`, past hurtServer's guards, as Fabric's `ALLOW_DAMAGE`; health.rs's
  `HURT_HOOK` removed), left clicks (`java_attack_block` beside `native_left_click_block`). New
  cases `punch` (eventsmod: poppies can't be punched) and `hurt` (magma doesn't hurt, `/damage`
  does); `seasons`, `mixins1` and the other 16 mixin cases still pass. Doesn't: entity
  attack/use (ours answer the packet, for Java's own entities too), deaths (two notes with the
  hub's tracker), commands and placing stay where they were. Outside the area: one line each in
  game.rs, lan_actions.rs, native_hooks.rs; horizon-world and health.rs lose our hooks. Test:
  `regress.py --jars <jars> punch hurt seasons mixins1`.

- **mixins 5-2** (`bridge-mixins-5-2`, on top of `bridge-mixins-5-1`), by Claude for
  iaxivers: effects from beacons, conduits, raids and the Warden ask the mods. Works: they go
  through `Game::add_effect_asking_mods` (the heads of `LivingEntity.addEffect`, only when a
  mod hooks it); `effects2`: mixinmod refuses a beacon's haste (its speed still comes) and the
  ominous bottle's Bad Omen (the bottle is still drunk). Doesn't: the turtle helmet's water
  breathing (asked each tick) and the trial omen swap don't ask. Outside the area: one line
  per site in beacon.rs, block_entity_anims.rs (conduit), raids.rs, sculk.rs. Test:
  `regress.py --jars <jars> effects2 effects`.

- **mixins 5-3** (`bridge-mixins-5-3`, on top of `bridge-mixins-5-2`), by Claude for
  iaxivers: falls. Works: a routed fall after a mace hit or wind charge burst cuts its
  distance at the impact point first (it stayed Rust's before); `FallEvent::Routed` carries
  the mod block landed on, and `routed_fall_damage` runs upstream's `mod_block_fall` on the
  mods' damage. `windfall`: a wind charge lifts the player ~6 blocks, the landing goes to the
  mods and does no damage. Also: around.sh's scenes kill the items their fills dropped (plants
  picked up into slot 0 made `veinminer`/`dismount` flaky). All 20 mixin cases pass in one run.
  Doesn't: `mod_block_fall` on the routed path isn't tested at runtime (it needs a native mod
  block). Outside the area: the client lib's `landing.rs`, `lan_actions.rs` (a few lines).
  Test: `regress.py --jars <jars> windfall fall`.

- **entities 5 (round 5, tasks 1 and 4: riders seen sitting, charged jumps, a flat `mount`
  lane)**, `bridge-entities-5`. Agent: Claude (iaxivers' chats). Works: each player's
  snapshot payload carries `riders` (player entity id → the mod's mob it rides); the games put
  those players on it as a passengers packet would (`extra.mount`, sitting pose), and clear it
  when they get off. A mod's horse-style mob (`PlayerRideableJumping`) jumps: the mirror charges
  the jump from the held jump key as `LocalPlayer.aiStep` does and calls `onPlayerJump` then
  `handleStartJump` on release (`HorizonEntities.ridingJump`); the rider's game draws the jump
  bar (`MountState::charge_jump`, shared with Horizon's horses). The fixture mount jumps
  (power in chat: "Mount jumps 81"). Task 4: the `mount` scene builds a flat stone lane 24
  blocks east (40 wasn't loaded) and drives 0.8 s (~11 blocks; at 2.5 s it ran off the lane in a full run). horizon-ctl `entities` now gives a mob's
  `passenger` and `mount`. Outside the area: `mounts.rs` (charge_jump split out),
  `remote_vehicles.rs`, `agent.rs` (2 fields), `HorizonServer.tick` (1 call), `lan_e2e.py`.
  Test: `regress.py --jars … mount mountjump`; `lan_e2e.py --java-mods <critter jar> --check
  javamount` (now also: the host sees the guest ride). Not done: the mod mob's hearts on the
  rider's HUD.
- **entities 6 (round 5, task 2: mod mobs and Horizon's mobs)**, `bridge-entities-6` (on
  `bridge-entities-5`). Agent: Claude (iaxivers' chats). Works, one way: Horizon's mobs within
  32 blocks of a player (128 a level, every level) stand in the mirror as shadows of their
  type (`HorizonNeighbours`: no AI, not ticked, moved and given their health each tick), so
  mods' mobs see them as any entity (`HorizonServer.entities` lists them): a mod's predator
  hunts Horizon's sheep, a mod's prey flees Horizon's fox. What a mod's mob does to one goes to
  Horizon (`CombatCaptureMixin` → `Natives.hurtMob` → `Action::HurtMob` → `java_hurt_mob`):
  Horizon's mob takes it as a mob attack, knocked back from the attacker, in its own level.
  During a routed player attack the shadows step aside for HorizonCombat's copies. New fixture
  mob `horizon_critters:hunter` (hunts sheep). Outside the area: `HorizonServer` (2 lines
  and a delegate), `CombatCaptureMixin` (mixins), `Natives`, `events.rs`, `lib.rs`. Test:
  `regress.py --jars … neighbours` (Horizon's sheep at 8, then killed by the hunter). Not done:
  the other way. Horizon's mobs don't see mods' mobs (its AI targets and flees only its own
  mobs and players), and a Horizon mob a mod's mob hurt doesn't strike back.
- **entities 7 (round 5, task 3: a bigger mob pack)**, `bridge-entities-7` (on
  `bridge-entities-6`). Agent: Claude (iaxivers' chats). The 134-jar pack plus 10 mob mods
  (Alex's Mobs, Wilder Wild, The Graveyard, Mo' Animals, Ender Zoology, Eternal Nether, Fish of
  Thieves, Variants and Ventures, Cosy Critters, Snuffles; 147 jars) loads and runs. Measured
  (`HORIZON_JAVA_PROFILE=1`, ~50 mod mobs near): entity ticks 11-17 ms a tick, the snapshot
  6.5-10 ms. Top three: (1) the snapshot of animated mobs (GeckoLib: Graveyard's magpie and
  nightmare, Alex's bison and cockroach, 0.5-1.5 ms each), (2) Naturalist's whale ticks
  (0.55-0.9 ms each, its body parts' collisions, as entities 1 found), (3) Graveyard's skeleton
  creepers (10-12 of them at 0.1-0.3 ms each; JFR shows only vanilla movement and collisions,
  nothing of the mod's own). Cut: the snapshot. A GeckoLib mesh's vertices are written as text
  at once (`HorizonJson.Raw`, `tenThousandths`) instead of a boxed Gson number each, and mobs
  far from every player are drawn again every 2nd tick past 24 blocks and every 4th past 48
  (`HorizonEntityRender.drawEvery`: their position still each tick, a hurt one each tick).
  Same 40 s scene (bison, cockroaches, magpies, nightmares at 10 and 40 blocks), JFR samples
  in the snapshot: 497 before, 286 after (-42%). Not cut: the whale's and skeleton creepers'
  ticks (vanilla's own movement code; the bridge's block reads are a part of it). Test:
  `regress.py --jars … entity gecko hostile mount carry ber`; to measure, a mobs pack and
  `HORIZON_JAVA_PROFILE=1` ("entity snapshot" lines).

- **gpu 4, XaeroPlus and MiniHUD** (`bridge-gpu-4`, round 5 gpu 1), by Claude for iaxivers.
  Works: XaeroPlus over Xaero's Minimap and World Map (minimap and M's World Map), MiniHUD with
  MaLiLib (its info lines top left). Fixes useful beyond them: a GPU texture blitted to the
  GUI is read back at the frame's end, not at its blit (XaeroPlus clears the World Map's target
  as it's added, then the map is drawn: read at once, it was black); Mixin's renamed handlers
  (`<mod>_` prefix dropped) are found (`handlerNamed`); `Gui.extractRenderState` TAIL
  injections run (`fireGui`) and `Minecraft.<init>` RETURN ones (`START_END`);
  `isGameLoadFinished()` is true once the models load; `Gui.minecraft` is set. Small changes
  outside gpu: `HorizonHooks` (mixins: `fireGui`, `handlerNamed`, `START_END`) and
  `HorizonClient` (client: `gameConfig()`, the GUI hook in `hud()`, the load flag). Regress
  cases `xaeroplus`, `minihud` (new); `maps diorama journeymap voxelmap` pass. Test: XaeroPlus +
  Xaero's Minimap + World Map in `~/hz-test/mods-java`, join, M; or MiniHUD + MaLiLib: the lines
  top left.

- **gpu 5, JourneyMap per player, VoxelMap's leaving thread** (`bridge-gpu-5`, on top of
  `bridge-gpu-4`; round 5 gpu 2), by Claude for iaxivers. Works (LAN, headless): a guest's
  JourneyMap settings are theirs in memory (its properties objects join `HorizonClientConfig`;
  a guest's never overwrite the host's files) and their waypoints are their own, in
  `waypoints/horizon-players/<uuid>/` of the world's JourneyMap folder (two new optional
  mixins in `horizon-platform-mods.mixins.json`); a guest who leaves has their VoxelMap thread
  stopped. Doesn't: a guest's JourneyMap settings last their session only (as other mods'); the
  shared `MiniMap` object may keep the host's minimap properties for its shape; JourneyMap's
  server-side `START_SERVER_TICK` NPE and VoxelMap's `sendWorldIDPacket` NPE (seen before) are
  still there. Small change outside gpu: `HorizonClientConfig` is public (the mixins read it).
  Regress `maps-lan` now also quits the guest and checks both (pass). Test: JourneyMap +
  VoxelMap, `tools/java_bridge/lan_test.sh '$G quit'`: "VoxelMap's map of a player who left
  stopped its thread" in the log.

- **gpu 6, Diorama's water colours** (`bridge-gpu-6`, on top of `bridge-gpu-5`; round 5 gpu 3),
  by Claude for iaxivers. Checked headless, each against vanilla's values: the client chunks'
  biomes (plains, water `3f76e4`, grass `91bd59`), Diorama's blended tints, its tint kinds
  (water → water resolver, grass, leaves), the fluid model's sprite and its image, and its
  block looks (water `b2b2b2` tinted, grass `959595` tinted). All match. Found and fixed: the
  client level's light engine gave no sections (`getDataLayerData` null; Diorama then takes
  sky 15 everywhere): now Horizon's light, a section at a time (new native `lightSection`).
  Not changed: shallow rivers still bake grey-lilac, likely Diorama's own "clear water" over a
  sand/clay bed. Needs the user's eye: real Fabric + Diorama 0.6.0, a world with seed 1234,
  `/tp -525 67 66`, compare the minimap's river with Horizon's (the screenshot in the report).
  Small change outside gpu: `horizon-jvm/src/lib.rs` and `Natives` (the native),
  `HorizonLight`. Regress `diorama voxelmap journeymap` pass.

- **client r5-1, run from a normal install** (`bridge-client-4`). Works (headless, screenshot):
  a launcher-like install (binary, research-lock.json, the preparation's `.local/minecraft` and
  `.local/horizon-client`), the game started as the launcher starts it (game folder,
  `--resources` only, no HORIZON_ROOT/JAVA_HOME/Java on the PATH): the bridge finds the install
  above `--resources`, fetches Fabric's jars, downloads Mojang's Java 25 (147 files, 26 s) into
  the launcher's runtime folder, and Farmer's Delight's blocks show. The launcher needs no
  change (it already starts the game that way; `mods-java/` in the game folder). Touched
  outside the area: `horizon-jvm` (runtime.rs, the download agent, find_java_home) and
  `Cargo.lock` (rustls/webpki-roots edges, no new crates). Doesn't: release builds with
  `java-mods` (CI needs a prepared game for the platform mod); no Mods page for `mods-java/`
  in the launcher. Test: `MODS=~/hz-test/mods-java tools/java_bridge/install_test.sh 'sleep 3'`;
  regress `pot join rei reiscreen` pass.

- **client r5-2, dedicated server: the rest of the client mods** (`bridge-client-5`). Found:
  every dedicated server with Java mods hung at "Preparing level" since the upstream merge
  (the spawn search moved a stream center a standalone server doesn't use), and then held
  each joining player two minutes. Fixed both (small changes in `initial_spawn.rs` and
  `lan.rs`, worldgen/server code: one helper, one chain). Works then (screenshots): Xaero's
  minimap, Jade, AppleSkin, Xaero's settings (Y), Nature's Compass's biome screen, Options →
  Mods' Java mods, all for a dedicated server's player. Not tried here: JourneyMap, VoxelMap,
  Xaero's World Map on dedicated (no jars in this sandbox; LAN works). Test: regress `server`
  (new: server=True cases), `MODS=... tools/java_bridge/server_test.sh '$G key y tap'`;
  `pot hud` pass.

- **client r5-3, REI's settings per player** (`bridge-client-6`). Works (LAN, screenshots):
  REI's config and craftable-only switch are each player's; the guest turns its filter on,
  the host's stays off with its full list. The copy no longer breaks drawing: REI's entry
  stacks, favourites and filtering rules are shared, and its config (held by REI's manager
  and AutoConfig) is copied once. Doesn't: REI's search runs on its own threads for all
  players, so a filtered search can briefly come out with another player's switch; the
  craftable search is very slow here (30+ s, 2 cores), so the guest's filtered list may lag.
  Test: regress `rei-lan` (new, LAN), `rei reiscreen modsettings` pass.

### Upstream sync 3 (2026-11-02, branch `bridge-upstream-sync-3`)

`origin/java-bridge` (fd41885b9, round 5) with upstream main fdd290b99 merged; replaces
`bridge-upstream-sync-2` (not merged: its changes are redone here). 34 files conflicted.
What upstream changed under us (sync 2's included):
- **The client lib is its own crate**, `horizon-client-lib` (root `client_lib.rs`, same
  folder); `horizon-client`'s lib is `game`, `entry`, `mod_api`, and `main()` lives in
  `entry.rs`: worldgen's quit through Java (`horizon_jvm::exit`) moved there. `horizon-ctl` is
  `horizon-client-lib`'s: build `-p horizon-client -p horizon-client-lib --features
  horizon-client/java-mods` (scripts, `regress.py` and java-bridge.md do now).
- pack-6: upstream's **lazy loot tables** (`LootTables(HashMap<String, LazyJson>)`) replace
  ours, `loot.rs` is upstream's. Our small `BakedQuad` stays; upstream's new users of the
  model's material adapted (Fusion's connecting textures by layer, `force_translucent` on
  `BakedFace` for OptiFine tiles, `baked_model_vertices`). The behaviours' second read from
  the file stays ours, inside upstream's startup timing.
- game.rs: the window's in-frame player tick is gone upstream (single player is on its
  server thread): `tick_java_server` stays after `native_tick`; the local-only
  `note_java_death` went with it (deaths come through the server's `note_java_death_cause`).
  Java's HUD draws before upstream's `gui_layer`s; the Java mount's jump bar moved to
  `draw_jump_bar`. `player_uuid` (dropped upstream) is java_server.rs's own now.
- Smaller: `Mob::knockback` takes the damage; world-gen settings take a single-biome preset;
  content blocks lay states out in property order (own-state blocks keep one row); menus
  report `item_crafted` beside `crafted_takes`; streaming's `priority` lost its loading arg;
  `lan_process_events` lost its state arg; the local chat and death message went upstream.

Tests: `regress.py --jars` all cases: **42 pass** (pot, terminal, ber, hud, rei, reiscreen,
join, modsettings, failmsg, keys, entity, hostile, mount, mountjump, neighbours, worldgen,
veinminer, harvest, carry, seasons, orbs, preset, skyblock, oneblock, attack, bettercombat,
ride, dismount, sweep, fall, effects, effects2, windfall, slot, snow, punch, hurt, mixins1,
mixins2, lost, rei-lan, prism) and **server** (with Xaero's minimap fetched for it). Skipped
(no jars here): gecko, maps, journeymap, voxelmap, diorama, xaeroplus, minihud, maps-lan,
bigpack, tech, gridlod. Clippy `-D warnings` clean with `java-mods`.
`horizon-client-render` lib tests: all pass, after two fixes of ours: a test still calling
`priority` with the loading arg, and the HUD atlas test that said mods' block textures stay
out (the bridge adds them since round 2; it failed on java-bridge too). Not run here (disk):
`horizon-client`'s gameplay tests.

- **one-off, Windows** (`bridge-windows-2`, off `bridge-upstream-sync-3`; first done as
  `bridge-windows-1`; Claude for iaxivers).
  Works: the bridge cross-checks for Windows (`x86_64-pc-windows-gnu`, horizon-client +
  horizon-client-lib with `java-mods`, clippy clean on horizon-jvm); `horizon-jvm`'s `boot` test
  built for Windows passes under Wine with Temurin 25 for Windows (`jvm.dll` loads, natives both
  ways). Fixed: `jvm.dll`'s C runtime found through the Java home's `bin` on the PATH; build.rs's
  javac takes an argument file (the platform mod's command line is ~28k characters with short
  Linux paths, past Windows' limit with real ones). Doesn't (not tested here): Fabric past
  Mixin's start (Wine lacks a network call the JVM's random seeding makes), the window, the
  launcher; non-ASCII paths into JNI options. Test on a Windows PC: java-bridge.md, "Windows".
  Redone on sync 3: round 5's Java download (`runtime.rs`) left `link_depth` unused on Windows
  (dead code, clippy failed): it's for Unix links only now. Clippy for Windows clean on
  horizon-client, horizon-client-lib and horizon-jvm; the `boot` test passes under Wine again.

- **entities 8 (round 6: Horizon's mobs see mod mobs; the rider's hearts; `preset`)**,
  `bridge-entities-8`. Agent: Claude (iaxivers' chats).
  1. Horizon's own mobs see the mods' living entities: the snapshot's `seen` lists each (id,
     box, place, health, Enemy or not) with the vanilla type it is taken for
     (`HorizonNeighbours.like`: its nearest vanilla class among zombie, skeleton, creeper,
     spider, villager, illager, wolf, sheep, chicken, rabbit, fish; else a spider for an Enemy,
     a pig for the rest), and Horizon's mob tick adds them to what its mobs see
     (`java_neighbours_seen`, one line in creatures.rs). So they target (an iron golem hunts a
     mod's Enemy), flee (by the type they're taken for) and strike back (HurtByTarget: the
     hurt's attacker is the mod mob's id). A Horizon mob's hit on one goes to the mirror
     (`mob_hits_mob` → `java_mob_hits_entity` → `HorizonNeighbours.hurtByHorizon`: a mob attack
     by the attacker's shadow, so the mod mob strikes back too). Regress `golem` (Horizon's
     golem kills crittermod's biter, now an Enemy) and `strikeback` (the hunter, no Enemy,
     bites the golem, which kills it). Not done: Horizon's projectiles (a skeleton's arrow)
     don't hit mod mobs; a mod mob that is no vanilla kind is seen as a pig or a spider.
  2. The rider's HUD shows the mod mob's hearts where the food bar was (the ride's `hp`,
     `draw_mount_hud`). horizon-ctl `status` gives `riding_java: [riding, [health, max]]`.
     Regress `mounthud` (status, and the mount's orange hearts in the screenshot).
  3. `preset`'s oak log: One Block's join sets grass at 0 64 0 while Horizon may not have
     loaded that chunk yet (a slow, busy run). Horizon refused the write and the mirror forgot it
     after 3 ticks; One Block's safety check (every 10 ticks: loaded and air → a random block)
     then put a random block (an oak log) there. Now a write into a chunk Horizon hasn't loaded
     waits for it on both sides (`apply_java_level_writes` keeps it ~20 s; the mirror keeps
     reading it and starts its 3 ticks at the load), as vanilla's setBlock loads the chunk.
     Couldn't make the flake show here (2 of 2 runs pass with two cores kept busy); the
     mechanism is from One Block's `OneBlockSafety.ensureOneBlock`.
  Outside the area: `creatures.rs` (2 hooks), `java_server.rs`, `HorizonServer` (writes,
  1 delegate), `mounts.rs`, `control.rs`, `events.rs`. Test: `regress.py --jars … golem
  strikeback mounthud preset`. Full regress: all pass but `mixins1` once (its wheat didn't grow
  in 6 s of a busy run; passes alone 3 of 3): its step now waits up to 20 s for the growth.

- **pack round 6, task 1: the full pack with Java in 8 GB** (`bridge-pack-7`; Claude for
  iaxivers). Works: the 148-jar pack (577k states) with Java now loads and joins here (it was
  killed before the join): Java's heap ran out in mods' worldgen, from DataFixerUpper keeping
  a rule chain per old structure DataVersion (250 MB; now cleared past 6 chains) and the
  mirror's baked models (~700 MB at 577k; skipped when the heap can't hold them, logged,
  `HORIZON_JAVA_MODELS=on|off`). G1 now gives unused heap back; `HORIZON_JVM_OPTIONS` passes
  options (`-Xmx3g`). Xaero's minimap draws (screenshot). Doesn't: it lives ~3 min in this
  sandbox, then passes its ~6.2 GB as chunks load (5.7 → 6.0 GB, ~1.2 GB of it llvmpipe's
  textures: should hold on a GPU, untested); with the models skipped, Xaero's World Map's
  frame hook fails (no ModelManager). Test: regress `fixer` (new: D&T, chains let go, new land
  still generates) and `bigpack` (now forbids `OutOfMemoryError` and the packet-rate kick);
  `pot worldgen lost skyblock tech` pass.

- **pack round 6, task 2: toward SkyFactory, more tech** (`bridge-pack-7`). Tried 12 more
  26.3 Fabric tech, storage and automation mods (plus deps, 22 jars), alone: XNet, Simple
  Copper Pipes, Chunk Loaders, XP Storage, ALA Industrial, Heater, HydroFarm, Ultimate
  Compression, FabricAutoCrafter, More Barrel Variants, Mineral Chance, Energetic Sheep (most
  other popular ones are already in the 148-jar pack). Works: they load and the player joins
  (regress `bigpack` with this folder); their blocks place and draw; XNet's controller and
  HydroFarm's autocrafter open their screens (screenshot). Fixed: McJtyLib's machines threw on
  use (the mirror's `ChunkMap.updatingChunkMap` was null: fastutil fields are now filled on
  mirror objects). Doesn't: ALA Industrial's content is dropped (its pipes' 76k-state
  declarations pass the 16 MiB fence; needs a compact own-states form, see java-bridge.md);
  More Barrel Variants' barrels, Heater's heater and Chunk Loaders' loader open no screen on
  use; one of the 12 blocks draws the missing texture. Test: regress `xnet` (new).

- **mixins 6-1** (`bridge-mixins-6-1`), by Claude for iaxivers: the rest of our hooks onto
  upstream's hook points. Works: deaths (`java_player_died` right before `native_player_died`,
  one call instead of three notes; `death`: eventsmod's DeathMixin hears a lava death by its
  message and a fall death by the tracker), placing (`java_block_event` right before
  `native_place_block`). Commands and `java_attack_mob` were already beside the native
  decision. Entity attack/use at the packet (Java's own entities) has no fit: a proposed hook
  point `native_unknown_entity_interact` is written up in java-bridge.md for Ludovic. Test:
  `regress.py --jars <jars> death punch hurt`; all 18 older mixin cases still pass.
- **mixins 6-2** (same branch, same commit), by Claude for iaxivers: the turtle helmet's
  water breathing and Bad Omen → Trial Omen ask the mods. Works: `effects3` (new scene
  `omen`): mixinmod refuses the helmet's water breathing and the Trial Omen; the spawner
  still turns ominous and takes the Bad Omen. Found and fixed on the way: on a server thread
  the swap never reached the guest (it changed the serving game's own effects, by the world
  owner's UUID). Unit test `a_refused_trial_omen_still_takes_the_bad_omen`. Outside the
  area: `trial_spawner.rs`, `potions.rs` (a few lines). Test: `regress.py --jars <jars>
  effects3 effects2 effects`.

- **mixins 6-3** (`bridge-mixins-6-3`, on top of `bridge-mixins-6-1`), by Claude for
  iaxivers: the next "Rust does" lines by count. Works: a player's jump
  (`jumpFromGround`, 4 mods) and teleport in its level (`ServerPlayer.teleport`, 5) fire their
  heads and ends on the mirror (`jump`: mixinmod hears two jumps; `teleport`: it hears `/tp`
  at the new place). All 24 mixin cases pass in one run. Doesn't: `PlayerAdvancements.award`
  (mostly mid-method; the mirror has no advancements), `tickEffects`, `openMenu`, teleports
  across dimensions; a head's cancel doesn't undo Horizon's jump or teleport. Fabric API
  hooks `ServerPlayer.teleport` itself, so `teleports` is on in every Java run (one call per
  teleport). Outside the area: `lan_actions.rs` (a few lines). Test: `regress.py --jars
  <jars> jump teleport`.

- **gpu 7, more GPU mods** (`bridge-gpu-7`, round 6 gpu 1), by Claude for iaxivers. Scanned the
  300 most downloaded client-required 26.3 Fabric mods' jars (nested jars too) for their own
  render passes, pipelines, pictures in picture and targets; tried five that draw on screens:
  - CC: Tweaked: works. Its computer's terminal draws and typing runs commands
    (`mkdir typed` lands on the computer's disk). Fix useful beyond it: Fabric's extended menus
    get their client menu from the opening data. Regress `computer` (scene `computer`).
  - Legendary Tooltips (Iceberg, Prism): partly. Java screens' tooltips take vanilla's path
    when mods hook it: centred title, rarity border. Its 3D model for equipment doesn't draw
    yet. Regress `tooltips`.
  - Item Collectors (SuperMartijn's core lib): its screen works; the lib's picture in picture
    isn't used there.
  - Gnetum: loads, nothing to see: it caches vanilla's HUD, which Horizon draws itself.
  - Advanced Loot Info (with REI): doesn't. Its REI plugin asks the server for its data from
    REI's reload thread, where no player's client runs: the request is dropped, and it waits
    (3 x 30 s).
  Small changes outside gpu: `HorizonClient`/`HorizonServer` (extended menus: client area),
  `game/control.rs` + `game/java_screen.rs` (`type` into mod screens), `HorizonGraphics`'
  tooltips, a scene in `screen_test.sh`. Regress `pot terminal reiscreen` pass with it. Test:
  CC: Tweaked in `~/hz-test/mods-java`, place `computercraft:computer_advanced`, right-click,
  type `ls`.

- **gpu 8, map mods on a dedicated server** (`bridge-gpu-8`, on top of `bridge-gpu-7`; round 6
  gpu 2, client 3's map half), by Claude for iaxivers. Works (server_test.sh, headless): a
  dedicated server's player gets JourneyMap (minimap, its About screen, then the full-screen
  map), VoxelMap, Xaero's minimap and World Map, XaeroPlus, Diorama and MiniHUD. Fixed:
  JourneyMap's server side failed each tick (`LoadedChunkOverlayPlugin` walks
  `ChunkMap.visibleChunkMap`, null in the mirror: now the chunk map's fastutil maps are made
  empty). Not ours: JourneyMap + VoxelMap in one game fail as in the real game (both register
  `worldinfo:world_id`). Small change outside gpu: `HorizonServer.chunkMap` (pack/mixins).
  Regress `maps-server` (new), `journeymap` pass. Test: JourneyMap, Xaero's Minimap and World
  Map in the server's `mods-java/` and the player's, join, M; J twice (About, then the map).

- **client r6-1, release builds with Java mods, the launcher's Java mods page** (`bridge-client-7`).
  Works: a game built with no Minecraft jar on the machine (any CI) carries the platform mod's
  sources and builds them on the player's first start with Mojang's Java, against their own
  game (~11 s once); tested headless with no Java installed (Java downloaded, platform built,
  Farmer's Delight's pot opens). `game-build.yml` has an opt-in `java_mods` input that adds
  Linux and Windows `-java` assets; nothing calls it yet (release/nightly signing is the
  maintainer's call). Launcher (screenshots): a Java mods tab on Home for an instance with
  `mods-java/` (jars with Fabric names, Add/drop, Open folder, Remove to the trash, the last
  game's failures on their rows) and `horizon-launcher java-mods list|add|remove|failed`;
  clippy and its tests pass. Doesn't: the launcher can't yet install a `-java` build (no
  signed assets exist); the tab only shows when `mods-java/` exists. Test:
  `HORIZON_BRIDGE_PLATFORM=source cargo build ... --features java-mods` then
  `tools/java_bridge/install_test.sh`; regress `pot join rei hud` pass.

- **client r6-2, REI's search for all players** (`bridge-client-8`). Works (LAN, screenshots,
  regress): each player's filter applies to their own list (the guest with 4 oak logs sees 2
  craftable entries, the host its full list). Speed: REI's display lookup index was off (its
  default), so the craftable filter scanned all displays (30+ s on 2 cores); it is turned on
  and rebuilt after each sync: a craftable search is ~0.3 s. With 2+ players searches run on
  the server thread inside the player's turn, so a craftable search is a ~0.3 s hitch for
  everyone (single player: REI's threads as before). Test: regress `rei-lan` (stronger now),
  `rei reiscreen` pass.

- **client r6-3, map mods on a dedicated server** (`bridge-client-9`). Not done: JourneyMap,
  VoxelMap and Xaero's World Map aren't in this sandbox (downloaded mod jars aren't run here),
  so only a regress case was added: `maps-server` (server=True: the four map mods for a
  dedicated server's player, the World Map opened with M; checks the player plays and no
  per-player state errors). Xaero's Minimap already works there (regress `server`). Test on
  the PC: `python3 tools/java_bridge/regress.py --jars ~/hz-test/mods-java maps-server`, then
  look at /tmp/hz-regress/maps-server/server-*.png.

### Pack (2026-11-02, round 7 task 1, branch `bridge-pack-8`)

ALA Industrial past the 16 MiB fence. A block with 1,024+ own states is declared as a compact
`state_table` (properties' values and distinct shapes once, one index row per state) and read
back into the same `State`s, so nothing downstream changed. Tests: `horizon-mods`
`a_state_table_reads_as_its_states`, `horizon-port-pack`
`many_own_states_fit_the_content_fence_and_read_back` (16k-state pipe, < 1/4 the bytes,
identical states after a real `.hmod` round trip). Content `FORMAT` 13 → 14 (rebuilds once).
Not run in-game: this sandbox had no JDK 25 / `.local` and fetching a JDK was refused, so no
regress or screenshot. To test: ALA Industrial in the mod set, check the log has no "over 16
MiB" and its pipes place. Tasks 2 and 3 not started (2 needs the game, 3 the user's PC run).

### Upstream sync 4 (2026-11-02, branch `bridge-upstream-sync-4`; Claude for iaxivers)

`origin/java-bridge` (bcd6589d9, round 7 queue) with upstream main 97937bcf8 merged (327
commits). No conflicts, and nothing in the bridge needed changing: it builds (`-p
horizon-client -p horizon-client-lib --features horizon-client/java-mods`), clippy `-D
warnings` clean, and cross-checks for Windows (`x86_64-pc-windows-gnu`: horizon-client +
horizon-client-lib with `java-mods`, horizon-jvm), clippy clean there too.
What upstream changed near the bridge (all additive): native-mod hooks for screen lifecycle,
camera, hand pose, movement input and FOV; entity joins/leaves for native mods now flushed
once per tick phase (`native_joins_flush`/`native_leaves_flush`) instead of id diffs; fence
knots answer a guest's Interact by id; a client's ContainerClose also closes dialogs; the
options and graphics menus rewritten (`graphics_ui.rs`); streaming forgets columns in a batch.
Tests: regress, the 25 cases that need only the repo's fixture mods: **25 pass** (keys,
entity, hostile, mount, mountjump, neighbours, golem, strikeback, mounthud, attack, ride,
dismount, fall, effects, effects2, windfall, effects3, jump, teleport, slot, punch, hurt,
death, mixins1, mixins2). Not run: every case with Modrinth jars, the sandbox here refused
to run downloaded mod jars; run them on a PC (`regress.py --jars <jars>`).
- `punch` failed 3 of 4 runs after the merge: the first left click after the scene was lost
  (the poppy stayed, but AttackBlockCallback never ran; a second click works, a 4 s wait
  once did, 3 s didn't). Nothing upstream in the dig path changed; cause not found. The case
  now clicks up to 3 times (`retries.txt` in its scratch says how many it took). If a player
  sees a first click after `/gamemode` or a teleport do nothing, this is it.
- Windows (task 2): only the cross-check above; Wine wasn't set up here, so the `boot` test
  and Fabric past Mixin's start weren't tried. The steps for a Windows PC are unchanged
  (java-bridge.md, "Windows").
- Setup on a fresh container, for the next sync: Java 25 (`openjdk-25-jdk-headless`),
  `libasound2-dev libudev-dev libxkbcommon-dev xvfb mesa-vulkan-drivers`; the vanilla files
  from Mojang's manifest laid out as a launcher folder (`versions/26.3/`, `libraries/`,
  `assets/`) for `horizon-prepare --prepare --minecraft-dir <it> --server-jar <it>`; then
  build again (`touch crates/horizon-jvm/build.rs`): a build before `.local/` exists skips
  the Fabric API modules and every fixture fails with "no Fabric API modules".
- **entities 9 (round 7: projectiles both ways; mob tick cost)**, `bridge-entities-9`. Agent:
  Claude (iaxivers' chats).
  1. Horizon's projectiles hit mod mobs: arrows, tridents, snowballs, eggs, fireballs meet the
     mods' living entities (`java_projectile_targets`; a mob a player rides is left out) and hurt
     them on the mirror (`HorizonNeighbours.hurtByProjectile`) by a transient projectile of the
     same type owned by the shooter: vanilla's own damage source, knockback along the flight, a
     burning arrow ignites, the mob strikes back at the shooter. Also fixed on the way: a
     right-click on a mod mob ended the click, so a bow drawn at one did nothing; with an item
     aimed past entities (bow, crossbow, trident, throwables, shield...) the item is used too.
     Regress `arrows` (the player's bow kills crittermod's biter). Not done: Punch, tipped
     arrows' effects, piercing on.
  2. A mod's projectile on players and Horizon's mobs: worked already (the projectile is a
     Java entity; its search finds mirrored players and shadows, whose hurts go to Horizon).
     crittermod gets a `spitter` throwing its own `Pebble` (ThrowableProjectile, 2 damage).
     Regress `spit` (player in survival loses health) and `spitsheep` (Horizon's sheep dies).
  3. Whale and skeleton creeper ticks: jcmd samples of the server thread (no JFR in the
     bridge's JVM here) put ~40% of a whale's tick in the bridge's block reads, and a JNI call
     per read (`Natives.threadEpoch`) was 5-13% of all mod entity ticks. The view counter is
     now read in place (`Natives.threadEpochBuffer`: a direct buffer over Rust's thread-local).
     Same scene (4 whales, 10 skeleton creepers), 400 samples each: block reads 21% → 18% of
     entity-tick samples, threadEpoch 5% → 0%. Wall-clock medians here swing ±30% run to run
     (2 shared cores), so no ms claim. Left: vanilla movement and collisions, Naturalist's
     `resolveBodyCollisions`, the mixin's handler (~9% self), `Natives.chunkRevision` (JNI +
     a dimension string per chunk per view).
  Outside the area: `creatures.rs` (targets + one hit branch), `remote.rs` (the right-click),
  `HorizonServer` (delegate, `Sections.epoch`), `Natives`, `horizon-jvm` `lib.rs`/`world.rs`,
  scene `range` in `screen_test.sh`. Test: `regress.py --jars … arrows spit spitsheep golem
  strikeback mounthud`. Full set here (16 cases) passes but `lost`: 0 of 28 lost, only 28 set
  in time on this 2-core box (30 needed; same with the old jars).
- **mixins 7-1** (`bridge-mixins-7-1`, round 7 mixins 1 and 2), by Claude for iaxivers.
  Works: a mod's HEAD cancel undoes a jump (`jumpStart` before the guest's moves: no server
  jump, statistic or exhaustion, the guest put back where it took off; the ends only for jumps
  made) and a teleport, in the level (the guest keeps place and look), to another dimension
  by command and through a portal (`teleportStart`/`teleportEnd`, the new level in the
  `TeleportTransition`). Advancements on the mirror (routed action `advancements`): vanilla's
  whole `PlayerAdvancements.award` runs there before Horizon grants a criterion, progress
  copied from Horizon's, rewards and chat left to Horizon; a refusal keeps it ungranted, an
  injection inside `award` (before the rewards) is heard. New cases `jump2`, `teleport2`,
  `advancement` (mixinmod: slime ball, x 666, Acquire Hardware); the older mixin cases rerun.
  Doesn't: task 3 (the next "Rust does" lines): this machine has none of the pack's jars, so
  no `mixin-table.tsv` to rank; the host's own in-frame player isn't routed; `revoke`.
  Outside the area: `advancements.rs`, `lan_actions.rs`, `server_travel.rs` (a few lines
  each). Test: `regress.py --jars <jars> jump2 teleport2 advancement jump teleport`.
- **gpu r7, tasks 1-3** (`bridge-gpu-9`, on `java-bridge` bcd6589), by Claude for iaxivers.
  Java and regress.py only (no Rust change). Tested headless here (llvmpipe), screenshots.
  - Task 1, Legendary Tooltips' 3D model: works for tools, weapons and other items (an iron
    sword turns in its box). Needed: the client's item model resolver over the mirror's
    models, the GUI's lights set from the start (pipelines reading `Lighting` drew nothing),
    `Minecraft.renderFrame` injections run each client tick. Armour doesn't: Iceberg draws it
    on an armour stand through the entity renderers, whose context has no equipment renderer
    (entities area: `HorizonEntityRender.context`). Regress `tooltips` (stronger).
  - Task 2, World Map without the models: works. Models are now baked on first use when they
    don't fit (`HorizonLazyModels`, two mixins) instead of skipped: there's always a
    `ModelManager`; Xaero's maps baked ~1k of 36k states. Regress `worldmap-lazy`
    (`HORIZON_JAVA_MODELS=off`). Not tried on the big pack here (it doesn't fit this sandbox).
  - Task 3, more GPU mods: one of three. Scanned the 300 most downloaded client-required 26.3
    Fabric mods again (44 reference renderpearl drawing; most draw the world, which Horizon
    draws itself). Pufferfish's Skills works: K opens its tree, item icons (oversized items on
    the GPU), lines at their angle. Not drawn: its category background; toasts. Its Default
    Skill Trees pack needs Pufferfish's Attributes (no 26.3 build). Regress `skills`. Left for
    next time from the scan: Cape Provider (its player picture in picture), Resourcify (pack
    browser: needs a way into vanilla's pack screen), Puzzles Lib's pictures in picture, TSL
    Entity Status, Spell Engine.
  Outside the area (small): `HorizonLoop` (messages carried by their codecs, client joined
  before its first message), `HorizonHooks` (`renderFrame` in the client methods),
  `HorizonClient` (toast manager). Regress `skills pot terminal computer rei reiscreen hud
  tooltips worldmap-lazy maps xaeroplus` pass. Test: build, then
  `regress.py --jars ~/hz-test/mods-java tooltips worldmap-lazy skills`.
- **client r7, round 7's three** (`bridge-client-10`), by Claude for iaxivers. Headless, this
  sandbox (2 cores, lavapipe), screenshots:
  1. Advanced Loot Info works: its REI plugin's request from REI's thread is queued
     (`HorizonLoop.INBOX`, as the player whose client ran last) and answered at the next
     flush; Horizon's REI reload pumps the queue while it waits. Block Drops pages show, REI's
     reload ~3 s (was 3 x 30 s of the plugin's timeouts). Regress `lootinfo`.
  2. Mods' settings screens: `getProvidedConfigScreenFactories` listed (a mod's own screen
     wins), methods called through the interface (non-public implementations), broken
     entrypoints skipped as Mod Menu does, a parent screen given (`HorizonModsPage`), Done
     goes back to Options → Mods (small Rust change in `java_screen.rs`). Compressed Blocks
     3.1.0 has no settings at all (its "modmenu" class is empty): nothing to show, now one
     quiet line instead of an error. BuildCraft (BCRefabricated 26.10.15) has no "modmenu"
     entrypoint: its failure wasn't reproducible from its jar; the pack's log would say which
     path. Regress `modsettings2` (fixture `configmod`).
  3. A LAN guest's kept config files are read at their next join (AutoConfig's holders
     `load()` with their files laid over `config/`, then `config/` put back). Regress
     `config-rejoin`: guest rejoins, REI's list on their left, the host's on the right.
  Test: `regress.py --jars ~/hz-test/mods-java lootinfo modsettings2 config-rejoin` (needs ALI,
  Advanced Core Info, REI, Architectury, Cloth Config).

### Merge batch 7 (2026-11-03, boss; Claude for iaxivers)

Merged: upstream sync 4, entities-9, mixins-7-1, gpu-9, client-10 (pack-8 was already in).
Conflicts: docs (both kept), `HorizonLoop.java` (gpu's `carried()` + client's `INBOX`/`pump()`).
`check_public_tree.py` now allows a fixture's `compile-only/` stubs (client-10's Mod Menu API).
Built here (2 cores, lavapipe); fmt and clippy clean. Regress, 30 cases (round 7's, `server`,
`maps-server`, `maps-lan`, `rei-lan`, the fixtures): all pass except `maps-server`'s "the server
starts" — a real bug, queued (client 1): Java's log4j and Horizon both write the server's
`logs/latest.log`, "Done (" was overwritten (server.log has it; the maps draw). `jump2`,
`attack`, `worldmap-lazy` failed in the long run on timing and passed alone; `teleport` was
cut off and passed alone.
- **worldgen 6.1, far terrain for Tectonic** (bridge-worldgen-6): far terrain now uses the
  mods' noise. Java writes the dimension's noise settings, density functions and noises as a
  data pack (mod types made vanilla, Tectonic's noise seeds kept: `horizon_seed`), and Horizon's
  own noise reads it. Values identical to Java's at test points; the seam at the render
  distance is gone. Costs a second worldgen data load (13–44 s on a far thread, ~0.9 GB):
  sharing it is the follow-up. Touches `horizon-core` (noise seed alias) and `horizon-world`
  (`noise_seed`), small. Test: a pack with Tectonic, far terrain on, fly up; log says "far
  terrain from the mods' noise".
- **worldgen 6.2, the quit through Java**: world quit 3 s (7 s writing the archive), title
  1.3 s, SIGTERM 3.4 s, SIGINT 2.4 s, SIGSEGV 9.4 s, dedicated `stop` 6.4 s: all end. Details
  in java-bridge.md.

### gpu 10: invisible mod blocks (2026-11-03, branch `bridge-gpu-10`; Claude for iaxivers)

Task: the user's PC run, "mod blocks invisible when placed until a relaunch, a few blocks in a
swamp invisible". Here (2 cores, llvmpipe, the 81-jar pack, fresh and cached content):
- **Found and fixed: a placed mod block turned into air in the player's game.** A mod's block
  item places in the mirror; its `SetBlock` waited for the next tick while the click's
  correction (air) and ack went out at once, so the predicted block was replaced by air (polled
  every 50 ms: stove, air, stove). It came back a tick later here; it stayed gone wherever that
  later update didn't arrive. Now the use's writes are applied before the click is answered
  (`java_server.rs` `apply_java_use_writes`). Regress `placed` (Farmer's Delight; the same poll on the
  old build read air). `pot ber terminal hud placed` pass.
- **Ruled out:** missing models (`state_mesh_scan`: all 110,458 states mesh and light, no
  panic; unbound states are only vanilla's own empty ones), state ids past 65,535 (FD 78,866,
  Tom's 110,406 render), sprites in the 2nd texture array, GPU occlusion culling
  (`HORIZON_GPU_CULL=1`), placing in empty sections, holding right-click (client == server).
  Meshes here wait only while a neighbour column isn't loaded yet (vanilla's rule; slow here as
  Java generates).
- **Not reproduced:** a block staying invisible until a relaunch, and the swamp blocks (a
  vanilla swamp at -183 1730, seed 1234, looked right). Needs the user's answer: which blocks,
  can they be targeted (outline/Jade), which swamp; his `logs/latest.log` (`panicked`,
  `missing block model`, `Java mods: a mod set`).
- Also: `research-lock.json` accepts Mojang's newest republished 26.3 version JSON (a fresh
  machine's `lab.py prepare` refused it).
Test: `regress.py --jars ~/hz-test/mods-java placed`; in-game, place mod blocks by hand.

### Mixins 8 (2026-11-03, branch `bridge-mixins-8`; Claude for iaxivers)

**Hoppers.** Not Lithium. Headless with the 81-jar pack, Horizon's own hoppers work as without
mods: chest → hopper → chest, item pickup, hand-placed and sneak-placed hoppers, filling a
chest or hopper through its screen (a mirror menu: this pack routes every block use), an
auto-smelter, after a save and reload; tick-exact counts (`tick freeze` + `tick step`) are the
same with the pack (Lithium in), with Lithium alone and with Fabric API alone. What didn't work:
**any Java mod's container**: a hopper into Farmer's Delight's cooking pot did nothing (Horizon
has no block entity there). Now Horizon's hoppers push into and pull out of Java mods' blocks
through Fabric's `ItemStorage.SIDED` (`HorizonHoppers`; java-bridge.md, the mixin section).
Tested: hopper → cooking pot (beef in the pot's screen), hopper → FD oak cabinet → hopper →
chest, in FD alone and in the full pack. Regress: new case `hoppers` and `pot` pass. Touches
`machines.rs` (two hooks, ~40 lines: shared Horizon file). Not done: droppers into Java blocks;
hoppers on Java mods' entities (minecarts of mods). (Both done in mixins 9.)
Test on the PC: pull, build, start the pack; a hopper with items on a cooking pot or an FD
cabinet fills it; a hopper under the cabinet empties it. Other mods' machines (Energized
Power) not tried yet: they work if they expose Fabric's item storage.
Setup note: Mojang republished 26.3's version JSON again (sha1 `80608ff…`, asset index
`331e5c8…`); `lab.py prepare` refuses it until `research-lock.json` accepts it (not changed
here: upstream's file).

### Worldgen 7: mods' overrides of vanilla structure templates (2026-11-04, `bridge-worldgen-7`)

Beta testers' bug: The Sift's ancient city centres were vanilla's in new worlds. Two causes:
- Horizon's own structure code read templates only from the prepared vanilla data. Now data
  packs' `structure/*.nbt` override them (world packs and the Java mods' jars; the jars give
  Horizon's generator their templates, template pools and processor lists only).
- None of The Sift's blocks loaded: its `ichorlogged` property on vanilla leaves made a
  carried block's shape unknown and failed its content mod (pack area, `mod_content.rs`,
  small: falls back to the vanilla state). So its sonorous deepslate came out as air.
Regress `templates` (The Sift + Fabric API + GeckoLib): /locate an ancient city, go there,
8 `the_sift:sonorous_deepslate` found at the centre (vanilla's centre has none); the
overworld is Horizon's generator ("generates as vanilla's"). Unit tests in `overrides.rs`,
`mod_content.rs`. Also: `research-lock.json` accepts Mojang's newest 26.3 JSON republish
(same jars, asset index 331e5c8…). Not done: `/place template` from mods' data, mods' new
structures/structure sets in Horizon's generator (a structures-only mod in an otherwise
vanilla overworld still gets none).

### Client 11: JEI over Horizon's inventory (2026-11-04, branch `bridge-client-11`; Claude for iaxivers)

JEI (31.17.0.108, Fabric 26.3) works like REI: its item list beside Horizon's inventory, its
search box (typed in), clicks on entries (left: recipes, right: uses), R/U over its entries and
over Horizon's slots, its recipe screens (Escape back to the inventory), `/recipes` and `/uses`.
It never started: its start listener was registered from an injection into `Minecraft.<init>`
the headless client never ran. Now started with the mirror's recipes as a joining client is
(`HorizonJei`). Shared fixes: Fabric's screen input/tick/frame events are fired for every
screen (`HorizonScreenEvents`), client levels have the server's tick rate (potion tooltips threw),
reload listeners added at `CLIENT_STARTED` run. Search text and keyboard focus per player.
Not yet per player: JEI's bookmarks and settings. Regress `jei`, `jei-lan` (new), `rei`,
`reiscreen`, `pot`, `terminal`, `rei-lan`, `modsettings`, `config-rejoin`, `keys`, `failmsg`.
Test: `regress.py --jars <jars with jei, fabric-api, Farmer's Delight> jei jei-lan`.

### Pack 9: a modpack's resource packs, the test window's frame rate (2026-11-04, branch `bridge-pack-9`; Claude for iaxivers)

1. **A modpack's bundled resource packs are picked up.** Prism (wrapper and `--instance`):
   `horizon-play` links the instance's `resourcepacks/` into the game folder (or links its packs
   into an existing `horizon/resourcepacks/`, where a tester copied packs by hand) and turns on
   the instance's selection from its `options.txt` (first start, and when the pack's selection
   changes; the player's own choice in Horizon stays otherwise). A pack's folder for the test
   scripts (`resourcepacks/` and `options.txt` beside `MODS`, e.g. `~/hz-test/`): `launch.sh`
   does the same. A game folder that holds `mods-java/` with `resourcepacks/` and `options.txt`
   (`~/hz-play`) already worked: the game reads both from its working folder.
2. **The test window's frame rate is capped.** Test games get `maxFps:30` (`fps_cap.sh`, used
   by launch/lan/server/install tests; `MAX_FPS=n` to change). And the offscreen loading screen,
   which drew with no limit at all (a big pack's minutes of loading at 99% GPU), is held to
   vanilla's 60 for a menu (small change in game.rs, `Loading::about_to_wait`, outside pack).
   Headless check: the limiter holds offscreen (maxFps 10 → 10.0 FPS measured by `bench`).
Regress (this sandbox, lavapipe; FD + Fabric API): `prism` (now checks the packs and their
selection), `packs` (new: a stone wall in the pack's checkered texture; maxFps 30), `pot`, `ber`
pass. Unit tests in `horizon-play` (`packs.rs`). Test on the PC:
`regress.py --jars ~/hz-test/mods-java prism packs pot`, or Prism: put a resource pack in the
instance (Prism's Resource packs tab, turn it on), press Play: it is on in Horizon.

### Mixins 9 (2026-11-04, branch `bridge-mixins-9`; Claude for iaxivers)

1. **The FD meal never eaten** (beta tester): reproduced headless (hunger 0, beef stew and
   cooked bacon held: nothing eaten, the stack stayed). Cause: a mod item's use in the air
   runs vanilla's `useItem` on the mirror; a food *starts* the use on the mirrored player
   (never ticked, so it never finishes) and the answer "taken" kept Horizon from eating. Now a
   mod's consumable the mirror started is handed to Horizon, which eats it from the carried
   components (food, effects, the bowl). Stew: hunger 0 → 12, bowl back; bacon feeds. Regress
   `eat` (fails on the old jars). Not done: items whose `finishUsingItem` adds code beyond
   the components (FD's hot cocoa, milk bottle), and non-food held uses of mod items (bows).
2. **Droppers into Java mods' blocks** (Fabric's `DropperBlockMixin`: 3 carrots into an FD
   cabinet) and **hoppers on mods' container entities** (a hopper fills a mod's cart, a hopper
   under it empties it; hopper minecarts too). Horizon's droppers also fill Horizon's chest
   minecarts in front now (vanilla's `getContainerAt`). Fixture `cartmod`, regress
   `droppers`. Found: a mod's minecart isn't drawn (`MinecartRenderer` NPE,
   `blockModelResolver` null in the headless client): client/gpu.
3. **Mods' fluids flow** (the Create honey entry, flow half): reproduced with BuildCraft
   Refabricated's oil (Create Fly has no 26.3 Modrinth build; CurseForge can't be fetched
   here): the oil spread `minecraft:lava` and water made cobblestone. Now horizon-world has a
   `LevelAccess::foreign_fluid` hook (shared file: `level/{mod,fluids,update}.rs`, ~30 lines,
   defaults keep Horizon's behaviour) and the mirror runs mods' fluid ticks with their own
   code: the oil spreads as oil (levels 1-7, radius 7), BuildCraft's gaseous fuel rises 8
   blocks to a glass ceiling and spreads under it, water beside oil makes nothing. Regress
   `fluids`. Should work the same for Create's honey and chocolate (their own FlowingFluids):
   not tried (no jar here). Still open: drawing (gpu), entities in mod fluids (Horizon's physics
   read the template: lava burns), Horizon's water spreading into them as into lava,
   milk-lib not carried (its id has a `-`).

Regress here: `eat droppers fluids hoppers pot placed` pass. horizon-world `level::` tests:
fluids pass; 6 piston tests fail on java-bridge without this branch too.
Test on the PC: pull `bridge-mixins-9`, build, then
`python3 tools/java_bridge/regress.py --jars ~/hz-test/mods-java eat droppers fluids hoppers`
(put `BCRefabricated-*.jar` from Modrinth in the folder for `fluids`); in-game: eat an FD meal
at low hunger; a hopper/dropper into an FD cabinet; a Create honey bucket should flow as honey
(still drawn wrong).


### Client 12: pot stand-in, Create's client NPEs, Sodium, the server's log, Litematica (2026-11-04, branch `bridge-client-12`; Claude for iaxivers)

- **Cooking pot as the stand-in**: a client-only screen opened over a menu's screen (JEI's uses
  or recipes from the pot) replaced it; Escape there went back to the pot as a client-only
  screen, and the next Escape to Horizon's one-row stand-in. Now the menu's screen and its
  window are kept under it and come back (`HorizonClient.UNDER`, Rust `JAVA_UNDER`); a new menu
  screen always sends its first frame. Shown failing with the old Java (`pot-jei`: screen
  `java-screen`, window -1), passing now. `pot` opens it 20 more times (all FD's screen).
- **Create's client NPEs** fixed (the `LocalPlayer`'s `firstPersonHandsAndItems`; a synced-data
  builder for its class), and the two HUD failures behind them (`Hud`'s contextual bar, the
  connection's waypoints). Left: Flywheel (gpu). Regress `create`.
- **Sodium/Iris** (and canvas, nvidium, vulkanmod, what needs them) left out of Fabric's mods;
  players get a gray "[Java mods] Sodium isn't needed in Horizon: Horizon has its own renderer
  (left out)". Regress `sodium`.
- **Dedicated server's latest.log**: log4j starts with the bridge's console-only config, then
  `HorizonLog` sends mods' lines into Horizon's log. Regress `server-log`.
- **Litematica's UI**: MaLiLib's hotkeys never got keys (they're not KeyMappings). Every key
  while playing goes to the mods' `SDLEventHandler.handleKeyEvent` and `KeyboardHandler.keyPress`
  handlers; M opens Litematica's menu, its buttons work. Regress `litematica`. Not tried:
  placing a schematic (its overlay draws with vanilla's level renderer: gpu).
- Tools: `screen_test.sh`, `server_test.sh`, `lan_test.sh` ran stale jars after a failed
  `java.sh` (`export X=$(...) || exit 1`); they stop now. `horizon-ctl state` has `java_screen`.
- Regress (`--java-only` after a full build): pot, pot-jei, sodium, litematica, create,
  server-log (new or extended), jei, minihud, server, terminal: all pass.

### Worldgen 8: mods' new structures (2026-11-04, `bridge-worldgen-8`; Claude for iaxivers)

The queued premise didn't hold: Towns and Towers alone (vanilla jigsaw types) already
generated in Horizon's generator, through the content conversion's worldgen bundles; Dungeons
and Taverns makes the overworld the mods' anyway (it overrides `minecraft:` placed features).
The real gap: **structures of a mod's own type** (Friends & Foes' illusioner shack, citadel,
iceologer cabin) never generated in an overworld Horizon made. Now Horizon's generator lists
the structures it can't make and the mods generate such a dimension. Also: the jars give
Horizon's generator their structures, sets, features and biome tags directly (a fallback when
a content mod isn't carried over), and Java's "does a mod change this dimension" check reads
26.3's `worldgen/feature` (it read 26.2's `configured_feature`). Regress `newstructures`
(T&T), `structuretypes` (Friends & Foes + ResourcefulLib) pass; `fixer` passes; `templates`
not run here (no The Sift jar). Unit tests: `datapack` (structures-only pack), `start.rs`
(`unsupported`). Not done: Cristel Lib's placement config on Horizon's side; mod pool
element/processor types in jigsaws. Unrelated, seen: Friends & Foes' moobloom drawing NPE
(`blockModelResolver` null; entities area).
Test on the PC: `regress.py --jars <jars with friendsandfoes, ResourcefulLib, t_and_t,
cristellib, cloth-config, fabric-api> newstructures structuretypes`; or play a new world with
Friends & Foes and `/locate structure friendsandfoes:illusioner_shack`.

### gpu 12: Flywheel initialised for Create (2026-11-04, branch `bridge-gpu-12`; Claude for iaxivers)

The boss probe's three Create failures, fixed in the bridge (no Create-specific code):
- **Flywheel's model helpers** (`ModelRenderHelper.INSTANCE` null in every Create block entity
  renderer) are made at the end of `LevelExtractor.allChanged`, Flywheel's level renderer
  reload. Horizon cancels that method at its head (it draws the world), which also skipped the
  mods' injections at its end. Now, once the models are in and per client level, the mods'
  RETURN injections there run (`HorizonClient.levelRendererReady`), and those at the head of
  `Minecraft.onResourceLoadFinished` once the models load (Flywheel picks its backend there:
  off, the GPU not being OpenGL, so Create draws its parts through its renderers, which the
  bridge records). They're found from the merged class (`HorizonHooks.fireMerged`): Create's
  client mixins come from its Mixin plugin's list, which the census never read.
- `LocalPlayer.firstPersonHandsAndItems()` is made (and ticked) for the client player.
- The client player's class gets its synced data built (`new SynchedEntityData.Builder(local)`):
  Create keys its player data accessors by class and had none for `LocalPlayer`.
- `HORIZON_JAVA_DEBUG=ber`: each drawn mod block entity every 5 s (models submitted, its data:
  Create's `Speed`).
Not tested with Create itself: its jar couldn't be downloaded here (CurseForge's CDN refused;
building it from source wasn't allowed). A fixture with Create's three patterns (`kineticmod`:
a renderer throwing until an `allChanged` injection from a Mixin plugin ran, the hands and
class-keyed data read each client tick) fails on the old bridge exactly as Create did and
passes now. Regress `kinetic` (new), `ber`, `pot`, `hud`, `placed` pass; `create` (new) needs
the jar. Not done: the blank items in the creative tab (custom item renderers), Create's own
reload listeners (registered from `Minecraft.<init>` at a point the bridge doesn't fire),
fluids, contraptions.
Test: jar into `~/hz-test/jars`, `regress.py --java-only --jars ~/hz-test/jars create kinetic`.

### gpu 12 (second chat): Create with its jar, OBJ models, mods' fluids, 3D Skin Layers (2026-11-04, branch `bridge-gpu-12`; Claude for iaxivers)

Jars: CurseForge `create-fly-unofficial-port` file 9321329, Modrinth `3dskinlayers` 1.11.4,
Fabric API 0.165.2. 2 cores, llvmpipe.
Two chats took gpu 12; this one had Create's jar (CurseForge's edge CDN served it) and was
rebased on the first's commit (its Flywheel hook firing and both NPE fixes are kept; this
chat's own `HorizonFlywheel` and debug flag were dropped for them).
- **Create with its jar**: the kinetic network turns in Java (16 rpm through motor, shafts,
  cog); shafts, cogs and the motor's shaft draw and turn, in their sprites' UVs (atlas meshes
  split by sprite); the still chunk models under them are gone (states "drawn whole", Rust
  mesher). OBJ models (`horizon-assets/src/obj.rs`) fix the blank cogwheel, large cogwheel and
  water wheel items. Create's HUD hook no longer fails (empty contextual bars, a waypoint
  manager). Regress `createwhole`. Not done: the wrench item (custom renderer only),
  contraptions, other machines untested.
- **Mods' fluids**: honey and chocolate drawn with their own sprites, not as lava (Java's baked
  fluid models → mesher). Flow is still lava's (Horizon's ticks: mixins). Single player and LAN
  host only for now. Regress `fluids`.
- **3D Skin Layers**: its voxel meshes are drawn on players (146-164 quads on default skins,
  front/back checked). Horizon's flat outer layers still draw under them. Regress `skinlayers`.
`create createwhole fluids skinlayers kinetic pot terminal ber placed hud` pass.
Test: `regress.py --jars <folder with create-fly*.jar, skinlayers3d*, fabric-api*> create createwhole fluids skinlayers kinetic`;
in-game: a creative motor with shafts and cogs, a honey bucket, third person with 3D Skin Layers.

### generic maps (2026-11-04, branch `bridge-generic-maps`; Claude for iaxivers)

Ludovic's condition 2 for the map mods. Removed: JourneyMap's four mixins
(`mixin/mods/JourneyMap{MiniMap,Fullscreen,Files,Properties}Mixin`) and its fields in
`ModsPlugin`; JourneyMap's and VoxelMap's per-player state in `HorizonClientState` (field list,
waypoint reload, player-cache reset, VoxelMap image re-registering and thread stop);
JourneyMap's settings holders in `HorizonClientConfig` (and `guestNow`/`playerNow`, used only
by those mixins); Xaero's `currentSession` cleared at each join (`HorizonClient.joined`).
XaeroPlus, MiniHUD, MaLiLib, Litematica: no code tied to them was there (their hooks are
generic: GUI tail, key handlers, start points; they are only named in comments).
Licences (Modrinth): JourneyMap, Xaero's Minimap, Xaero's World Map, VoxelMap Updated: All
Rights Reserved. XaeroPlus: MIT. MiniHUD, MaLiLib, Litematica: LGPL-3.0-only.
Regress (2 cores, llvmpipe): `maps journeymap voxelmap xaeroplus worldmap-lazy minihud
litematica maps-server voxelmap-server server` pass. `maps-lan` fails (it passed before on the
same box): its two per-player checks, and both players' minimaps black/empty with a guest in.
Listed in java-bridge.md's Status. Generic fix to consider: a client per player.
Test: `regress.py --java-only --jars <map jars, trinkets, jade, appleskin> maps journeymap
voxelmap xaeroplus worldmap-lazy minihud litematica maps-server maps-lan`.

### generic-ui: no code tied to REI, JEI, Jade, AppleSkin, Architectury, Cloth Config (2026-11-04, `bridge-generic-ui`)

- **Removed**: `HorizonRecipeViewer`, `HorizonJei`, `ReiGuiGraphicsMixin`, `ReiAsyncSearchMixin`,
  `JeiConnectionMixin`, REI's fields in `HorizonClientState`, the AutoConfig/Jade/AppleSkin/REI
  holders in `HorizonClientConfig` (and `loadKept`), REI's entry in `ModsPlugin`, Architectury's
  events fired by name, `/recipes` and `/uses`, Rust's R/U special case.
- **Generic instead**: the client's join gets the server's tags and recipes as a vanilla client
  (`recipesArrive`: REI and JEI start there; REI's own display sync arrives as a payload); the
  client connection is connected (`MirrorConnectionMixin`); mods' `@WrapOperation`s of
  MouseHandler/KeyboardHandler's screen calls run (`HorizonHooks.wrapCall`: Architectury's
  input events); a mod's own GuiGraphicsExtractor records (`PlatformPlugin`); keys the inventory
  doesn't use go to the overlay with the hovered slot (R/U for any viewer); typing seen from any
  focused EditBox (`EditBoxFocusMixin`) or a mod taking the typed characters; slot presses
  reach the mods (`horizon: true`). Small Rust change in `game/java_screen.rs` (client area).
- **Works**: regress `rei jei pot-jei reiscreen hud modsettings join` pass (rei, jei and
  reiscreen rewritten for the key path, screenshot checks). Found on the way: a screen opened
  before the client joined is laid out again at the join; a packet handler's injections run
  HEAD first (REI's reload never ended when Architectury's RETURN came first).
- **Broken, listed in java-bridge.md**: per-player settings/search of these mods on LAN and
  servers (`rei-lan`, `jei-lan`, `config-rejoin` fail by design now; JEI's search box can't be
  typed in while another player's inventory is open); REI's first typed character if it is an
  inventory key.
- Licences: REI MIT, JEI MIT, Jade CC-BY-NC-SA-4.0, AppleSkin Unlicense, Architectury and Cloth
  Config LGPL-3.0.
- Test: `regress.py --jars <fabric-api, FD, REI, Architectury, Cloth Config, JEI, Jade, AppleSkin>
  rei jei pot-jei reiscreen hud modsettings join` (`--java-only` after the Rust build).
### Generic bridge: entities and the rest (2026-11-04, branch `bridge-generic-entities`; Claude for iaxivers)

Ludovic's condition 2 for Better Combat, GeckoLib, Create, Farmer's Delight and the other mods
named in `crates/horizon-jvm`/`crates/horizon-client` outside the maps, recipe viewers, Jade and
Architectury. Jars from Modrinth (26.3 Fabric). 2 cores, llvmpipe.
- **Better Combat** (ARR): its reflection (`WeaponRegistry`, `TargetFinder`, `C2S_AttackRequest`,
  `ServerNetwork`) is gone. Generic instead: (1) the click's client half
  (`HorizonClient.startAttack`: the mods' head of `Minecraft.startAttack` on the player's client;
  a cancel takes the click, the copies' hit is left out), the attack key down for a click's 2
  ticks, `continueAttack`'s head fired each client tick; (2) **the configuration phase**
  (`HorizonConfiguration`): the mods' configuration tasks run at join on a real configuration
  listener, payloads carried by codec to the client's configuration receivers and back, so Better
  Combat's weapon/config sync and Fabric's attachment sync reach the client. Result: its client
  takes the click, but its swing never sends its request: **Better Combat's attacks don't work
  now** (`bettercombat` fails; `sweep` passes only through zombies burning in daylight, a weak
  check). Not dug further: that would mean reading its closed code.
- **3D Skin Layers** (tr7zw Protective License): `getInjectedMesh` is gone. Generic: when a mod
  patches `ModelPart.render`/`compile`, each leaf part of the player model rendered alone through
  vanilla's code; a part whose quads aren't its cubes' faces is the mod's and is drawn. Regress
  `skinlayers` passes (127 quads).
- **WorldWeaver** (MIT, parts ARR): its `WorldPresetsManagerImpl` reflection is gone. Generic:
  vanilla's `DedicatedServerProperties` from no properties (WorldWeaver's own patch makes the
  default `wover:normal`), then its `WorldDimensionData.create`. Checked with Better End, Better
  Nether, BCLib, WorldWeaver: level type `wover:normal`, the End generates through Better End
  (`WoverEndBiome`).
- **TerraBlender** (LGPLv3): the `base()`/`sources()` duck typing is gone; a mod's own surface rule
  kinds are walked by their fields (rules, holders, lists, maps). `worldgen` (BOP) passes.
- **Grid World** (Zlib): the far-terrain mask reading its generator's `getConfig()` is removed
  (Java `lodMask`, JNI, `GridMask` and its test). Its far land shows whole now. `gridlod` failed
  here earlier at world creation ("minecraft:overworld is not a noise generator") before far
  terrain; not checked against java-bridge.
- **GeckoLib** (MIT), **Create**, **Farmer's Delight** (MIT): no code of theirs; comments, our
  `geckomod` fixture (GeckoLib's public API) and `kineticmod` fixture, FD as test data. Kept.
- Generic fixes on the way: `HorizonHooks.fireMerged` returns its result (one CallbackInfo per
  call), `hasMerged`; mixin field inits `new T(this)`; `HorizonMixinTable.patchedAnywhere`; the
  mirror's `Connection` drops sends (`ConnectionMixin`).
Pass: `attack keys skinlayers gecko kinetic pot placed worldgen hud server`. Fail: `bettercombat`
(above). Not run: `cargo test` (the 2-core box timed out compiling), clippy (fails first in
`horizon-world`, untouched).
Test: `regress.py --jars <folder> skinlayers bettercombat attack gecko kinetic pot placed worldgen hud server`.
### Survey 1 (2026-11-04, branch `bridge-survey-1`; Claude for iaxivers)

The 60 most-downloaded Fabric 26.3 mods on Modrinth, each alone with its dependencies (headless,
switch on): 39 work, 4 left out on purpose (Sodium family), 16 play with a failure, 1 stops the game
(Essential). None adds blocks or mobs. Results: [java-bridge-survey.md](java-bridge-survey.md);
failures queued above ("Survey 1"). Tools: `tools/java_bridge/survey/`. No code changed.
### gpu 13: Create's machines, contraptions, the wrench, the blank tab items (2026-11-04, branch `bridge-gpu-13`; Claude for iaxivers)

Jar: CurseForge `create-fly-unofficial-port` file 9321329 (`create-fly-26.3-6.0.9-unofficial.3.jar`,
from `edge.forgecdn.net/files/9321/329/…`), Fabric API 0.165.2. Only generic fixes (java-bridge.md,
"Create's machines, contraptions and items"):
- **Works now**: the press (ingots → iron sheets on a depot two blocks under it, the sheets drawn
  on the depot), the millstone (wheat → flour, fed by Horizon's hopper), the deployer (places what
  a hopper gives it), belts (made with the connector between two shafts, drawn), a mechanical
  bearing's contraption (assembles, draws, turns), the wrench and every blank item of Create's
  tab (backtanks, linked controller...), in the tab and in hand.
- Fixes: mods' own item model types (Java's resolved quads to Horizon's item renderer); OBJ
  `map_Kd 0` as a slot; items block entity renderers draw (`submitItem`, `appendItemLayers`);
  mirrored players' item cooldowns tick (the belt's second click); a click a mod block consumed
  ends there (an empty off hand took the depot's items back); mods' client particles as packets
  (the `ParticleEngine` NPE); neighbour updates mods call themselves are no-ops in the mirror (the
  bearing's assembly NPE'd); renderers get the client's block models (contraptions).
- From the testers' list: "items on stations don't show" (depot): the click must be on its top
  face (Create's rule); it shows now. "Belts don't assemble": fixed (cooldown). "ParticleEngine":
  fixed. "Fancy 3D items": fixed. "A broken block's moving parts stay": not a drawing bug, the
  hand crank *block* stays after its cogwheel breaks (Horizon's neighbour update of a mod block
  doesn't ask its `canSurvive`/`updateShape` in Java): mixins. Not reproduced here: the pale
  blue-white square on presses/mixers/blaze burners (the press draws right on llvmpipe: ask for a
  screenshot on the newest build).
- Not done: Horizon's item entities in the mirror (thrown items onto belts, millstones, crushing
  wheels, fans), players colliding with contraptions, Ponder, fluids in screens, the saw's blade.

Regress: `createmachines contraption createitems` (new) and `create createwhole fluids kinetic
skinlayers ber pot placed hud entity` pass.
Test on the PC: `regress.py --jars <folder with create-fly*.jar, fabric-api*> createmachines contraption createitems`;
in-game: the creative tab's Create page, a wrench in hand, a press over a depot (one gap), a
bearing on a motor with a block on it (right-click the bearing).
### Client 13: Cloth Config, waypoints, Litematica, Chat Heads (2026-11-04, branch `bridge-client-13`; Claude for iaxivers)

Jars (Modrinth): cloth-config 26.3.159 (LGPL-3.0), Mod Menu 21.0.0, Chat Heads 1.3.2 (MPL-2.0),
Litematica 0.29.1 + MaLiLib 0.30.2 (LGPL-3.0), Fabric API 0.165.2. 2 cores, llvmpipe.
- **Cloth Config "lags badly"**: every mouse move drew the whole screen (4-18 ms here) and
  walked `config/`; a fast PC sends hundreds a second. Moves/drags now skip the walk and
  draw at most every 40 ms; the window sends moves at most every 20 ms. Also fixed: its icons
  were blank (mods with `-` in their id were never carried: generic, every such mod), and its
  lists never scrolled (screens got delta 0; vanilla passes the frame's delta ticks).
  Touches pack's files a little (`carry.rs`, content FORMAT 15).
- **Waypoint NPE**: already fixed on java-bridge; new fixture checks fail without it and pass
  on host, LAN guest and dedicated server. Testers: pull.
- **Litematica**: menus and HUD work; the schematic overlay in the world needs mods'
  world rendering (vanilla's level renderer, which Horizon replaces): gpu work, not started.
- **Chat Heads**: chat works with it; no heads (it patches vanilla's chat, which Horizon
  replaces). Plan in java-bridge.md; not started.
- Regress (`--java-only` after a full build): cloth, join, join-lan, join-server (new or
  extended), jei, pot, hud, rei, reiscreen, modsettings, litematica, server: all pass.
Test on the PC: `regress.py --jars <jars with cloth-config, modmenu, fabric-api> cloth join-lan`,
or play: Options → Mods → Cloth Config, scroll and move the mouse fast.
### Replay 1: ReplayMod records, Horizon plays .mcpr, Flashback told (2026-11-04, branch `bridge-replay-1`; Claude for iaxivers)
Jars (Modrinth): replaymod 26.3-2.6.28 (GPL-3.0), Flashback 0.43.6 (All Rights Reserved),
Fabric API 0.165.2. 2 cores, llvmpipe.
- **ReplayMod records**: the window's player's packets go through a real vanilla client
  pipeline in Java (`HorizonPacketTap`, Rust `lan/packet_tap.rs`); its recording starts at the
  login's end, saves on world close (`replay_recordings/`). Chat and HUD lines translated (its
  legacy `.lang`).
- **Playback**: ReplayMod's viewer needs vanilla's renderer, so Horizon plays the files itself:
  `--replay FILE.mcpr` (`lan/replay.rs`), a spectator watching from behind the recorder, chat
  `pause` / `play` / `speed <x>` / `skip <s>`.
- **Flashback**: client commands work now (generic), but its recording snapshot needs the vanilla
  client's own world: told at join, not supported.
- Shared files touched (small): `lan/conn.rs` (tap), `lan/mod.rs`, `integrated.rs` (one call),
  `game.rs` (`--replay`), `horizon-port-pack/src/carry.rs` (`.lang` → JSON), the horizon lang file.
`replaymod flashback hud join modsettings failmsg pot rei keys` pass.
Test: `regress.py --jars <folder with replaymod*, Flashback*, fabric-api*> replaymod flashback`;
in-game: play a single-player world with ReplayMod, quit, then start Horizon with
`--replay <game folder>/replay_recordings/recording/<date>.mcpr`.
### Mixins 10: the double break sound, Chunky (2026-11-04, branch `bridge-mixins-10`; Claude for iaxivers)

Jars (Modrinth, 26.3 Fabric): FallingTree 26.3-25, WorldEdit 7.4.6, Biomes O' Plenty
26.3.0.0.16 (+TerraBlender, GlitchCore), Terralith 2.6.5 (+Lithostitched 2.0.6), Geophilic
3.7, Chunky 1.5.66, Fabric API 0.165.2. Not Create Fly (CurseForge only; not fetched here).
2 cores, llvmpipe.
- **Double break sound**: each tester mod alone breaks dirt and a log with one sound each.
  The double comes from a *routed* break (a mod patching the inside of breaking: vanilla's
  `destroyBlock` on the mirror), whose level event 2001 reached the breaker too: vanilla's
  `levelEvent(player, …)` leaves its source out, the mirror dropped the source. Reproduced with
  the `mixinmod` fixture (2 sounds), fixed generically (the source player is left out: 1).
  Create is the likely router in the tester's set (untested). Regress `breaksound`.
- **Chunky**: `/chunky start` now pregenerates through Horizon (load-only tickets kept,
  chunk futures completed when Horizon loaded the chunk, Horizon's region files read for
  "already generated", the server's running thread). 81 chunks in 12 s, 1,089 in 1:57; all
  saved full; a second run loads none. Regress `chunky`.
- Seen on the way, not a bug: with BOP the client hears ~40 `wet_grass` breaks while the
  first chunks generate, and sheep eating grass play `grass` breaks (vanilla's 2001 too).
Test: `regress.py --java-only --jars <folder with Chunky*, fabric-api*> breaksound chunky`.

### Merge batch 11 (2026-11-04, boss; Claude for iaxivers)

Merged: launcher-only, generic-maps, generic-ui, generic-entities, survey-1, gpu-13,
neoforge-research, 112-research, client-13, replay-1, mixins-10, voice-1, entities-10, trello-1,
crash-1. Conflicts: README's status table (batch 10's rows, launcher-only's launcher row);
generic-maps and generic-ui each emptied the per-mod lists (`HorizonClientConfig.holders`,
`HorizonClientState.FIELDS`, `ModsPlugin`, the mods mixin config: all empty now);
client-13's move/drag throttle and entities-10's `as()` without `HorizonRecipeViewer`;
replay-1's `fireMergedSkipping` on mixins' `Result`-returning `fireMerged`; entities-10's
ChunkMap `getPlayers`/`isChunkTracked` beside `readChunk` (one `level` shadow); NeoForge's start
used the game's Java download launcher-only removed: `find_java_home` now.
New: `regress.py` fails when a mod's package root is in the bridge's code (`nomodcode`, every
run; `--code-check` alone). Allowed: Mod Menu's API only.
Regress (2 cores, llvmpipe), 39 cases: pass `nomodcode server pot placed create createmachines
contraption createitems scaffold cloth join-lan join-server replaymod flashback breaksound
chunky maps journeymap voxelmap xaeroplus worldmap-lazy minihud litematica maps-server rei jei
pot-jei hud modsettings join skinlayers attack gecko kinetic worldgen`. Fail as their chats
reported: `rei-lan` (per-player REI), `maps-lan` (per-player maps), `bettercombat`.
`maps-server` failed in the long run (the player's client still loading) and passed alone.
`reiscreen` fails here also on generic-ui's own tip (REI's search bar, no list): this box, or
a flake; run it on the PC. Not run: TACZ (entities-10, no jar here).
Simple Voice Chat broke in the merge (`voice_test.sh`: 0 tones; voice-1's own tip 31/39):
replay-1's `HorizonPacketTap` gave the host's client connection a plain EmbeddedChannel, and
SVC's host client couldn't read its address. The tap's channel answers a `LocalAddress` now, as
`memoryConnection`'s does: 34/39 and 36/39; `replaymod flashback pot` pass again.
