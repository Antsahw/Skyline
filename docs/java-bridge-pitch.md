# Java mods in Horizon: the bridge, in one page

For Horizon's maintainers. By iaxivers (Windows support by martipops). Branch `java-bridge` of
[iaxivers/horizon](https://github.com/iaxivers/horizon/tree/java-bridge). Details:
[java-bridge.md](java-bridge.md).

## What it is

An opt-in way to play **real Fabric 26.3 mods in Horizon**, unchanged jars, the ones players
already download. Horizon stays the game: world, ticks, mobs, redstone, physics, saving,
networking and all rendering stay Rust. A Java VM inside Horizon's process runs Fabric Loader,
the mods and their mixins against the real 26.3 classes from the player's own install, and only
where a mod changes something. With the switch off, no Java starts and nothing changes.

Why a bridge and not ports: the mods people play are huge (Create, Mekanism, Cobblemon) and many
of the biggest are All Rights Reserved (Xaero's Minimap and World Map, JourneyMap, Simple Voice
Chat, Biomes O' Plenty, Waystones, Collective, Balm...), so they can't legally be decompiled,
translated or rewritten. The bridge copies nothing: it runs the jar the player has, as Fabric does.

## How it fits Horizon

- **Off by default.** Cargo feature `java-mods`, and per instance a launcher switch "Fabric 26.3
  mods (beta)". Off: the plain build, no Java, no downloads.
- **Owners only.** Started from Horizon's launcher after Microsoft sign-in. No offline path,
  nothing downloaded before sign-in. Mojang's files come from the player's own install.
- **Generic, no mod-specific code.** Nothing in the bridge targets one mod's internals.
  `regress.py` fails if any mod's package name appears in the bridge's code.
- **Isolated.** Its own crates and files (`horizon-jvm`, `game/java_*.rs`, its Java platform);
  Horizon's files get small hooks into it, listed in [java-bridge-upstream.md](java-bridge-upstream.md).
- **Real HotSpot.** C2 and its garbage collectors: mods behave as on Fabric (mixins, reflection,
  class loading). No custom runtime to maintain.
- **Tested.** 100+ headless regress cases with screenshots (`tools/java_bridge/regress.py`),
  including LAN, dedicated server and voice chat between two clients.

## What works today

- **Survey of the 60 most-downloaded Fabric 26.3 mods** ([java-bridge-survey.md](java-bridge-survey.md)):
  39 work, 4 renderer replacements are left out on purpose (Sodium, Iris: Horizon has its own),
  16 play with one feature missing, 1 stops the game.
- **Content mods:** Farmer's Delight, Biomes O' Plenty, Terralith, The Sift, mod mobs (Naturalist,
  Friends & Foes, GeckoLib), machines and their screens, JEI and REI, Xaero's maps, JourneyMap,
  Jade, ReplayMod, Flashback, Chunky, Simple Voice Chat. Create in progress (kinetics turn and draw).
- **An 81-jar pack** (166 mods) plays on a real PC. LAN and dedicated servers, Linux and Windows.
- Daily testers on a Trello board; their reports are fixed as generic gaps, which helps every mod.

## Honest costs

Mods run in Java, so they cost what they cost on Fabric: an 81-jar pack uses ~4 GB in total and
starts in ~3 minutes on a 2-core cloud box ([java-bridge-perf.md](java-bridge-perf.md)); a real PC
does much better. Known fixes are queued (heap headroom, fewer cached converters, fewer chunk
copies, the mods' HUD sent as pixels). Without mods: Horizon exactly as it is.

## Maintenance and next steps

I maintain it: daily syncs with main, bridge failures never block your merges, CODEOWNERS on its
paths so its PRs come to me. Upstream as a series of small PRs (hooks, crates, gameplay files,
launcher, CI with Java 25 and a few headless cases, docs), each under ~500 lines where possible.
Next: NeoForge 26.3 (boots in the bridge with a test mod) and Forge 1.12 on Cleanroom (SkyFactory
4's 217 mods boot inside the bridge; not playable in Horizon yet).

Questions for you: is providing Mod Menu's public API when Mod Menu isn't installed OK (as the
bridge provides Fabric API)? Is Java for mods fine as an opt-in feature next to "everything
players run is Rust"? Are LGPL loaders (NeoForge, Cleanroom: run as jars, never read) welcome?
