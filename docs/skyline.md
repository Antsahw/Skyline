# Skyline

**Skyline is a fork of [Horizon](https://github.com/horizon-rust/horizon) that runs real Java
mods inside it.** Horizon is Minecraft: Java Edition 26.3 rewritten in Rust; Skyline keeps all
of that and adds the bridges that let Fabric mods — written for Java, compiled against Mojang's
jars, never touched — run against Horizon's world.

This page is the map for anyone new to the fork, human or agent. It says what the fork is for,
how the two bridges work, and where to start. For the depth, read
[java-bridge.md](java-bridge.md) (26.3) and [java-bridge-120.md](java-bridge-120.md) (1.20.1).

## What it is for

1. **Run a mod nobody is maintaining any more.** A great deal of Minecraft's mod ecosystem is
   stuck on old versions, 1.20.1 above all. Those mods will never be ported. Skyline's answer is
   not to port them but to run them where they are, against a world that is no longer theirs.
2. **Run mods at all on a Rust client.** A rewrite with no mods is a tech demo. The 26.3 bridge
   is what makes Horizon a thing people could actually play.
3. **Several versions at once.** 26.3 mods and 1.20.1 mods in the same game, in the same world,
   each in a runtime that believes it is at home.

Not goals: being a mod loader of Skyline's own; changing mods to fit; asking mod authors for
anything. A mod that needs a patched jar is a mod Skyline has failed to run.

## The shape of it

Both bridges rest on the same idea, which is worth saying in one line:

> A real Minecraft jar, a real Fabric Loader and the real mod jars run in a JVM. The classes a
> mod reaches through — the server, the level, the player list — are **hollow**: allocated
> without constructors, with their world-facing methods redirected by Mixins to Horizon. The mod
> believes it is talking to Minecraft. It is talking to Rust.

The mod's own code is never modified, decompiled or reimplemented. What is written here is the
*scenery* it acts against.

Where the two differ:

| | 26.3 bridge | 1.20.1 bridge |
|---|---|---|
| Crate | `horizon-jvm` | `horizon-jvm-remote` |
| Mods go in | `mods-java/` | `mods-java-1.20.1/` |
| Cargo feature | `java-mods` | `java-mods-120` |
| JVM | in-process, JNI | a child process of its own |
| Calls cross by | JNI (`Natives.java`) | a Unix socket (`wire.rs`, `peer.rs`) |
| Cost of a call | a JNI call | ~14–32 µs round trip |
| Version translation | none needed | 1.20.1 ↔ 26.3, both ways |
| Java side | `java/platform/` (~138 files) | `java/platform120/` |
| Game side | `game/java_mods.rs`, `java_server.rs` | `game/java_mods_120.rs` |

The 1.20.1 side runs out of process for one reason: it needs *its own* Minecraft, its own Fabric
Loader and its own class graph, which cannot share a JVM with 26.3's. The socket is a deliberate
serialisation of the same call surface JNI gives the 26.3 side, so methods map one to one. A
side waiting for a reply keeps serving incoming calls (`peer.rs`), so calls nest in both
directions as they do on one JNI thread.

### What crosses, and how

- **Blocks.** Reads go to Horizon; writes come back as a list applied after the call.
  1.20.1 block states cross by name, with properties Horizon does not know dropped; a block
  newer than 1.20.1 reads as stone.
- **Block entities.** Horizon keeps a mod's block entity as `BlockEntity::Mod`: opaque NBT plus
  decoded items. Horizon never learns what it means; the mod reads it back unchanged.
- **Items.** 1.20.1 NBT ↔ 26.3 components, through vanilla's own datafixers
  (`horizon_world::datafix`, 1.20.5's `ItemStackComponentizationFix` is exactly the conversion
  needed). The downgrade direction — 26.3 items shown *to* 1.20.1 code — is still thin.
- **Shapes.** A block whose shape is its block entity's has none worth exporting, so Horizon
  asks the mod per position and keeps the answer as a synced attachment
  (`game/mod_block_shape.rs`).
- **Content.** Mod jars are converted once into Horizon content mods — blocks, items, models,
  recipes, tags — cached by jar-set hash. A mod's block becomes a native Horizon block.

### Things that cost people days

- **Constructors never run.** A hollow object's fields are null, including the ones *other mods*
  set in constructor injections. `Constructed120` replays those handlers; if a mod's state hangs
  off the level or the chunk map, that is where it has to be rebuilt.
- **A Mixin on a method that does not exist fails the whole class**, and surfaces as something
  unrelated far away. Always check the method is really on that class.
- **Bare method names bind to the wrong overload silently.** Write the full descriptor.
- **Mods merge their own overrides.** A fix on `Level` can be bypassed by a mod's override on
  `ServerLevel`. Fix it where the call actually lands.
- **Obfuscated names are not guessable.** Look them up in the mappings; do not assume from shape.
- **The 26.3 side has solved a lot of this already.** Before inventing a fix for the 1.20.1 side,
  look in `java/platform/` — ~75 mixins of prior art.

## Where to work

| You want to | Start at |
|---|---|
| Make a 1.20.1 mod work | [java-bridge-120.md](java-bridge-120.md), then `java/platform120/` |
| Make a 26.3 mod work | [java-bridge.md](java-bridge.md), then `java/platform/` |
| Change what crosses the wire | `crates/horizon-jvm-remote/src/{wire,peer,mirror120}.rs` |
| Change how mod jars become content | `crates/horizon-jvm-remote/src/content120.rs`, `crates/horizon-port-pack/` |
| Fix Horizon itself (not the bridge) | [../AGENTS.md](../AGENTS.md) and the upstream docs |

Building, running with both mod sets, and the tests: [skyline-dev.md](skyline-dev.md).

Tests for the 1.20.1 bridge are in `crates/horizon-jvm-remote/tests/boot120.rs`: they boot a real
runtime with real mod jars. Point `HORIZON_120_MODS` at a folder of jars to run them against your
own mods. `HORIZON_120_TRACE=1` turns on the bridge's tracing in a running game.

## How to judge a change here

The rules in [../AGENTS.md](../AGENTS.md) hold — vanilla is the specification, never commit
Mojang's files, keep runs away from real player data. Three more belong to the fork:

- **A fix is not done until it is seen working.** The tests boot a real JVM for a reason. For
  anything the player sees, drive the real game (`horizon-ctl`) and look. Several fixes in this
  project passed their tests and did nothing in the world.
- **Verify by removing the fix again.** If the test still passes without it, the test is not
  testing it.
- **Say what is not done.** Every status section in these docs ends with what is still missing.
  Keep that habit: a half-working feature reported as working costs the next person a day.

## What is supported, from the mod's side

[skyline-mod-support.md](skyline-mod-support.md) lists what common mods need and what each gets
today -- done, partial, not yet, or unverified -- and says what "fully supported" would mean.
Read it before promising anyone a mod works.

## Status

What works, what does not, and what is next are kept in the phase sections of
[java-bridge-120.md](java-bridge-120.md) and [java-bridge.md](java-bridge.md) rather than here,
so there is one place to update. The short version for 1.20.1: mod blocks place, respond to
clicks, keep their block entity state, tick, drop items, and have the shape their mod gives
them. They are drawn with a stand-in model, their screens do not open yet, and their own
networking does not reach players.
