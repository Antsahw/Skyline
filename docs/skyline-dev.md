# Building, running and testing Skyline

The [README](../README.md) has the short path: build the game, put jars in two folders, play.
This page is the long one — the launcher, both mod sets together, and the tests — for anyone
working *on* the bridges rather than playing with them.

**Read first:** the launcher is **26.3 only**. It has a Java mods tab, a Fabric switch and an
instance manager, and none of it knows 1.20.1 exists. The 1.20.1 bridge is run from source,
from a folder of jars. So "play with both through the launcher" is not a thing you can do
today; "play with both" means running the binary yourself, which is below.

## What you need

| | |
|---|---|
| Rust | stable, with the repo's toolchain |
| Java | **a JDK 25**, not a runtime — both bridges compile Java |
| Linux packages | `libasound2-dev libudev-dev`; the launcher also wants `libxkbcommon-x11-dev` (Fedora: `alsa-lib-devel systemd-devel libxkbcommon-x11-devel`, and `java-25-openjdk-devel`) |
| Minecraft | your own copy of Java Edition, played once in the official launcher |
| Disk | ~10 GB for a full build, plus the caches |

Point `JAVA_HOME` (or `HORIZON_JAVA_HOME`) at the JDK if it isn't your default.

## 1. Resources from your own install

Nothing in the repository contains Mojang's files, so the first step reads them off your
machine. This is also what the error `manifest.json is missing: prepare with --prepare first`
is asking for:

```sh
cargo run --release -p horizon-prepare -- --prepare      # -> .local/horizon-client/26.3
cargo run --release -p horizon-prepare -- sounds         # optional, not part of --prepare
```

## 2. The 1.20.1 runtime

Only needed for 1.20.1 mods. Downloads Mojang's 1.20.1 client and the Fabric stack, every file
pinned by hash, into `~/.cache/horizon-120` — outside the repository:

```sh
python3 -I tools/java_bridge/fetch120.py
tools/java_bridge/build120.sh
```

`build120.sh` compiles the bridge's own Java side (`crates/horizon-jvm/java/platform120/`).
**Re-run it after any change under `crates/horizon-jvm/java/`** — a Rust-only rebuild will
happily run yesterday's Java and the symptom looks like your fix doing nothing.

## 3. Build the game

```sh
cargo build --profile fast -p horizon-client --features java-mods,java-mods-120
```

Drop either feature for one bridge only. `--profile fast` is optimised and links quickly: it is
what to play on. Plain `--release` is slower to link for no gameplay gain; `--debug` is far too
slow to play.

Building both bridges needs a JDK for `horizon-jvm` (26.3, JNI) as well as the 1.20.1 side.

## 4. Mods, both versions at once

Two folders in the game folder — where you run the binary, or `--game-dir`:

```
mods-java/            Fabric 26.3 jars
mods-java-1.20.1/     Fabric 1.20.1 jars
```

Each folder needs that version's dependencies beside the mod, exactly as a normal install
would: Fabric API for *that* version, plus whatever the mod lists (Fabric Language Kotlin and
so on). Libraries bundled inside a mod's jar are found on their own. A missing or empty folder
means that bridge starts nothing — which is also how you turn one side off without rebuilding.

```sh
./target/fast/horizon-client --resources .local/horizon-client/26.3 --world MyWorld
```

The first start with a new set of jars converts them into content Horizon can load. That takes
a few minutes per side and is cached in `.horizon-cache/mod-content`, keyed by the jars; later
starts reuse it. Both bridges' content goes into **one** root, so a 26.3 mod and a 1.20.1 mod
can hold blocks in the same world.

The 1.20.1 runtime then boots in the background, and says so:

```
1.20.1 mods: their code isn't running yet (it starts in the background)
1.20.1 mods: their code runs
```

Clicking a 1.20.1 mod's block before that second line does nothing.

### The 26.3 mods without the launcher

