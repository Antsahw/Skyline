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
| Its blocks drawn by code (no blockstate file) | **partial** | phase 7: a Fabric model built per position (LibMultipart parts: Wired Redstone's wires and cables, linked and powered) is drawn by the mod's own model code into Horizon's mesher; block entity renderers draw (phase 7b: Farmer's Delight's cutting board with its item, checked in a game); LibMultipart's dynamic part renderers not yet. **Known issue (parked):** in Anthony's own game the wires still can't be broken and drop nothing, though a headless test game breaks them, drops them and lights a lamp from them (2026-11-12) |
| **Screens: machines, backpacks, anything you right-click into** | **partial** | phase 4: the mod's own screen opens on the player's game and takes clicks, shift-clicks, drags and keys, with counts and tooltips; screens with vanilla's recipe book work (it shows no recipes yet); checked in a real game (Farmer's Delight's cooking pot); tooltip images and shader tints missing |
| Items with their own use behaviour (tools, food, wands) | **partial** | phase 5d: a right-click at nothing runs the item's own use (wands, guide books, backpacks); food, bows and shields are Horizon's; tools on blocks run with phase 3a's clicks |
| Sounds the mod ships | **done** | phase 5f: what the mod's server code plays reaches the players near, its own sounds by name; sounds its client code plays (ambient loops) not yet |
| Particles the mod spawns | **done** | phase 5f/5g: particles its server code sends and those its blocks' client code makes (`animateTick`: a stove's flames and smoke, checked in a game) reach the players near, vanilla's types and its own simple ones; a mod's particle types with parameters not yet |

The one that blocks most mods today is **screens**. A machine that cooks but cannot be opened is
not a working machine, and almost every content mod has at least one screen.

## Tier 2 — most mods of their kind need these

