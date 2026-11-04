# Survey: the 60 most-downloaded Fabric 26.3 mods (2026-11-04, `bridge-survey-1`)

The 60 most-downloaded mods on Modrinth with a Fabric 26.3 file (Modrinth search, `index=downloads`,
2026-11-04), each run alone with its required dependencies, Modrinth's Fabric API, and Mod Menu
when it has a settings screen. Headless (llvmpipe), the Java mods switch on (`launch.sh`:
`HORIZON_JAVA_MODS=1`), a fresh scratch folder each. Each run: join a world, a chest with diamonds
looked at (HUD/tooltip mods), survival, the inventory opened, `/modconfig <id>`, and keys or commands
of the mod's own screens (Xaero's M and Y, `/recipes minecraft:chest` for JEI, `/veinminer`, V for
voice chat). Log lines below are the ones that weren't in a Fabric-API-only run.

None of the 60 adds a block or a mob (they are client, library and performance mods), so nothing was
placed or spawned. Failures are queued in [java-bridge-tasks.md](java-bridge-tasks.md) ("Survey 1").

Rerun: `tools/java_bridge/survey/` (`fetch.py [N]`, `inspect.py plan.json jars`, `survey.py
[slug…]`, `analyze.py`; `SURVEY_DIR`, default `/tmp/hz-survey`). A 60-mod run takes ~80 min on 2 cores.

## Summary

| | Mods |
|---|---|
| Work (load clean; screen/HUD seen where it has one) | 39 |
| Left out on purpose (renderers: "isn't needed in Horizon") | 4 |
| Load and play, with a failure in the log (a feature missing) | 16 |
| Stop the game | 1 (Essential) |

Every run but Essential's joined the world and kept playing to the end.

## Works

