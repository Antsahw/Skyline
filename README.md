<h1 align="center">Skyline</h1>

<p align="center"><b>Real Fabric mods — 26.3 <i>and</i> 1.20.1 — running inside <a href="https://github.com/horizon-rust/horizon">Horizon</a>, the Rust rewrite of Minecraft: Java Edition 26.3.</b></p>

<p align="center">
  <img src="https://img.shields.io/badge/Fabric-26.3-DBD0B4" alt="Fabric 26.3">
  <img src="https://img.shields.io/badge/Fabric-1.20.1-8FB8A8" alt="Fabric 1.20.1">
  <img src="https://img.shields.io/badge/status-beta-F0795A" alt="Beta">
  <img src="https://img.shields.io/badge/platform-Linux%20%7C%20Windows-2A2470" alt="Linux and Windows">
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-3B2F7E" alt="MIT license"></a>
</p>

<p align="center">
  <a href="#run-it-from-source"><b>Run it</b></a> ·
  <a href="#what-works"><b>What works</b></a> ·
  <a href="#how-it-works"><b>How it works</b></a> ·
  <a href="docs/skyline-dev.md"><b>Build &amp; test</b></a> ·
  <a href="docs/skyline.md"><b>Architecture &amp; goals</b></a> ·
  <a href="NOTICE.md"><b>Credits &amp; licences</b></a>
</p>

