# What a mod needs, and what Skyline gives it

A checklist of the things **common** Fabric mods actually use, in rough order of how many mods
need them, with what the 1.20.1 bridge does about each. Edge cases are deliberately left out:
the goal is the mod people install, not every mod ever written.

Status words mean one thing each:

- **done** — works, with a test or an in-game check behind it.
- **partial** — the common path works, a named part of it does not.
- **not yet** — not written; the phase it belongs to is named.
- **unverified** — probably works or probably doesn't, but nobody has checked. Treat these as
  unknown, not as yes.

The phases themselves are in [java-bridge-120.md](java-bridge-120.md); this page is the view
from the mod's side rather than the bridge's.

## Tier 1 — a mod is unusable without these

| What the mod does | Status | Where |
|---|---|---|
| Register blocks, items, recipes, tags, creative tabs, translations | **done** | phase 2, `content120.rs`; each mod's own creative tab, beside the 26.3 mods' |
| Its blocks place, break and drop themselves | **done** | phase 2; phase 5a: broken by their own code, contents and all, facing the placer |
| Right-click behaviour runs the mod's own code | **done** | phase 3a |
| Block entities: state kept, saved, reloaded, ticked | **done** | phase 3b |
| Block shape for aiming, where the block entity decides it | **done** | phase 3d |
| Its blocks drawn from the models in its jar | **done** | phase 2 |
| Its blocks drawn by code (no blockstate file) | **not yet** | phase 7 |
| **Screens: machines, backpacks, anything you right-click into** | **partial** | phase 4: the mod's own screen opens on the player's game and takes clicks, shift-clicks, drags and keys, with counts and tooltips; screens with vanilla's recipe book work (it shows no recipes yet); checked in a real game (Farmer's Delight's cooking pot); tooltip images and shader tints missing |
| Items with their own use behaviour (tools, food, wands) | **partial** | phase 5d: a right-click at nothing runs the item's own use (wands, guide books, backpacks); food, bows and shields are Horizon's; tools on blocks run with phase 3a's clicks |
| Sounds the mod ships | **unverified** | — |
| Particles the mod spawns | **not yet** | phase 5 |

The one that blocks most mods today is **screens**. A machine that cooks but cannot be opened is
not a working machine, and almost every content mod has at least one screen.

## Tier 2 — most mods of their kind need these

| What the mod does | Status | Where |
|---|---|---|
| Mod packets between server and client | **done** | phase 5c: both halves in one runtime, each packet handed to the other half's receiver |
| Fabric API events (block break, player tick, server start…) | **partial** | phase 5a/5b: block breaks (a mod's block broken by its own code, break events on every block), server start and level load, server and level ticks, players joining and leaving, item use, the client tick (phase 5e) |
| Items moving between mod blocks and Horizon's (hoppers, pipes) | **partial** | phase 3e: Horizon's hoppers and droppers feed and empty mods' blocks (Transfer API), checked in a real game; mods' own pipes into Horizon's containers unverified |
| Energy (Team Reborn Energy) and fluids | **not yet** | phase 3 |
| Mod mobs and their AI | **not yet** | phase 6 |
| Worldgen: ores, biomes, structures | **partial** | phase 6a: the features mods add to vanilla's biomes through Fabric's biome API generate when they are of vanilla's types (ores, patches); a mod's own feature types (FD's wild crops) are next (6b); mods' structures and biomes not yet |
| Mod dimensions | **not yet** | phase 6 |
| Keybinds and HUD overlays (minimaps, tooltips, info bars) | **partial** | phase 5e: mods' key bindings reach them (pressed in Horizon, clicked in 1.20.1's client tick); HUD overlays not yet |
| Config screens (Cloth Config and friends) | **not yet** | phase 4 |
| Mod commands | **unverified** | — |
| Enchantments, status effects, attributes | **unverified** | — |
| Mixins on vanilla behaviour (harvest tweaks, drop changes) | **partial** | phase 3a; only where the hook exists |

## Tier 3 — common enough to matter, not blocking

| What the mod does | Status | Where |
|---|---|---|
| Loot table and recipe injection into vanilla | **partial** | phase 2 carries its own; injection unverified |
| Villager trades, advancements | **unverified** | — |
| Armour and tool tiers, durability, repair | **unverified** | — |
| Mod fluids placed in the world | **not yet** | phase 6 |
| Client-only mods with no server half | **unverified** | — |
| Mods that draw in the world (holograms, outlines) | **not yet** | phase 7 |

## What "full support" would mean

A 1.20.1 mod is **fully supported** when all of this is true of it:

1. It loads and its entrypoints run. *(done for 149 of the top 150)*
2. Its content is in the game: blocks, items, recipes, tags, creative tabs, names. *(done)*
3. Its blocks behave: placing, breaking, clicking, ticking, keeping state. *(done)*
4. Its blocks look right, including ones drawn by code. *(phase 7)*
5. Its screens open, draw, and respond to clicks and keys. *(phase 4: done, checked in a real game)*
6. Its messages reach the player and back. *(phase 5c: done)*
7. Its items and fluids move through other mods' and Horizon's containers. *(phase 3: items through Horizon's hoppers done; fluids, energy and mods' pipes next)*
8. Its mobs spawn, move, fight and are fought. *(phase 6)*
9. Its worldgen appears in new chunks. *(phase 6a: vanilla-type features; 6b: the mod's own feature types)*
10. Its client bits work: keys, HUD, config. *(keys: phase 5e, done; HUD and config next)*
11. None of the above costs more than a few frames a second. *(measured per phase)*

Points 1–3, 5 and 6 are done. Point 7 is partly done (items through Horizon's hoppers). Points
4 and 8–10 are the rest of the roadmap.

## How to find out where a mod stands

`tools/java_bridge/scan120.py` and `survey120.py` check whether mods load. "Works in a world" is
the tracker that phase 8 grows into — until then, the honest answer for any given mod is to put
it in `mods-java-1.20.1/`, play with it, and report what broke.

The unverified rows above are the cheapest work on this page: each is a mod in a folder and ten
minutes of play. If you want to help and don't want to write Rust, that's the job.
