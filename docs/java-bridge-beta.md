# Fabric mods in Horizon: beta testers' guide

Thanks for testing! This fork of [Horizon](https://github.com/horizon-rust/horizon) (Minecraft:
Java Edition 26.3 rewritten in Rust) runs **real Fabric mods for 26.3**: the mods' own Java code,
inside Horizon. This guide gets you from nothing to playing a modded world, and says what to
send back when something breaks.

> **Beta: things will break.** Play in a separate folder (below), never in a world you care
> about, and keep backups. This is an unofficial fork, not part of upstream Horizon, and not
> made by Mojang or Microsoft.

## What you need

- **Linux** (tested on Fedora and Ubuntu). Windows and macOS: not yet.
- **A copy of Minecraft: Java Edition** (a Microsoft account that owns it). Horizon contains no
  Minecraft files: it builds what it needs from your own 26.3 install.
- **A GPU with Vulkan** (any recent AMD, NVIDIA or Intel GPU).
- About **10 GB** of disk for the build, and **16 GB of RAM** for a big pack (8 GB works for a
  small one).

## 1. Install the tools (once)

Fedora:

```sh
sudo dnf install git gcc python3 java-25-openjdk-devel alsa-lib-devel systemd-devel libxkbcommon-x11-devel
```

Ubuntu / Debian:

```sh
sudo apt install git build-essential python3 openjdk-25-jdk libasound2-dev libudev-dev libxkbcommon-x11-dev
```

