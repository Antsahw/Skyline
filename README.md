<h1 align="center">Horizon Java Bridge</h1>

<p align="center"><b>Real Fabric mods, running inside <a href="https://github.com/horizon-rust/horizon">Horizon</a>, the Rust rewrite of Minecraft: Java Edition 26.3.</b></p>

<p align="center">
  <img src="https://img.shields.io/badge/Fabric-26.3-DBD0B4" alt="Fabric 26.3">
  <img src="https://img.shields.io/badge/status-beta-F0795A" alt="Beta">
  <img src="https://img.shields.io/badge/platform-Linux%20%7C%20Windows-2A2470" alt="Linux and Windows">
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-3B2F7E" alt="MIT license"></a>
</p>

<p align="center">
  <a href="#get-it"><b>Get it</b></a> ·
  <a href="#what-works"><b>What works</b></a> ·
  <a href="#how-it-works"><b>How it works</b></a> ·
  <a href="docs/java-bridge-beta.md"><b>Testers' guide</b></a> ·
  <a href="https://trello.com/b/2uhi4jkj"><b>Mod tracker</b></a>
</p>

> **Unofficial, opt-in, beta.** Made by [iaxivers](https://github.com/iaxivers), with Windows support by [martipops](https://github.com/martipops). Built on [Horizon](https://github.com/horizon-rust/horizon) by Ludovic, who has agreed to take it into the main repository; that work is underway. Not an official Minecraft product, nor approved by or associated with Mojang or Microsoft. No Minecraft code, assets or data are included: you need your own copy of Java Edition 26.3.

Horizon plays Minecraft in Rust. The bridge lets it play the **mods people already use**: the same Fabric 26.3 jars you'd put in a normal mods folder, unchanged. Horizon keeps running the game (world, mobs, redstone, physics, saving, multiplayer, all rendering); a Java VM inside Horizon runs only the mods' own code, where a mod changes something. Turned off, it's plain Horizon with no Java at all.

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

---

# About Horizon

This project builds on [Horizon](https://github.com/horizon-rust/horizon) by Ludovic: Minecraft: Java Edition 26.3 rewritten in Rust, checked against the Java game by hundreds of differential harnesses. Everything that isn't the Java bridge is Horizon's own work; its docs, roadmap and status live in [its repository](https://github.com/horizon-rust/horizon) and on [playhorizon.dev](https://playhorizon.dev). For plain Horizon without mods, use [its own launcher](https://playhorizon.dev).

The bridge is being brought into Horizon's main repository: [design note (#10216)](https://github.com/horizon-rust/horizon/pull/10216).

## Licence

MIT, like Horizon ([LICENSE](LICENSE)). Mods keep their own licences and are never included: you bring the jars you downloaded.
