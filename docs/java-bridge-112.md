# Forge 1.12.2 mods in Horizon (SkyFactory 4): research and plan

Research round for "Next big steps" 6 in [java-bridge-boss.md](java-bridge-boss.md), 2026-11-04
(`bridge-112-research`). Nothing here runs yet. Read [java-bridge.md](java-bridge.md) first: this
plan reuses its shape (a mirror of Horizon's world on the Java side, Horizon stays the game).

## The answer in short

- **Yes, Cleanroom can be the bridge's 1.12 side.** It is a maintained Forge 1.12.2 fork that
  runs on Java 25 (the bridge's own runtime), with LWJGL 3.4.3 + SDL 3, built-in Mixin, and Fugue
  for old-mod quirks. 0.7.14-alpha came out 2026-10-28; it is alpha, so pin a version.
  SkyFactory 4 has a published Cleanroom migration (a few mods swapped, see below).
- **One process, one Java side per world.** JNI allows one JVM per process, so 1.12 runs in the
  same JVM as the 26.3 bridge. A world uses either Fabric 26.3 (Knot) or Forge 1.12 (Cleanroom's
  Foundation class loader), never both at once (two `net.minecraft` trees, two LWJGLs, two log4j
  set-ups: not worth fighting).
- **Same design as the Fabric bridge, with three new pieces.** (1) Block, item and NBT translation
  between 1.12 (numeric ids + 4-bit metadata) and 26.3 states. Horizon already ports vanilla's
  flattening tables (`horizon-world/src/datafix/`), so the vanilla half exists. (2) The 1.12 client
  draws with raw OpenGL, not a GPU API we can swap out the way we swap 26.3's renderpearl. So it
  gets a real hidden GL context: screens are drawn there and read back, block models are exported
  as quads, and in-world renderers are recorded. (3) Forge's event bus, not Mixins, is what mods
  use to change vanilla's behaviour. Horizon fires Forge events where it already fires Fabric ones.
- **Rough size:** bigger than the Fabric bridge's first month. The first visible milestone is
  "a 1.12 mod's block placed in Horizon", reachable in 2–3 worker rounds (phases 1–2 below).

## What SkyFactory 4 needs (measured)

SkyFactory 4.2.4's server pack (CurseForge file 3565687: 202 jars, 201 mods, 48k classes; the
client-only mods such as JourneyMap aren't in it), read with `tools/java_bridge/scan112.py`. The
script reads class files only. It resolves superclasses through every jar and counts
`@SubscribeEvent` handlers by event type. Run it on the user's full client instance for the
client-only mods:

```sh
python3 -I tools/java_bridge/scan112.py ~/path/to/SkyFactory4/mods --tsv /tmp/sf4.tsv
```

| Need | Count | Who |
|---|---:|---|
| Block classes / item classes | 1406 / 873 (107 / 97 mods) | everything |
| Tile entities (ticking) | 1106 (841) in 88 mods | NuclearCraft 151, Mekanism 84, Cyclic 74, Industrial Foregoing 67, Thermal Dynamics 47, AE2 45, Astral Sorcery 41 |
| Container GUIs / other screens | 679 / 172 in 70 / 56 mods | Mekanism 85, NuclearCraft 57, Cyclic 52, ViesCraft 48, PL2 43, AE2 42 |
| Forge event handlers | 2692 in 184 mods, 362 event types | see the event table |
| Capability use: items / fluids / energy (FE) | 340 / 222 / 155 refs, 74 / 49 / 39 mods; 57 own capabilities in 32 mods | pipes, machines, AE2, Mekanism |
| Network messages (`IMessage`) | 687 in 91 mods (`SimpleNetworkWrapper` in 84) | every machine GUI |
| Blockstate files (Forge `forge_marker` format) | 2724 (1609) in 93 mods | 73 mods use Forge's format |
| Custom `IBakedModel`s / OBJ+B3D files | 219 in 48 mods / 104 files in 13 mods | BiblioCraft 44, AE2 24 (cables), OpenComputers, Chisels & Bits, CTM |
| Tile entity renderers (TESR) | 293 in 61 mods | Mekanism 26, Cyclic 22, BiblioCraft 22 |
| Direct GL calls (`GL11`) / `GlStateManager` / `Tessellator` | 385 / 1446 / 366 refs | Astral Sorcery 57, BiblioCraft 43; GL shaders in 5 mods |
| Mobs / other entities | 94 / 127 classes | Twilight Forest 56 mobs |
| Dimensions / chunk generators / world types | 10 / 10 / 5 | Topography, Sky Grid, Lost Cities, Twilight Forest, Compact Machines, Hunting Dimension, Mystcraft |
| `IWorldGenerator` (ore and plant gen) | 32 in 20 mods | Tinkers, NuclearCraft, AE2, Mekanism, Sky Orchards |
| Coremods (ASM transformers) / Mixin configs | 15 mods (57 transformers) / 2 mods | BnBGamingCore 25 (Topography's hooks), Apotheosis 8, MalisisCore 8, Aroma1997Core 6; Surge and FoamFix go on Cleanroom |
| Custom `IRecipe` classes; CraftTweaker scripts | 71 in 26 mods; 136 `.zs` files | ContentTweaker, RecipeStages, Extended Crafting |

**The world SF4 makes.** `server.properties`: `level-type=DEFAULT`,
`generator-settings={"Topography-Preset":"Sky Factory 4"}`. The default preset
(`config/topography/SF4/dirt_tree.txt`) is a void world with a spawn structure
(`structures/*.nbt`, 1.12 structure templates), a skybox renderer and fog colours. Other presets:
cages, oceans of Thermal fluids, Lost Cities, Hunting Dimension, the Nether/End as void. So the
overworld is almost free (void + one template). The hard dimensions are Twilight Forest, Compact
Machines' rooms, Mystcraft ages and Lost Cities.

**Progression that must work:** Sky Orchards' resource trees (saplings: random ticks of mod
blocks), Prestige + GameStages + RecipeStages/ItemStages/MobStages/DimensionStages (per-player
gating: crafting must ask Java), advancements (Prestige points come from them; Triumph adds
more), Forgiving Void (fall into the void → back on top), CraftTweaker's recipe changes, and
InControl's spawn rules.

### The Forge event table (top of 362 types, handlers / mods)

| Event | Handlers | Where Horizon fires it (the Fabric bridge's hook) |
|---|---:|---|
| `RegistryEvent.Register`, `ModelRegistryEvent`, `ModelBakeEvent`, `TextureStitchEvent.Pre`, `ColorHandlerEvent` | 374/112, 119/80, 50/34, 39/33, 29 | Java's own boot: free |
| `ConfigChangedEvent`, `ClientTickEvent`, `RenderTickEvent` | 78, 87/51, 23 | Java's client: free |
| `PlayerLoggedIn/OutEvent`, `PlayerChangedDimensionEvent`, `PlayerEvent.Clone` | 65+23, 24, 16 | join/leave, `server_travel`, respawn |
| `PlayerInteractEvent.RightClickBlock` / `LeftClickBlock` / `EntityInteract` | 54/32, 27/18, 22/14 | `use_hand` (routed use), dig start, `java_entity_interact` |
| `TickEvent.PlayerTickEvent`, `LivingUpdateEvent` | 48/36, 43/22 | each mirrored player each tick (Forgiving Void, Iron Jetpacks, Wall-Jump) |
| `AttachCapabilitiesEvent` | 46/30 | Java side; also on copies of Horizon's chests and players |
| `EntityJoinWorldEvent`, `LivingSpawnEvent` | 45/33 | Horizon's spawns ask (InControl) |
| `LivingHurtEvent`, `LivingAttackEvent`, `LivingDeathEvent`, `LivingDropsEvent` | 43, 19, 31, 27 | combat on mob copies (`HorizonCombat` pattern) |
| `BlockEvent.BreakEvent`, `HarvestDropsEvent`, `PlayerEvent.BreakSpeed` | 32, 30, 18 | `java_break` (routed break), mining speed |
| `RenderWorldLastEvent`, `RenderGameOverlayEvent.Post`, `DrawBlockHighlightEvent` | 39, 28, 23 | the client's frame (see Drawing) |
| `ItemTooltipEvent`, `GuiOpenEvent`, `KeyInputEvent`, `MouseEvent` | 46, 23, 24, 17 | Horizon's tooltips ask; keys forwarded |
| `WorldEvent.Load/Unload/Save`, `ServerTickEvent`, `WorldTickEvent` | 38/47/16, 37, 22 | the mirror's levels and tick |
| `LootTableLoadEvent`, `ItemCraftedEvent`, `EntityItemPickupEvent` | 18, 16, 14 | loot, crafting result, pickup |

## Cleanroom in the bridge's JVM

What it is: Cleanroom = CleanroomLoader (FML's continuation) + patched Minecraft + Forge, with
classes loaded by **Foundation** (a LaunchWrapper replacement: `top.outlands.foundation.boot
.Foundation.main` with `--tweakClass net.minecraftforge.fml.common.launcher.FMLTweaker` or
`FMLServerTweaker`), LWJGL 3.4.3 with **LWJGLY** (the LWJGL 2 API on LWJGL 3) and SDL 3 windows.
It can run as a dedicated server too. Licence: Forge's (LGPL-2.1). It needs Mojang's 1.12.2
jar, fetched like 26.3's (after sign-in, pinned in `research-lock.json`, never committed).

How it would boot here:

1. `horizon-jvm` (already starts a Java 25 VM) puts Cleanroom's libraries on a class path of
   their own and calls `Foundation.main` with FML's client tweaker plus ours
   (`dev.horizonlab.bridge112.HorizonTweaker`). It must be the **client** side: Forge strips
   `@SideOnly(Side.CLIENT)` code on a server, and we need the mods' screens and models.
2. Our launch target runs `Minecraft`'s normal start-up (mods' preInit/init/postInit, texture
   stitching, model baking) with an SDL window that is **hidden** (`Window.Builder.hidden`;
   Cleanroom already makes a hidden 1×1 "shared context" window). It uses `SDL_VIDEO_DRIVER=offscreen`
   (EGL) where there is no display (the cloud sandboxes: Mesa llvmpipe). Then it never enters
   `Minecraft.run()`: Horizon's clock drives `runTick`.
3. A mirror `MinecraftServer` (an `IntegratedServer` in shape, like the 26.3 mirror) whose
   `WorldServer`s read Horizon's world, ticked from Horizon's server tick. One 1.12 local
   connection (`NetworkManager.provideLocalClient`) per player joins the headless client to
   it, so FML's handshake and the 687 mod messages work unchanged. The client's world fills
   from vanilla's own chunk packets (keep its view distance small).

Traps found already:

- **Forge decides the side by thread group**, not thread name: `getEffectiveSide()` is SERVER
  only on threads in `SidedThreadGroups.SERVER`. Horizon's JNI-attached "Server thread" is in
  the main group, so its 1.12 calls would look like client calls. So server work runs on a Java
  thread made in that group (Horizon hands it the tick and waits). The client runs on its own
  thread that owns the GL context.
- **Pin Cleanroom.** It is alpha (0.7.x); its Kirino render engine is work in progress and may
  change the client's GL path we hook.
- **SF4's Cleanroom list** (the PTFE optimisation guide's SF4 section): remove FastFurnace,
  FastWorkbench, FoamFix, JEI (→ Had Enough Items), Mercurius, Shadowfacts' Forgelin
  (→ Forgelin-Continuous), Surge, SwingThroughGrass, Toast Control. Swap Mekanism and its
  Generators for Mekanism CE. Update Flux Networks and CraftTweaker. Add Fugue and Scalar Legacy.
  The pack importer should apply this list.
- 1.12 ids: FML numbers blocks (4096) and items per world (`level.dat` `FML` registry). Keep the
  mirror's numbers in `horizon-java-112/registry.dat` for good: some mods save numeric ids in NBT.

## Mapping 1.12 to 26.3

**Height.** 1.12 is y 0..255. A Horizon world made for a 1.12 pack overrides the overworld's
dimension type with a data pack (`min_y` 0, `height` 256), so mods and Horizon agree. The Nether
and End already are 0..255. Upgraded 1.12 saves already open this way in vanilla.

**Vanilla blocks, 1.12 → 26.3:** `datafix/block_state_data.rs` (vanilla's `BlockStateData`:
`id << 4 | meta` → 1.13 state; generated from vanilla), then the later block renames of
Horizon's datafix chain up to 26.3. Properties that 1.12 computes (`getActualState`: fence and
pane connections, stair shapes, door halves, snowy grass) come from Horizon's own shape updates
once placed. Flower pots, beds, banners, skulls and note blocks carry part of their state in the
tile entity: convert with the TE (as `chunk_flatten.rs` does on upgrade).

**Vanilla blocks, 26.3 → 1.12** (what 1.12 code reads from Horizon's world): the inverse table,
built once. States with no 1.12 form: drop the new properties (`waterlogged`), else use one of a
few placeholder 1.12 blocks (`horizon112:modern_solid` / `_transparent` / `_liquid` / `_air`)
with the right material, so mods see "some solid block". Biomes: 26.3 → the nearest 1.12 biome id
(2D in 1.12: the column's biome at the surface).

**Mods' blocks:** one Horizon content-mod block per 1.12 block, named as registered
(`modid:path`; 1.12 lowercases), with only the **stored** states: the distinct
`getStateFromMeta(0..15)`, at most 16 per block. This is far smaller than 26.3's state sets
(ALA's 76k problem doesn't exist here). Properties that come from `getActualState` or
`getExtendedState` (pipe connections, CTM, AE2 cables) aren't stored: their look comes from the
1.12 client per position (see Drawing). Mods' fluids are Forge `BlockFluidClassic`/`Finite`, so
they run as foreign fluids, the same path bridge-mixins-9 built for Fabric mods' fluids.

**Items:** 1.12 = item + damage/meta + NBT.

- Vanilla: `datafix/items.rs` (`ItemStackTheFlatteningFix`, `dye:4` → `lapis_lazuli`) +
  renames, and the item-NBT fixes (enchantments, display names) in reverse for the way back.
- Mod items with subtypes (`getHasSubtypes`: Thermal's `material:128` is copper ingot) become one
  26.3 item per listed meta (`getSubItems` over all creative tabs), `modid:path.meta`. Damageable
  items become one item with `max_damage`/`damage`. A meta nobody lists keeps the base item with
  its damage.
- NBT goes whole into `custom_data`, so a stack survives the round trip. Horizon reads its own
  components from it (name, lore, enchantments) where it needs them. Item capabilities
  (`ForgeCaps`: energy, fluid in item) stay in that NBT.
- OreDictionary → item tags (`oredict:ingotcopper`), so exported recipes can name them.

**Tile entities:** mods' TEs live in Java, as `HorizonBlockEntities.LIVE` does. They are ticked
(`ITickable`) and saved as 1.12 NBT in `horizon-java-112/tile_entities/`. Horizon's own
containers (chests, furnaces, hoppers, dispensers) get 1.12 copies, as `COPIES` does, with item
conversion both ways. Forge's capabilities on them then let Thermal ducts, XNet, Integrated
Tunnels, AE2, Mekanism and SSN move items into Horizon's chests. Horizon's hoppers ask Java's
`IItemHandler` on a mod block's face (bridge-mixins-8's path, with Forge's capability in place of
Fabric's `ItemStorage`).

**Entities:** mods' entities live and tick in Java, on the mirror's world (Twilight Forest's 56
mobs; their AI reads Horizon's blocks through the mirror). Drawing: their `ModelBase` trees
(`ModelRenderer`: boxes, rotation points, angles after `setRotationAngles`) map directly onto the
part trees Horizon's `java_entities.rs` already draws. Horizon's own mobs get copies when a mod
event needs one (hurt, death, drops, interact), as `HorizonVanillaActions` does.

**Players:** one 1.12 `EntityPlayerMP` per player, inventory synced through the item mapping.
Its NBT (Baubles, GameStages, Astral Sorcery perks: `ForgeCaps`) is saved per UUID in
`horizon-java-112/players/`. Prestige's global unlocks live in its own files.

**Recipes:** CraftTweaker, ContentTweaker and the mods change the registries in Java at load.
Export the expressible crafting-table recipes (shaped/shapeless, ore-dict ingredients as tags)
and furnace recipes (meta-sensitive) to Horizon. Route the crafting result to Java for custom
`IRecipe`s (71) and whenever RecipeStages is loaded (gating per player). Machines keep their
recipes in Java.

**Worldgen:** a dimension whose `IChunkGenerator` is a mod's runs in Java. The 1.12 chunk is
written as 1.12 Anvil NBT (`AnvilChunkLoader.writeChunkToNBT`) and upgraded by Horizon's own
datafix chain (`chunk_flatten.rs` = vanilla's `ChunkPalettedStorageFix`), taught the mods'
numeric ids from the FML registry. This reuses the code that opens 1.12 saves. Population (1.12
populates at +8,+8 over 2×2 chunks, `IWorldGenerator`s too) uses the held-region pattern of
`worldgen_decorate`. SF4's default overworld is Topography's void + spawn template: place the
template (1.12 structure NBT → convert blocks) when the world is made.

## Drawing

1.12 draws with OpenGL 1.x–2.x calls (`GlStateManager`, `Tessellator`, display lists, `GL11`).
There is no backend to swap out, so the plan splits by what is drawn:

| What | How | Needs GL? |
|---|---|---|
| Block models (static) | Export baked quads per stored state from `ModelManager` (`IBakedModel.getQuads` for every side + null), sprites from the 1.12 block atlas (`TextureAtlasSprite` frames, CPU side). Becomes Horizon's quad models (like `obj.rs`). Covers vanilla JSON, Forge blockstates, OBJ/B3D and static custom models | no |
| Blocks whose look depends on neighbours or a TE (CTM, Chisel, AE2 cables, ducts) | The 1.12 client gives quads per position (`getActualState` → `getExtendedState` → `getQuads`) when the chunk changes; Horizon's mesher takes per-position overrides (new, like `hide_model_drawn_by_block_entity`) | no |
| Item icons in inventories | Draw each stack's GUI icon with `RenderItem` in the hidden context into an FBO, read it back into an icon atlas. Cache by item+meta+NBT hash (Tinkers' tools are built from NBT) | yes |
| Screens and HUD overlays | The mod's `GuiScreen` drawn for real into an FBO at the window's size, read back, sent as an image frame (the 512-slot big-image path Xaero's map uses); mouse and keys forwarded. Works for every mod; text is pixels | yes |
| TESRs, entities, held items, `RenderWorldLast` | A **GL recorder**: a Foundation transformer that redirects mods' and vanilla client classes' `GL11`/`GlStateManager` matrix, texture, colour and display-list calls and `Tessellator.draw` into a recorder during world passes. It records meshes with matrices and textures, which Horizon draws as `HorizonEntityRender` meshes. Outside world passes the calls go to real GL | records |
| Topography's sky and fog | Horizon draws them itself from the preset (textures, angle ranges) | no |

## Phases (each a worker round with a regress case)

1. **Boot (`boot112`).** Cleanroom inside the bridge's JVM: fetch and pin Cleanroom, Mojang's
   1.12.2 jar and libraries; boot hidden through postInit with a fixture mod and then the SF4
   server pack plus Cleanroom's swaps. Dump facts (blocks with stored states, items with
   subtypes, fluids, entities, biomes, dimensions, registry ids). Measure start time and
   memory. Write the platform code in `java/platform112/` against Cleanroom's dev jars (MCP names,
   reobfuscated to SRG by CleanroomGradle).
2. **First block (`placed112`).** Content conversion for 1.12: blocks, items, baked-quad models,
   sprites, the vanilla mapping both ways. A 1.12 mod's block (Snad, a Chisel block, a Thermal
   ore) placed and drawn in Horizon, saved and reloaded. Screenshot.
3. **Mirror world and machines (`machine112`).** `WorldServer` over Horizon's blocks, mod TEs
   ticking and saved, Horizon's containers as copies with Forge's item capability. Test: a
   Simple Generators furnace generator powering a Mekanism smelter fed by a Horizon hopper,
   output piped into a Horizon chest.
4. **Screens (`gui112`).** Local connections, `IGuiHandler`/`openGui`, the FBO frame read back,
   input forwarded, slot clicks through the mod's own `Container`. Test: open and use the
   smelter.
5. **SkyFactory start (`sf4`).** Topography void + spawn template, Forgiving Void, Sky Orchards'
   saplings growing (random ticks of mod blocks), GameStages/Prestige, advancements on the
   mirror shown in Horizon's screen, crafting through CraftTweaker's recipes and RecipeStages.
6. **The Forge event table.** The table above, at the Fabric bridge's hook points, with the
   routed-action rule (a cancellable event a mod subscribes to: ask first, honour the cancel).
   Movement mods (Iron Jetpacks, Wall-Jump, Elevator) change the client's motion: they need
   Horizon's client prediction to take the 1.12 client's velocity.
7. **In-world drawing.** The GL recorder (TESRs, mod entities, held items), per-position models,
   item icons.
8. **The rest.** Twilight Forest, Compact Machines, Hunting Dimension, Lost Cities, Mystcraft;
   LAN and dedicated servers; the launcher's SkyFactory import (CurseForge pack zip → Cleanroom
   swaps → mods folder).

## Routes considered and dropped

- **Translating 1.12 mods to 26.3** (remap and shim): Forge 1.12's API (metadata, rendering,
  registries, capabilities, world gen) has no 26.3 counterpart. A shim would mean rewriting
  Forge 1.12 on 26.3. That route stays for 1.20–1.21 mods only.
- **A real Cleanroom server in its own process, with Horizon as a 1.12-protocol client:** less
  bridging, but Horizon becomes only a window. It still needs the same content conversion, and
  it keeps 1.12's slow server for everything. Keep it as a fallback if the mirror's translation
  costs too much on big automated bases.
- **Running 1.12 and Fabric 26.3 mods in one world:** two `net.minecraft` trees in one VM can
  work in separate class loaders, but there is no pack that needs it.

## Sources

- Cleanroom: github.com/CleanroomMC/Cleanroom (README; `CHANGELOG.md` 0.7.14-alpha, 2026-10-28;
  `build.gradle`: Foundation 0.22.2, LWJGL 3.4.3; `FMLCommonHandler.getEffectiveSide`;
  `client/sdl/Window.java`), cleanroommc.com/wiki (end-user guide: Java 25, LWJGL 3, Fugue and
  Scalar Legacy).
- SF4 on Cleanroom: Polytetrafluoroethylene-PTFE/MC-Optimization-Guide,
  `modpack-specific/packs-1.12.2.md`; Rongmario's "Cleanroom Modpack Migration Notes" gist.
- SkyFactory 4.2.4 server files: curseforge.com/minecraft/modpacks/skyfactory-4/files/3565687.