Started by hand, a `java-mods` build plays without its 26.3 mods and says to use the launcher.
That is the 26.3 bridge's ownership check (iaxivers'): before it downloads Fabric and runs mods
on your Minecraft jar, the game asks Mojang's services whether the signed-in account owns
Minecraft: Java Edition, and only the launcher can hand it that sign-in. The 1.20.1 mods are
not affected.

For your own testing, build with `java-mods-dev` instead (everything `java-mods` has, plus a
developer's switch) and turn the switch on:

```sh
HORIZON_JAVA_MODS=1 cargo run --profile fast -p horizon-client --features java-mods-dev,java-mods-120 -- \
    --resources .local/horizon-client/26.3 --world MyWorld
```

Never hand out a `java-mods-dev` build: the check is how the fork keeps Mojang's game for its
owners, and release builds don't have the switch at all.

## 5. The launcher (26.3 only)

```sh
cd launcher && cargo build --release
horizon-launcher java-mods use-build ../target/fast/horizon-client
```

Then sign in, install a version, let it prepare from your Minecraft, turn on **Fabric mods** on
Home, and add jars in the **Java mods** tab. Start a self-built launcher with
`HORIZON_LAUNCHER_NO_SELF_UPDATE=1` so it doesn't replace itself with the release build.

Its commands are in [launcher-commands.md](launcher-commands.md); the player-facing walkthrough
is [java-bridge-beta.md](java-bridge-beta.md).

## 6. Tests

**1.20.1 — boots a real JVM with real jars.** This is the one to run while working on the
1.20.1 bridge:

```sh
HORIZON_120_MODS=/path/to/your/1.20.1/jars \
  cargo test --profile fast -p horizon-jvm-remote --test boot120
```

With no `HORIZON_120_MODS` the mod-specific cases are skipped and you are testing vanilla
behaviour only — a run that passes without your jars has not tested your mod.

**1.20.1 without Rust** — the runtime alone, hidden, writing a report of what loaded:

```sh
tools/java_bridge/run120.sh ~/jars/1.20.1 report.json
```

**26.3 — the regression run**, every scene that once worked, headless, checked by what the game
reports plus its screenshots:

```sh
python3 tools/java_bridge/regress.py --jars ~/hz-test/jars        # all
python3 tools/java_bridge/regress.py --jars ~/hz-test/jars pot    # one case
python3 tools/java_bridge/regress.py --list
```

**Horizon itself**, when you touch anything outside the bridges:

```sh
python3 tools/pr_ready.py --quick
```

**Before publishing anything**, the two guards that keep this repository legal:

```sh
python3 tools/check_public_tree.py                  # no Mojang files
python3 tools/java_bridge/check_no_fabric.py --tree # no Fabric/Mixin/ASM jars
```

## 7. When it misbehaves

- **`HORIZON_120_TRACE=1`** turns on the 1.20.1 bridge's tracing in a running game: which clicks
  it routed, what the mods answered, what shape they gave a block.
- **Delete `.horizon-cache/mod-content`** to force the content conversion to run again. A
  half-written cache from a crashed run looks like missing blocks and unknown ids.
- **Drive the game instead of playing it** when you need a repeatable check:
  `horizon-ctl --socket ctl.sock launch --world W --creative --offscreen --resources …`, then
  `state`, `give`, `interact X Y Z FACE`, `look-at X Y Z`, `looking-at`, `break X Y Z`.
  Commands: [horizon-ctl-commands.md](horizon-ctl-commands.md).
- **A fix that does nothing** is usually one of three things: the Java jar wasn't rebuilt
  (step 2), the content cache is stale, or the change is on the server side and never reaches
  the client. All three have cost a day here.

## Where to read next

[skyline.md](skyline.md) for what the fork is and how the bridges work ·
[java-bridge.md](java-bridge.md) for the 26.3 side ·
[java-bridge-120.md](java-bridge-120.md) for the 1.20.1 side ·
[../NOTICE.md](../NOTICE.md) for who wrote what and what may never be committed.
