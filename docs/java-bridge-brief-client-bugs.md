# Brief: 5 beta-tester client bugs (2026-11-06)

From the release testers (java-mods-2026-11-05-17d322a). Read-only diagnosis at f90ab51; each section is a
worker chat's starting brief. `HC` = `crates/horizon-jvm/java/platform/dev/horizonlab/platform/HorizonClient.java`.
All fixes generic (no mod-specific code). 26.3 facts checked with javap on the client jar.

## 1. Mods' key bindings missing from Controls (Zoomify, Spell Engine)

**Today:** Java collects mod keys only from Fabric's `KeyMappingRegistryImpl.MODDED_KEY_BINDINGS` (HC:2771
`modKeys`); `boundKeys` (HC:2786) sends scancodes only (-1 all, -200 wheel, -100-n mouse) on `horizon:java_keys`
(`java_server.rs:1850`); the game keeps a `Vec<i32>` (`java_screen.rs:174`); `java_bind_key` (`java_screen.rs:212`,
from `game.rs:17770`) sends `{"kind":"bind","key":scancode}`; HC:2833 `bind` matches every mod mapping on that key;
`withKeys` (HC:3067) applies them per player. Controls is built only from the static
`horizon-client-input/src/keybinds.rs:9` `MAPPINGS` (`options_ui.rs:3111`, `Act::Key(&'static str)` :1307).

**Cause:** (certain) scancodes only, Controls lists vanilla only, no rebinding. (possible, Spell Engine) mods not
registering via Fabric's helper are missed by `modKeys()`: check with `HORIZON_JAVA_DEBUG=screens` (a press logs
`bind <KEY> down: <mapping>`); add a startup log listing `modKeys()`.

**Plan:**
- Java: `modKeys()` = `KeyMapping.ALL` minus vanilla's (those `Options.<init>` makes).
- Java: `HorizonClient.keyMappings()` → JSON `[{name, category, key, default, label, categoryLabel}]`, labels
  resolved with the headless language.
- Rust: channel `horizon:java_key_mappings` beside `KEYS` in `send_java_keys` and `own_client_greet`; keep in
  `settings` like `settings.mods.java`.
- Rust keybinds.rs: `mod_keys: BTreeMap<String,String>` saved as `key_<name>:<key>` in Horizon's options.txt.
- Rust options_ui.rs: mod categories after vanilla's on `Page::KeyBinds`; `Act::JavaKey(usize)` /
  `ResetJavaKey(usize)`; conflicts across both lists.
- Send by mapping: `{"kind":"bind","mapping":name,"down"}` (keep `key` for -1 handlers); Java `bind()` sets that
  mapping in the player's `Keys` (per player, no write to shared `KeyMapping.key`). Also fixes rebinding after join.

**Test:** regress `keys` (fixture `keysmod`): `$C ui` finds the keysmod row under Key Binds; rebind G→H via
`click-at`; `$C key h tap` → `key G pressed by`. Add `keys-server`. Real: Zoomify (+YACL, Fabric Language Kotlin),
Spell Engine + deps.

## 2. Zoomify's zoom does nothing (mods' FOV not applied)

**Today:** in 26.3 FOV lives in `Camera` (`tickFov()`, private `calculateFov(float)`, `modifyFovBasedOnDeathOrFluid`,
`update(DeltaTracker)` writing `fov`/`hudFov`, public `getFov()`). The headless camera is `Natives.allocate(Camera.class)`
(HC:263-265): no constructor, `minecraft` null, `fovModifier` 0, never ticked. `CameraMixin` answers only
position/rotation. Horizon's FOV: `game/fov.rs` (`tick_fov` :83, `fov_degrees` :129) with native-mod hooks
(`native_hooks.rs:2715/2736`, `extras.rs:213-235` zoom + `zoom_turn_scale`); nothing from Java.

**Cause:** (certain) no Java→Rust FOV path; (secondary) the zoom key may not reach Zoomify (see 1);
`MouseHandler.turnPlayer` is cancelled headless, so sensitivity changes don't run.

