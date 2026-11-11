# Notice: where this comes from, and what it is allowed to do

Skyline stands on two projects before it. This page records what it is built on and the rules
those parts come with. It is a description of the project's practice, not legal advice.

## Who made what

Skyline is the third link in a chain, and the credit has to say so:

1. **[Horizon](https://github.com/horizon-rust/horizon)**, by Ludovic Marie and its
   contributors — Minecraft: Java Edition 26.3 rewritten in Rust. Everything that plays the
   game is theirs: world, rendering, mobs, redstone, saving, multiplayer.
2. **[horizon-java-bridge](https://github.com/iaxivers/horizon-java-bridge)**, by
   [iaxivers](https://github.com/iaxivers), with Windows support by
   [martipops](https://github.com/martipops) — the 26.3 Java mod bridge: the embedded JVM, the
   mirror, the headless client, the content conversion, the launcher integration. The mirror
   idea this whole project rests on is theirs.
3. **Skyline** — this fork, which adds the 1.20.1 bridge (`horizon-jvm-remote`,
   `java/platform120/`, `game/java_mods_120.rs`) so that mods written for 1.20.1 run too.

Describing Skyline as though the 26.3 bridge were its own work would be false, whatever the
licence allows. When in doubt about a part, `git log` says who wrote it.

## This project's licence

Skyline is MIT-licensed, like both projects before it. [LICENSE](LICENSE) is Horizon's,
unchanged, and it stays: the MIT licence allows a fork to rename, modify and redistribute, **on
the condition that the copyright notice and the permission notice travel with the code**.
Renaming the project does not retire that line.

    Copyright (c) 2026 Ludovic Marie and the Horizon contributors

Changes made in this fork are the fork's contributors' own, under the same licence. Add your own
copyright line beside the original if you want one; do not replace it.

## Minecraft

Skyline does not contain Minecraft, and must not. It is a reimplementation that needs the
player's own copy of the game to run — the same posture as a launcher.

- **Nothing of Mojang's is in this repository**: no game jars, no assets, no decompiled source,
  no prepared resources. `tools/check_public_tree.py` rejects them, and the commit hook runs it.
  If it ever fires on your commit, the answer is to remove the file, not to bypass the hook.
- **The player's own install is the source.** `horizon-prepare` builds Horizon's resources from
  the player's Minecraft install on their machine. The 1.20.1 runtime downloads the 1.20.1
  client from Mojang's own endpoints (`piston-meta.mojang.com`,
  `resources.download.minecraft.net`), pinned by SHA-1 from Mojang's own manifest
  (`tools/java_bridge/fetch120.py`). Those files land outside the repository, in a cache.
- **Do not redistribute any of it.** Not the downloaded jars, not the prepared resources, not
  the converted content cache (`.horizon-cache/mod-content`), which is derived from Mojang's
  assets and the player's mod jars. Those are build outputs on one machine, not releases.
- **Minecraft is Mojang's trademark.** Skyline is not an official Minecraft product and is not
  approved by or associated with Mojang or Microsoft. Say so wherever the project is described,
  keep "Minecraft" out of the project's name and logo, and follow Mojang's brand and commercial
  usage guidelines if you ever distribute a build.

## Fabric, Mixin and the mod toolchain

The bridges run a real Fabric Loader and real Mixin. None of it is vendored here:

| Part | Licence | How it gets here |
|---|---|---|
| Fabric Loader | Apache-2.0 | downloaded, hash-pinned (`research-lock.json`) |
| Fabric API | Apache-2.0 | downloaded, hash-pinned |
| SpongePowered Mixin | MIT | downloaded with the loader |
| MixinExtras | MIT | downloaded with the loader |
| ASM, sat4j | BSD-3-Clause, EPL/LGPL | downloaded as loader libraries |
| Intermediary / Yarn mappings | CC0-1.0 | downloaded, hash-pinned |

`tools/java_bridge/check_no_fabric.py` enforces this: it rejects any `fabric-loader`,
`fabric-api`, `sponge-mixin`, `mixinextras`, `sat4j` or `asm` jar in the tree, and any Java
source declaring one of their packages. **If you ever do vendor one of these, its licence text
and notices must ship with it** — Apache-2.0 in particular requires the NOTICE file and a record
of changes. Keeping them out of the tree is the simpler path, which is why it is the one taken.

## Mods

Mod jars belong to their authors, under their own licences, and this project neither ships nor
redistributes any.

- **Mod jars are the user's.** They put them in `mods-java/` and `mods-java-1.20.1/` themselves.
  Nothing in the repository contains a mod.
- **Mods are run unmodified.** Skyline does not patch, decompile or reimplement a mod's code. It
  loads the author's jar as published and builds the scenery around it. The Mixins in this
  project target *Minecraft's* classes, not mods'.
- **The converted content is local.** Turning a mod's blocks, models and recipes into a Horizon
  content mod produces a derivative of that mod's assets. It stays in the local cache, for that
  user, from jars they already have. Do not publish converted content without the mod author's
  permission — that would be redistributing their work.
- **Test jars are not committed either.** The 1.20.1 tests take a folder of jars through
  `HORIZON_120_MODS`; they do not carry mods in the tree.

## What to check before publishing anything

1. `python3 tools/check_public_tree.py` — no Mojang files.
2. `python3 tools/java_bridge/check_no_fabric.py` — no Fabric/Mixin/ASM jars or sources.
3. No mod jars, no converted content cache, no `.local/`, no world saves.
4. [LICENSE](LICENSE) present and unchanged, this page updated if a new dependency arrived.