| Mod | Seen |
|---|---|
| Fabric API | baseline |
| Cloth Config | its demo settings screen |
| FerriteCore | loads (memory only) |
| Mod Menu | its options screen (`/modconfig modmenu`) |
| YACL, Fabric Language Kotlin, Architectury, Placeholder API, GeckoLib, Forge Config API Port, Collective, Puzzles Lib, Konkrete, Balm, Melody, Bookshelf, Fzzy Config, TerraBlender, Searchables | libraries: load; their settings screens where they have one (Collective's) |
| Xaero's Minimap | the minimap on the HUD, its settings (Y) |
| Xaero's World Map | the map (M), its settings |
| Entity Texture Features | settings screen |
| Entity Culling | settings screen (culling itself: Horizon draws) |
| Veinminer | `/veinminer` answers (its version) |
| AppleSkin | hunger HUD; settings with Cloth Config (its optional dependency) |
| Not Enough Animations | settings screen |
| JEI | item list beside the inventory, `/recipes minecraft:chest` (crafting page) |
| Jade | the chest's tooltip (5× Diamond), its settings |
| MoreCulling, LambDynamicLights, Zoomify, Mouse Tweaks, Chat Heads, Presence Footsteps, Inventory Profiles Next | settings screens; IPN's buttons and profile bar on the inventory |
| Krypton, Clumps, Controlling, AmbientSounds | load clean (nothing to look at headless) |

Not checked (no easy headless trigger): Zoomify's zoom, Mouse Tweaks' drags, Chat Heads in chat,
dynamic lights, footstep and ambient sounds, Veinminer's vein breaking.

Small noise in Works runs: Entity Culling and Presence Footsteps write `config/<x>` relative to
the working folder before `config/` exists (`NoSuchFileException: config/entityculling.json`,
`config/transition.json`; `PathMonitor/ERROR … config/presencefootsteps`); they go on with
defaults.

## Left out on purpose

Sodium, Iris, Sodium Extra, Reese's Sodium Options: `Java mods: Sodium isn't needed in Horizon:
Horizon has its own renderer (left out)` (`horizon-jvm/src/renderers.rs`). `/modconfig iris` then
says "No settings screen for iris".

## Load, with failures

| Mod | What is missing | Log |
|---|---|---|
| Simple Voice Chat | its client part (settings screen, V) | `Could not execute entrypoint stage 'client' … provided by 'voicechat' … NullPointerException: Cannot invoke "…IPackRepository.voicechat$addSource(…)" because "repository" is null` (`FabricClientCompatibilityManager.addResourcePackSource`: `Minecraft.getResourcePackRepository()` is null headless); then `voicechat's settings screen failed: … "VoicechatClient.CLIENT_CONFIG" is null` |
| Continuity | its resources; settings screen half drawn (no title, no Done) | `a mod's resource reload (…RenderUtil$ReloadListener) failed: IllegalArgumentException: Invalid atlas id: mi…`, `(…CtmResourceReloader) failed: NullPointerException`, `the models couldn't be loaded`, `a screen failed to draw: IllegalStateException: Texture view does not exist, can't get it before something initializes it` |
| 3D Skin Layers | the player preview in its settings (black box) | `a screen failed to draw: NullPointerException: … "ClientAvatarState.deltaMovementOnPreviousTick()" …` at `AvatarRenderer.extractFlightData` ← `WPlayerPreview.drawEntity` |
| Entity Model Features | its models (vanilla's used) | `[EMF/WARN] default model returned for minecraft:copper_golem#main due to exception: NullPointerException: … "ModelManager.entityModels()" because "this.modelManager" is null` (every model) |
| ImmediatelyFast | its pack check | `A mod failed at RETURN of Minecraft.onResourceLoadFinished (…checkResourcePackCompatibility): NullPointerException: … "VanillaPackResources.fullResources()"` |
| Lithium | (none seen) | `a client level has no chunk source: IllegalArgumentException: The value 0 is not in the specified inclusive range of 1 to 32`; `Mixin fields: Entity.vicinityCache not made: NoSuchFieldException: vicinityCache` |
| C2ME | its lighting threads (generation went on) | `A mod failed around a call in ChunkMap's constructor (…c2me-threading-lighting$onLightExecutorInit): NullPointerException: Cannot invoke "String.equals(Object)" because "name" is null`; `…onLightExecutorInit1 … "TaskScheduler.name()" because "arg" is null` |
| Dynamic FPS | its window watch (settings need Cloth Config: says so) | `A mod failed in the client's <start at its end> (…dynamic_fps_common$onInit): NullPointerException at org.lwjgl.sdl.SDLVideo.SDL_GetWindowFlags ← WindowObserver.<init>` |
| CreativeCore | its client tick | `A mod failed in ClientTickEvents.START_CLIENT_TICK [CreativeCoreClient]: NullPointerException at SDLVideo.SDL_GetWindowPosition ← MouseHandler.resyncMousePosition ← Minecraft.resizeGui ← Options` |
| No Chat Reports | its client init (settings open with Cloth Config) | `A mod failed in the client's <start at its end> (…nochatreports$onConstructed): NullPointerException: … "ProfileKeyPairManager.prepareKeyPair()"` |
| Sound Physics Remastered | its join hook (settings open with Cloth Config) | `ClientPacketListenerMixin failed at the client's handleLogin: NullPointerException: … "ClientLevel.getGameTime()" because "clientLevel" is null` |
| FancyMenu | its client→server messages | `[FANCYMENU] Failed to send packet to server! NullPointerException: … "Queue.add(Object)" because "this.pendingActions" is null` at `Connection.send` |
| ShulkerBoxTooltip | its server messages (preview from the server) | `A mod failed in ClientPlayConnectionEvents.INIT [ClientNetworkingImpl]: IllegalStateException: Cannot register receiver while not in game!` |
| Language Reload | saving options; a screen's close | `[Options/ERROR] Failed to save options … "Window.getPreferredFullscreenVideoMode()" is null`; `a screen could not close: … "this.userPropertiesFuture" is null` (`Minecraft.userProperties`) |
| BadOptimizations | its settings (silent, with Cloth Config too) | `a player's mod layers couldn't be drawn: NullPointerException: … "this.mannequinRenderers" is null` |
| Veinminer Client | its version check (settings work) | `NullPointerException: getLaunchedVersion(...) must not be null` (`VeinminerClient.kt:53`, `Minecraft.getLaunchedVersion()`) |
| AppleSkin, Dynamic FPS, Chat Heads, Sound Physics, No Chat Reports | settings without Cloth Config | the mods' own "Cloth Config is required" (expected; AppleSkin's `Mod Menu/ERROR … broken implementation of ModMenuApi: cloth-config not loaded`) |
| Architectury, Forge Config API Port, Balm, Fzzy Config, BadOptimizations | `/modconfig` does nothing, says nothing | the factory gives no screen (null); the command is silent |

## Stops the game

| Mod | Log |
|---|---|
| Essential | exit 78 before the world: `Java mods: not started: ExecutionException: NullPointerException: Cannot invoke "Object.toString()" because the return value of "kotlinx.coroutines.ExecutorCoroutineDispatcherImpl.getExecutor()" is null`, from `Essential.<init>` → `WorldsManager.<init>` (a coroutine on Minecraft's executor) ← `PackRepository.handler$…$addEssentialResourcePack` ← `HorizonData.prepare`. Also `A mod failed in the client's <start at its end> (…postInit): … "MinecraftHook.pos…"`. One Java mod failing to start stops the game ("Your Java mods couldn't start, so the game stopped") |

## Patterns

- **Headless `Minecraft` fields still null**: `resourcePackRepository` (Voice Chat), `profileKeyPairManager`
  (No Chat Reports), `launchedVersion` (Veinminer Client), `userPropertiesFuture` (Language Reload),
  `modelManager` (EMF), the window's SDL handle (Dynamic FPS, CreativeCore), `ClientAvatarState` on a
  preview player (3D Skin Layers), the Connection's `pendingActions` (FancyMenu), the mannequin renderers
  (BadOptimizations). Most are client: one pass over `HorizonClient`'s allocated `Minecraft` filling them
  with stand-ins would clear six or seven mods.
- **Client join events before the client "is in game"**: ShulkerBoxTooltip's INIT receivers, Sound
  Physics' `handleLogin` with no client level.
- **The mirror's `ChunkMap` without its constructor's state**: C2ME's light executor.
- **A start failure stops the whole game** (Essential): the other mods would be playable without it.