Then Rust, from [rustup.rs](https://rustup.rs):

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Check Java is 25: `java -version`. If it shows another version, point Horizon at 25:
`export JAVA_HOME=/usr/lib/jvm/java-25-openjdk` (the folder name can differ: `ls /usr/lib/jvm`).

## 2. Get Minecraft 26.3's files (once)

1. In the **official Minecraft launcher**, play version **26.3** once (just reach the title
   screen). That puts the game in `~/.minecraft`.
2. Download the **26.3 server jar** from [minecraft.net](https://www.minecraft.net/en-us/download/server)
   (Horizon reads the game's data from it). Save it as `~/Downloads/server.jar`.

## 3. Build the game (about 5–15 minutes the first time)

```sh
git clone -b java-bridge https://github.com/iaxivers/horizon ~/horizon
cd ~/horizon
cargo run --release -p horizon-prepare -- --prepare --server-jar ~/Downloads/server.jar
cargo build --profile fast -p horizon-client -p horizon-client-lib --features horizon-client/java-mods
```

The `prepare` step turns your Minecraft files into Horizon's (about a minute, kept in
`~/horizon/.local/`). The build makes the game, `~/horizon/target/fast/horizon-client`.

**Updating later:** `cd ~/horizon && git pull`, then run the `cargo build` line again (a minute
or two).

## 4. Get mods

Mods are **Fabric mods for Minecraft 26.3**, the same `.jar` files you'd use with Fabric
(from [Modrinth](https://modrinth.com/mods?g=categories:fabric&v=26.3) or CurseForge). Put
their required libraries too (Modrinth lists them as dependencies). Fabric API is optional: the
game brings its own.

**Or start with our test pack** (about 80 mods: Biomes O' Plenty, Farmer's Delight, REI, Jade,
Xaero's maps, JourneyMap, Naturalist, Friends & Foes, and more, with all their libraries):

```sh
python3 ~/horizon/tools/java_bridge/get_test_pack.py ~/hz-test/mods-java
```

## 5. Where mods go

Mods go in a folder called **`mods-java`** inside the **game folder**. The game folder is
**the folder you start the game from** (your terminal's current folder).

Make one game folder for testing, and put (or link) your mods in it:

```sh
mkdir -p ~/hz-play
ln -sfn ~/hz-test/mods-java ~/hz-play/mods-java     # or: mkdir ~/hz-play/mods-java and copy jars in
```

The game folder ends up holding everything for that setup: `mods-java/` (your mods),
`config/` (the mods' settings, made on the first start), `saves/` (worlds), `logs/` (the log
you'll send with bug reports).

## 6. Play

```sh
cd ~/hz-play
HORIZON_HOME=~/hz-play ~/horizon/target/fast/horizon-client --resources ~/horizon/.local/horizon-client/26.3
```

Add `--world MyWorld` to go straight into a world (made if it doesn't exist).

- **The first start with a new set of mods takes a few minutes**: the game prepares the mods'
  blocks, items and textures once. Later starts take about a minute.
- **Starting the same way from the game folder matters:** run it from somewhere else and it
  loads the mods of *that* folder (or none).
- **Check the mods loaded:** `/give @s ` then Tab should list the mods' items; the log says
  `Fabric Loader started: N mods`.

### Or with the launcher

The fork's launcher has a **Java mods** tab (add and remove jars, see which mods failed in the
last game):

```sh
cd ~/horizon/launcher && cargo build --release
./target/release/horizon-launcher import-build ~/horizon/target/fast/horizon-client --trust-unsigned
for j in ~/hz-test/mods-java/*.jar; do ./target/release/horizon-launcher java-mods add "$j"; done
HORIZON_LAUNCHER_NO_SELF_UPDATE=1 ./target/release/horizon-launcher
```

`import-build` needs one official Horizon version installed in the launcher first (it reuses its
game files); install one from the launcher if it says there's none. Always start the fork's
launcher with `HORIZON_LAUNCHER_NO_SELF_UPDATE=1`, or it replaces itself with the official one
(which has no Java mods).

## What works, what doesn't

**Works:** mod blocks, items, recipes and machines; REI, Jade, AppleSkin; Xaero's Minimap and
World Map, JourneyMap, MiniHUD; mod mobs and their AI; mod biomes, structures and terrain; mods
that change vanilla behaviour (Veinminer, Carry On, Right Click Harvest...); LAN and dedicated
servers with mods.

**Known problems** (no need to report these, but tell us if you find more about them):

- Some mod blocks are **invisible right after you place them**, until you restart the game; a
  few invisible blocks in swamps.
- **Hoppers** don't work in packs with **Lithium**.
- **Litematica**'s screens and its in-world overlay don't show.
- **VoxelMap** fails beside Xaero's maps (both use the same network message). Use one map mod.
- **Sodium and Iris don't work, and aren't needed**: they replace Minecraft's renderer, and
  Horizon has its own. Leave them out. Same for other renderer replacements.
- Some missing textures (a purple and black block) here and there.
- In menus the game renders without a frame cap, so some GPUs whine: set **Max Framerate** in
  Video Settings to your monitor's refresh rate.

## Reporting a bug

Open an issue at **[github.com/iaxivers/horizon/issues](https://github.com/iaxivers/horizon/issues)**
(one bug per issue) with:

1. **What happened, and what you expected.** Steps to make it happen again, if you know them.
2. **Your mods:** `ls ~/hz-play/mods-java` (paste the list), or "the test pack".
3. **The log:** attach `~/hz-play/logs/latest.log` from the game where it happened (it's
   replaced each start: copy it before you start again).
4. **A crash?** Attach the newest file in `~/hz-play/crash-reports/`.
5. **A screenshot** (F2 saves one in `~/hz-play/screenshots/`) for anything visual, and the
   coordinates (F3) for anything in the world.
6. Your **GPU** and **Linux** version (`lspci | grep -i vga`, `cat /etc/os-release`).

**The game froze?** Before closing it, run this in a terminal and attach both files: it shows
exactly where it's stuck.

```sh
P=$(pgrep -x horizon-client)
jcmd $P Thread.print > ~/java-threads.txt
gdb -p $P -batch -ex "thread apply all bt" > ~/rust-threads.txt 2>&1   # needs gdb
```

**Is it the mod or the bridge?** If you can, try the same mod in normal Minecraft with Fabric:
if it breaks there too, it's the mod.

## What we'd most like tested

- **Your favourite mods and packs**: which work, which don't.
- **Saving**: play, quit, come back. Do your mod blocks, chests and machines keep what was in
  them?
- **Long sessions**: an hour or two. Does it get slower, or use more and more memory?
- **Playing together**: a friend joining over LAN (Esc → Open to LAN), or a server.
- **Performance**: your FPS (F3) compared with the same mods in normal Minecraft.

## Where things are

| What | Where |
|---|---|
| The game | `~/horizon/target/fast/horizon-client` |
| Your mods | `<game folder>/mods-java/` |
| Mod settings | `<game folder>/config/` |
| Worlds | `<game folder>/saves/` |
| Log | `<game folder>/logs/latest.log` |
| Crash reports | `<game folder>/crash-reports/` |
| Cached mod content (safe to delete; rebuilt on the next start) | `<game folder>/.horizon-cache/` and `~/horizon/.horizon-cache/` |

More about how it works: [java-bridge.md](java-bridge.md).
