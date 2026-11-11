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
| Register blocks, items, recipes, tags, creative tabs, translations | **done** | phase 2, `content120.rs` |
| Its blocks place, break and drop themselves | **done** | phase 2 |
| Right-click behaviour runs the mod's own code | **done** | phase 3a |
| Block entities: state kept, saved, reloaded, ticked | **done** | phase 3b |
| Block shape for aiming, where the block entity decides it | **done** | phase 3d |
| Its blocks drawn from the models in its jar | **done** | phase 2 |
| Its blocks drawn by code (no blockstate file) | **not yet** | phase 7 |
| **Screens: machines, backpacks, anything you right-click into** | **partial** | phase 4: the menu opens and the mod's own screen draws a frame; not shown in the game yet, no input back |
| Items with their own use behaviour (tools, food, wands) | **unverified** | — |
| Sounds the mod ships | **unverified** | — |
| Particles the mod spawns | **not yet** | phase 5 |

The one that blocks most mods today is **screens**. A machine that cooks but cannot be opened is
not a working machine, and almost every content mod has at least one screen.

## Tier 2 — most mods of their kind need these

| What the mod does | Status | Where |
|---|---|---|
| Mod packets between server and client | **not yet** | phase 5 |
| Fabric API events (block break, player tick, server start…) | **not yet** | phase 5 |
| Items moving between mod blocks and Horizon's (hoppers, pipes) | **partial** | phase 3; Fabric Transfer API not yet |
| Energy (Team Reborn Energy) and fluids | **not yet** | phase 3 |
| Mod mobs and their AI | **not yet** | phase 6 |
| Worldgen: ores, biomes, structures | **not yet** | phase 6 |
| Mod dimensions | **not yet** | phase 6 |
| Keybinds and HUD overlays (minimaps, tooltips, info bars) | **not yet** | phase 4 + 5 |
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
5. Its screens open, draw, and respond to clicks and keys. *(phase 4: they open and draw)*
6. Its messages reach the player and back. *(phase 5)*
7. Its items and fluids move through other mods' and Horizon's containers. *(phase 3 + 5)*
8. Its mobs spawn, move, fight and are fought. *(phase 6)*
9. Its worldgen appears in new chunks. *(phase 6)*
10. Its client bits work: keys, HUD, config. *(phase 4 + 5)*
11. None of the above costs more than a few frames a second. *(measured per phase)*

Points 1–3 are done. Point 5 is half done: a screen opens and draws, but is not shown in the
game and takes no input yet. Points 4 and 6–10 are the rest of the
roadmap.

## How to find out where a mod stands

`tools/java_bridge/scan120.py` and `survey120.py` check whether mods load. "Works in a world" is
the tracker that phase 8 grows into — until then, the honest answer for any given mod is to put
it in `mods-java-1.20.1/`, play with it, and report what broke.

The unverified rows above are the cheapest work on this page: each is a mod in a folder and ten
minutes of play. If you want to help and don't want to write Rust, that's the job.