> **Unofficial, opt-in, beta.** Skyline is a fork of [horizon-java-bridge](https://github.com/iaxivers/horizon-java-bridge) by [iaxivers](https://github.com/iaxivers) (Windows support by [martipops](https://github.com/martipops)), which is itself built on [Horizon](https://github.com/horizon-rust/horizon) by Ludovic. **The 26.3 bridge is iaxivers' work; Horizon itself is Ludovic's.** Skyline adds the 1.20.1 bridge on top. Who wrote what: [NOTICE.md](NOTICE.md). Not an official Minecraft product, nor approved by or associated with Mojang or Microsoft. No Minecraft code, assets or data are included: you need your own copy of Java Edition.

Horizon plays Minecraft in Rust. The bridges let it play the **mods people already use**, unchanged: the same Fabric jars you'd drop in a normal mods folder. Horizon keeps running the game (world, mobs, redstone, physics, saving, multiplayer, all rendering); the mods' own code runs in a Java VM, and only where a mod changes something. Turned off, it's plain Horizon with no Java at all.

Two bridges, side by side in the same game:

| | Mods for 26.3 | Mods for 1.20.1 |
|---|---|---|
| Folder | `mods-java/` | `mods-java-1.20.1/` |
| Feature | `java-mods` | `java-mods-120` |
| Runs in | Horizon's own process | a JVM of its own |
| State | beta, 39 of the top 60 mods play | early: blocks place, tick and break; no mod screens yet |

## Get it

You need a Microsoft account that owns Minecraft: Java Edition, and Minecraft 26.3 played once in the official launcher. Playing with mods works only through the launcher, after sign-in: there's no offline way, and nothing for the mods is downloaded before you sign in.

1. **Download the launcher** from the [`java-mods` release](https://github.com/iaxivers/horizon-java-bridge/releases/tag/java-mods): `horizon-launcher-linux-x86_64.tar.gz` or `horizon-launcher-windows-x64.zip`. It never updates itself into the main Horizon launcher.
2. **Start it, sign in,** install a Horizon version and let it prepare from your Minecraft 26.3.
3. **Turn on "Fabric mods"** on Home, next to **Play** (also in Play's **⋯** menu). The launcher downloads the Fabric mods build of the game (signed, checked like the game itself) and Java 25 if you have none.
4. **Add Fabric 26.3 jars** in the **Java mods** tab and press **Play**. The first start with new mods takes a few minutes.

Found a bug? Post it with a screenshot or video, your mod list and `logs/latest.log` (in your game folder). The [testers' guide](docs/java-bridge-beta.md) has every step.

<details>
<summary>Build from source instead</summary>

```sh
git clone -b java-bridge https://github.com/iaxivers/horizon-java-bridge && cd horizon-java-bridge
cargo build --profile fast -p horizon-client --features horizon-client/java-mods
cd launcher && cargo build --release    # Fedora needs libxkbcommon-x11-devel
horizon-launcher java-mods use-build ../target/fast/horizon-client
```

Then steps 2–4 above. Start a self-built launcher with `HORIZON_LAUNCHER_NO_SELF_UPDATE=1`.
</details>

## Run it from source

The launcher above is the easy path for 26.3 mods. This is the developer path, and the only way
to run **1.20.1** mods today.

**You need:** a copy of Minecraft: Java Edition you own, Java 25 (a JDK, not just a runtime),
Rust, and on Linux `libasound2-dev` and `libudev-dev`.

**1. Build Horizon's resources from your own Minecraft install.** Nothing here ships Mojang's
files, so this step reads them off your machine:

```sh
cargo run --release -p horizon-prepare -- --prepare      # writes .local/horizon-client/26.3
cargo run --release -p horizon-prepare -- sounds         # optional: sounds aren't in --prepare
```

Without the second line the game starts fine and says `no sounds in …: they were not copied
from your Minecraft install`. That's the warning telling you to run it.

**2. Fetch the 1.20.1 runtime** (skip if you only want 26.3 mods). This downloads Mojang's
1.20.1 client and the Fabric stack, hash-pinned, into `~/.cache/horizon-120` — outside the repo:

```sh
python3 -I tools/java_bridge/fetch120.py
tools/java_bridge/build120.sh                            # builds the bridge's own Java jar
```

Re-run `build120.sh` whenever anything under `crates/horizon-jvm/java/` changes.

**3. Put the mods where each bridge looks.** Both folders sit in your game folder — the
directory you run the game from, or whatever you pass to `--game-dir`:

```
mods-java/            Fabric 26.3 jars        (feature java-mods)
mods-java-1.20.1/     Fabric 1.20.1 jars      (feature java-mods-120)
```

Put each mod's **dependencies** in beside it, exactly as you would for a normal install: Fabric
API for that version, and anything the mod lists (Fabric Language Kotlin, for instance).
Libraries bundled *inside* a mod's jar are found on their own — you don't unpack anything.
An empty folder, or a missing one, simply means that bridge starts nothing.

**4. Build and play.** Both bridges can be on at once:

```sh
cargo build --profile fast -p horizon-client --features java-mods,java-mods-120
./target/fast/horizon-client --resources .local/horizon-client/26.3 --world MyWorld
```

Drop either feature if you only want one side. `--profile fast` is optimised and quick to link;
it's what to play on.

**What to expect on the first start.** Each bridge converts your jars into content Horizon can
load — blocks, items, models, recipes — once per set of jars. That takes a few minutes and is
cached in `.horizon-cache/mod-content`; later starts with the same jars reuse it. The 1.20.1
runtime then boots in the background, and the game says so:

```
1.20.1 mods: their code isn't running yet (it starts in the background)
1.20.1 mods: their code runs
```

Clicks on a 1.20.1 mod's blocks do nothing until that second line appears.

The long version of all of this — building the launcher, running both bridges' tests, and what
to do when a fix seems to do nothing — is in **[docs/skyline-dev.md](docs/skyline-dev.md)**.
Note the launcher is 26.3 only: 1.20.1 mods are run from source, as above.

**When something goes wrong.** `HORIZON_120_TRACE=1` turns on the 1.20.1 bridge's tracing: what
it routed, what the mods answered, what shape they gave a block. Delete
`.horizon-cache/mod-content` to force the conversion to run again. Report a bug with the mod
list and the full log.


## What works

Of the 60 most-downloaded Fabric 26.3 mods, **39 work**, 4 renderer replacements are left out on purpose (Horizon has its own renderer), 16 play with one feature missing and 1 stops the game ([the survey](docs/java-bridge-survey.md)). An 81-jar pack (166 mods) plays on a real PC.

| Area | Status |
|---|---|
| Loading | ✅ 81- to 107-jar packs start (up to 265 mods, 20,000+ recipes); content made once per set of jars, Java's classes cached between runs |
| Blocks, items, recipes, creative tabs | ✅ placed mod blocks show at once; a modpack's bundled resource packs are picked up |
| Machines and their screens | ✅ Farmer's Delight (pot, stove, cutting board, meals), Tom's Storage, Energized Power, CC: Tweaked, XNet, BuildCraft; Horizon's hoppers and droppers fill and empty mod blocks |
| Create | 🚧 motors, shafts, presses, mixers, saws, burners and depots draw and work; value boxes, the toolbox, goggles' overlays, the mechanical arm's points, chain conveyors; items thrown into depots, millstones, crushers and fans; lava with Create's fluids. Not yet: train stations reliably, Ponder, contraptions' collisions |
| Mods' fluids | ✅ drawn with their own textures, flow by the mod's own code |
| JEI, REI, Jade, AppleSkin, tooltips | ✅ JEI's and REI's lists, search and recipes, per player |
| Maps | ✅ Xaero's Minimap and World Map, XaeroPlus, JourneyMap, VoxelMap, MiniHUD |
| Mod mobs | ✅ Naturalist, Friends & Foes, Mutant Monsters, GeckoLib mobs; they fight Horizon's mobs and players both ways |
| Mod worldgen | ✅ Biomes O' Plenty, Terralith, Towns and Towers, Dungeons and Taverns, mods' own structure types and template overrides (The Sift); Tectonic's far terrain; skyblock and one-block worlds; mods' own dimensions (The Sift, Every Mining Dimension) |
| Mods that change vanilla behaviour (mixins) | ✅ Veinminer, Right Click Harvest, Carry On, Serene Seasons, Clumps |
| Player look | ✅ 3D Skin Layers |
| Multiplayer with mods | ✅ LAN and dedicated servers: each player's own maps, recipe viewers and mod settings |
| The launcher | ✅ the only way to play: a **Fabric 26.3 mods (beta)** switch per instance and a **Java mods** tab (add, remove, which mods failed last game); fetches Java 25 after the Microsoft sign-in |
| Renderer replacements (Sodium, Iris) | ➖ not needed: Horizon has its own renderer, so they have nothing to replace |
| Known not working yet | 🐛 Flashback's recording, Chat Heads' heads, Litematica's overlay in the world; on LAN, the host's own REI list (each player running their mods' client, `HORIZON_JAVA_OWN_CLIENT=1`, is still opt-in). TACZ's guns fire and draw, still rough. Testers track mods on [their Trello](https://trello.com/b/2uhi4jkj) |
| Windows | ✅ builds and plays on Windows 11 (Windows support by [martipops](https://github.com/martipops)); macOS untested |


## How it works

- **Horizon owns the game.** Rust runs the world, ticks, mobs, physics, saving, networking and every frame.
- **Java runs only the mods.** A JVM in Horizon's process loads Fabric Loader, the mods and their mixins against the real 26.3 classes from your own install, and handles only what a mod changes; the result goes back into Horizon's world. Mods' screens and drawings are captured and drawn by Horizon's renderer.
- **Generic, no mod-specific code.** Nothing in the bridge targets one mod's internals; a test fails if a mod's package name appears in its code. Every bug fixed is fixed for every mod.
- **Off by default.** A cargo feature plus a per-instance launcher switch. Off means no Java, no downloads, no cost.

Details for developers: **[docs/java-bridge.md](docs/java-bridge.md)** (where everything is, the tick flow, 26.3 traps, the test tools). One-page overview: [docs/java-bridge-pitch.md](docs/java-bridge-pitch.md). Performance numbers: [docs/java-bridge-perf.md](docs/java-bridge-perf.md). Coming next: NeoForge 26.3 ([plan](docs/java-bridge-neoforge.md)) and Forge 1.12 / SkyFactory 4 on Cleanroom ([plan](docs/java-bridge-112.md)).

**Tests:** `python3 tools/java_bridge/regress.py --jars <jars>` plays every scene that once worked, headless, with screenshots.

## Mods for 1.20.1

The 1.20.1 bridge is younger and more honest about it. A real 1.20.1 client, a real Fabric
Loader and the mods' own jars run in a JVM of their own; the server, level and player list they
reach through are hollow, redirected to Horizon. A mod believes it is at home in 1.20.1 while
its world is Horizon's 26.3 one, with block states, items and NBT translated both ways.

Working: mod blocks place and break, respond to right-clicks with the mod's own code, keep their
block entity state across saves, tick, drop items with their enchantments and names intact, and
take the shape their own code gives them. Not yet: the mods' screens, their networking to
players, their entities and worldgen, and drawing their own geometry — a mod block is drawn with
a stand-in model.

The detail, phase by phase, is in **[docs/java-bridge-120.md](docs/java-bridge-120.md)**; what
the fork is for and how both bridges fit together is in **[docs/skyline.md](docs/skyline.md)**.

---

# Credits

Skyline stands on two projects, and both deserve the name:

- **[Horizon](https://github.com/horizon-rust/horizon)** by Ludovic — Minecraft: Java Edition
  26.3 rewritten in Rust, checked against the Java game by hundreds of differential harnesses.
  Everything that plays the game is Horizon's own work; its docs, roadmap and status live in
  [its repository](https://github.com/horizon-rust/horizon) and on
  [playhorizon.dev](https://playhorizon.dev). For plain Horizon without mods, use
  [its own launcher](https://playhorizon.dev).
- **[horizon-java-bridge](https://github.com/iaxivers/horizon-java-bridge)** by
  [iaxivers](https://github.com/iaxivers), with Windows support by
  [martipops](https://github.com/martipops) — the 26.3 Java mod bridge, and the mirror idea the
  1.20.1 side is built on too. It is being brought into Horizon's main repository:
  [design note (#10216)](https://github.com/horizon-rust/horizon/pull/10216).

Skyline adds the 1.20.1 bridge. Who wrote which part, and the licences of everything the
bridges load: [NOTICE.md](NOTICE.md).

## Licence

MIT, like both projects before it ([LICENSE](LICENSE)) — the copyright notice stays with the
code. Mods keep their own licences and are never included: you bring the jars you downloaded.
No Minecraft code, assets or data are in this repository, and none may be added.