| What the mod does | Status | Where |
|---|---|---|
| Mod packets between server and client | **done** | phase 5c: both halves in one runtime, each packet handed to the other half's receiver |
| Fabric API events (block break, player tick, server start…) | **partial** | phase 5a/5b: block breaks (a mod's block broken by its own code, break events on every block), server start and level load, server and level ticks, players joining and leaving, item use, the client tick (phase 5e) |
| Items moving between mod blocks and Horizon's (hoppers, pipes) | **partial** | phase 3e: Horizon's hoppers and droppers feed and empty mods' blocks (Transfer API), checked in a real game; mods' own pipes into Horizon's containers unverified |
| Energy (Team Reborn Energy) and fluids | **done** | phase 3f: between the mods' own blocks (Tech Reborn's panel powers its furnace, a bucket fills its tank), mod blocks placed by commands and structures included; with Horizon's own blocks unverified |
| Mod mobs and their AI | **done** | phase 6d: a mod's mob lives and ticks in the runtime (its AI, physics on Horizon's blocks, fall damage; Friends & Foes' moobloom), drawn by its own renderer, hit and used by players with the mod's code (milked, killed, its loot and experience dropped), saved with the world, made by `/summon`, spawned naturally where the mod adds them to biomes and despawned far from players (all checked in a game). Mobs a mod spawns in its own way (structures, raids, custom mob categories) not yet |
| Worldgen: ores, biomes, structures | **partial** | phase 6a/6b: the features mods add to vanilla's biomes through Fabric's biome API generate in new chunks, vanilla's types in Horizon's generator and a mod's own types by the mod's code in the runtime (FD's wild crops checked in a real world); data-driven structures generate (phase 6c, Towns and Towers checked in a real world); structures of a mod's own type are made and placed by its code (phase 6h, Deeper and Darker's mazes checked in a real world; not saved with the chunk yet); mods that replace vanilla's world generation as data generate their biomes (phase 6g, Terralith checked in a real world); biomes added by code (TerraBlender: Biomes O' Plenty) not planned on the 1.20.1 side, as those mods have 26.3 releases the 26.3 bridge already runs |
| Mod dimensions | **partial** | phase 6f: a mod's data-driven dimension is a level of Horizon's server, generated by Horizon from its own noise and biomes with the mod's features placed by the mod's code (Deeper and Darker's Otherside entered with `/execute in` and checked in a game); not yet: the mod's portals and teleports, its sky and fog; its own structures generate (phase 6h); carvers of a mod's own type (the Otherside's noodle caves) skipped for now, as they are rare |
| Keybinds and HUD overlays (minimaps, tooltips, info bars) | **partial** | phase 5e: mods' key bindings reach them (pressed in Horizon, clicked in 1.20.1's client tick). Phase 5i: what mods draw into vanilla's HUD parts or `HudRenderCallback` is drawn over Horizon's HUD (AppleSkin's saturation, checked in a game); overlays about what the player looks at (Jade) and minimaps, which read the client's world, not yet |
| Config screens (Cloth Config and friends) | **done** | phase 5h: Mod Menu's entrypoints listed on Horizon's Mods page; the screen opens, draws, takes clicks and saves (AppleSkin's, checked in a game) |
| Mod commands | **done** | phase 5j: in every player's command tree, run as the player at their permission level; what the mods tell a player (answers, errors, notices) reaches their chat |
| Enchantments, status effects, attributes | **partial** | phase 5k: the mods' effects are Horizon's (icon, colour, `/effect`, saved) and act through their own code (Farmer's Delight's Comfort heals, Nourishment holds hunger: checked in a game); their food gives them and leaves its bowl. Enchantments and attributes of the mods' own: not yet checked, none of the test mods adds one |
| Mixins on vanilla behaviour (harvest tweaks, drop changes) | **partial** | phase 3a; only where the hook exists |

## Tier 3 — common enough to matter, not blocking

| What the mod does | Status | Where |
|---|---|---|
| Loot table and recipe injection into vanilla | **partial** | phase 2 carries its own; injection unverified |
| Villager trades, advancements | **partial** | phase 5l: the trades mods add to vanilla's professions and the wandering trader join their pools (Farmer's Delight's rice and cabbage for a farmer's emerald, checked in a game); one that needs a villager to answer (a map), gives an item with data, or belongs to a mod's own profession is left out. The mods' advancements with vanilla's triggers are granted (Supplementaries', Tech Reborn's seen in a game); five use their own triggers, which don't fire yet |
| Armour and tool tiers, durability, repair | **done** | phase 5m: a mod's tools mine their blocks at their tier's speed and drop what their tier allows, its weapons and tools hit for their own damage and speed, its armour gives its own defence and toughness, all with their durability and repair items (Tech Reborn's bronze pickaxe and armour, Farmer's Delight's knife: checked in a game). A tool whose speed is its own code (a drill's energy) mines as its tier |
| Mod fluids placed in the world | **partial** | phase 6e: a mod's fluid flows by its own code (its spread, delay and drop-off: Tech Reborn's oil spreads as oil, eight blocks, and stops) and is drawn with its own sprites (black oil), checked in a game. Not yet: entities in them (Horizon's physics read the template: oil swims as water), what Horizon's water and lava do when they meet them, the fluids' sprites on a remote player's game, and Tech Reborn's oil lakes in worldgen (they fail in the runtime's chunk) |
| Client-only mods with no server half | **partial** | They load (your survey of the top 150: all of them). Phase 5n: what mods add to item tooltips shows in Horizon's (a mod item's own lines, Fabric's tooltip callback: Farmer's Delight's meal effects, checked in a game); Mod Menu's settings work (5h); render and performance mods (Sodium, Iris, Lithium...) have nothing to do, Horizon draws the world. Not yet: recipe viewers over the inventory (JEI, REI, EMI), minimaps, zoom and camera mods, image tooltips (AppleSkin's food bar, shulker box previews) |
| Mods that draw in the world (holograms, outlines) | **not yet** | phase 7 |

## What "full support" would mean

A 1.20.1 mod is **fully supported** when all of this is true of it:

1. It loads and its entrypoints run. *(done for 149 of the top 150)*
2. Its content is in the game: blocks, items, recipes, tags, creative tabs, names. *(done)*
3. Its blocks behave: placing, breaking, clicking, ticking, keeping state. *(done)*
4. Its blocks look right, including ones drawn by code. *(phase 7: models built in code and block entity renderers done; dynamic part renderers next)*
5. Its screens open, draw, and respond to clicks and keys. *(phase 4: done, checked in a real game)*
6. Its messages reach the player and back. *(phase 5c: done)*
7. Its items and fluids move through other mods' and Horizon's containers. *(phase 3: items through Horizon's hoppers, energy and fluids between mods' blocks done; mods' pipes into Horizon's containers unverified)*
8. Its mobs spawn, move, fight and are fought. *(phase 6d: done: they live, move, are drawn, fought, used, saved and spawn naturally)*
9. Its worldgen appears in new chunks. *(features: 6a/6b, data-driven structures: 6c, data-driven biomes and terrain: 6g, structures of the mods' own types: 6h, done; TerraBlender biomes via the 26.3 bridge)*
10. Its client bits work: keys, HUD, config. *(keys: phase 5e, config: phase 5h, done; HUD: phase 5i, the mods' own bars and overlays done, the ones that read the client's world next)*
11. None of the above costs more than a few frames a second. *(measured per phase)*

Points 1–3, 5, 6 and 8 are done. Point 7 is mostly done (items through Horizon's hoppers, energy and fluids between mods' blocks). Points
4, 9 and 10 are the rest of the roadmap.

## How to find out where a mod stands

`tools/java_bridge/scan120.py` and `survey120.py` check whether mods load. "Works in a world" is
the tracker that phase 8 grows into — until then, the honest answer for any given mod is to put
it in `mods-java-1.20.1/`, play with it, and report what broke.

The unverified rows above are the cheapest work on this page: each is a mod in a folder and ten
minutes of play. If you want to help and don't want to write Rust, that's the job.