**Plan:**
- Java: per-player `oldFovModifier`/`fovModifier` swapped onto the camera in `as()`/`inWorld`; set the camera's
  `minecraft` once; call `tickFov()` each client tick.
- Java, in the frame (HC:1794-1806 beside `sendHud`): `modded = calculateFov(1f)`, write `fov`, `modded = getFov()`;
  `base = options.fov * lerp(1, old, new)`; `scale = modded / base` (death/fluid inert headless).
- Send on change: `Natives.view(uuid, scale)` → `Action::View` → `horizon:java_view` (like `HudFrame`,
  `java_server.rs:2089`); local in own-client mode. Log once which mixins hook `Camera` FOV methods
  (`HorizonHooks.injected`).
- Rust: `self.java.view_scale` (prev/current, lerped) applied in `fov_degrees` after `native_compute_fov`, before the
  1..179 clamp, and in `zoom_magnification`; optionally scale turning while < 1.

**Test:** fixture `fovmod` (`@ModifyReturnValue` on `Camera.calculateFov`, halves FOV while its key is down); expose
final FOV in `horizon-ctl state` (`agent.rs:499`); regress `fov`. Then Zoomify: `$C key c down`, screenshot, up.
Note Zoomify's default C collides with Horizon's `saveToolbarActivator`.

## 3. Xaero's Minimap / World Map low-res

**Today:** headless window has fixed GUI scale 3 (`windowSize()` HC:681 → 1280x720 for everyone); `width/height` are
global statics (HC:678) set by any player's size input (HC:3895-3901); the game sends only the GUI-scaled size
(`java_screen.rs:514-520`). PIPs prepared at that scale (`HorizonGraphics.java:145`, `guiScale()` :371), redrawn ≤
every 100 ms. GPU images capped at 512 px with integer shrink (`readPending` :480-515, `MAX_IMAGE` :519,
`shrink` :523). Rust: 16 HUD atlas slots of 512 (`horizon-client-render/src/hud/java.rs:18-19`), nearest-neighbour.

**Cause:** World Map (certain): 1281x720 → shrink 3 → **427x240** stretched (regress `maps-server` even pins
427x240). Minimap: rendered for a 720p scale-3 window, upscaled 2–3× at 1440p/4K. Anything 513–1024 px halved.
Check with `HORIZON_JAVA_DEBUG=gpu` (`gpu: <name> WxH`).

**Plan:**
- Rust: send `{"kind":"size",width,height,scale,px_w,px_h}` (real GUI scale + framebuffer).
- Java: per-player window (by UUID) applied in `as()` (`windowSize(player)`: `guiScale`, framebuffer, window fields).
- Java: fractional box-filter shrink to ≤ MAX; higher MAX for the local connection.
- Rust: big images get their own wgpu textures (up to 2048–4096) with their own bind group; interim 4–8 slots of
  1024–2048. Deflate pixels for LAN/dedicated, lower cap remote.

**Test:** regress `maps` / `worldmap-lazy` / `xaeroplus` with `HORIZON_JAVA_DEBUG=gpu` on a 2560x1440 Xvfb with
`guiScale:6`; checks: World Map image ≥1280 wide, minimap PIP ≈ size×6; update `maps-server`'s regex; crops
before/after. Jars: Xaero's Minimap 26.6.2, World Map 1.47.2 (+ XaeroPlus 2.37.2), fabric-api.

## 4. 3D Skin Layers draws wrong (white boxes at the arms)

**Today:** mirrored players are `new GameProfile(uuid, name)` with no properties (`HorizonServer.java:1180`);
`SkinManagerMixin` always answers `DefaultPlayerSkin.get(profile)`; downloads blocked (`SkinDownloaderMixin`);
`avatar()` (`HorizonEntityRender.java:745-752`); `modParts` (:939) renders the mod's voxels textured with that
default skin (`texture()` :1883; log `mods draw N quads … (<texture>)`); drawn via `java_sprite`
(`java_entities.rs:620`). Horizon meanwhile draws the player's real skin (`skin_slots`, `skins/<sha1>`, or the
launcher's offline skin `horizon/offline_skin`, `remote.rs:5431`, which the server never knows) plus its own flat
outer layers.

**Cause:** (very likely) voxels built from and textured with a default skin over the player's real one; slim/wide
mismatch puts sleeve voxels off the arms. Tell-tale: log texture `minecraft:textures/entity/player/{wide,slim}/…`
while the tester has a custom skin. Reproducer: `HORIZON_OFFLINE_SKIN=<slim fixture PNG, bright sleeves>`
`HORIZON_OFFLINE_SKIN_MODEL=slim`. (less likely) flat layer + voxels doubled; (only if pure white) Recorder
defaults (:1814).

**Plan:**
- Java: mirrored profiles carry the `textures` property the server has (passed in `Natives` at player creation);
  `SkinManagerMixin` resolves real skins through vanilla's `SkinManager`, letting `SkinTextureDownloader` fetch skins
  only (bridge cache), texture id `skins/<sha1>` = Horizon's `skin_location` (`skins.rs:582`).
- Offline skin: the game sends its PNG + slim flag once on join (`horizon:java_own_skin`); server registers it in
  `HorizonTextures`.
- Rust: `java_sprite` resolves `skins/<sha1>` and the offline id through `skin_slots`.
- Rust: `modParts` lists the parts the mod draws; Horizon skips its own flat versions of those.

**Test:** regress `skinlayers` + `skinlayers-skin` (offline-skin env above, fixture skin made by a script); log
texture is the player's skin id; F5 front screenshot sleeves in fixture colours on slim arms. Jar:
`skinlayers3d-fabric-1.11.4-mc26.3.jar`, fabric-api.

## 5. Client-side mods work in single player, not on a multiplayer server

**Today:** mods' client halves run in the **server's** process (its headless client, `as(player)`). A game joined to
another server runs only "device mods" in its own JVM (`HorizonDeviceClient.join` :121-127 returns "" unless
`HorizonDeviceMods.mods()` non-empty; `java_device.rs:165-230`). All mod clients run locally only with
`HORIZON_JAVA_OWN_CLIENT=1` (`HorizonDeviceMods.java:43`, `ownJoin` :62; `java_device.rs:41`), off by default.

**Cause:** (most likely) the server doesn't run Xaero (vanilla/Paper/Fabric server, or a Horizon server without the
jar), so nobody runs Xaero's client for that player; tell-tale: player log lacks `every mod's client half runs here
for this player`. (possible) shared statics end Xaero's session at the next join (doc :2440). Ask testers which
server software.

**Plan:** client mods run in the player's own game by default when the server doesn't run them:
- Rust: a Horizon server with Java mods sends its mod ids at join (`horizon:java_server_mods`); none by `JOIN_AFTER`
  (40 ticks) → `device::join(..., own=true)`.
- Java: `OWN_CLIENT` per join; the own client runs exactly the mods the server lacks (generalise
  `HorizonDeviceMods.runs`); server lists them on `RUNS_OWN_CHANNEL` and stops running them for that player.

**Test:** `server_test.sh` gains `SERVER_MODS`/`PLAYER_MODS` and optional `VANILLA_SERVER=1` (`java -jar
.local/…/server.jar nogui`, offline; never committed); regress `maps-client-only` (server empty, player Xaero): player
log line + `gpu: a blit of horizon:dynamic/gpu`, minimap screenshot; then against a vanilla server.

## Plumbing already there

Keys (scancode/mouse/wheel, per-player `Keys`, own-client keys); native-mod FOV/zoom hooks in `fov.rs`/`extras.rs`;
PIP/GPU image path (16×512 slots); `modParts`; skin slots; own-client mode + device-mods filter.
