#!/usr/bin/env python3
"""The Java bridge's regression run: every scene that once worked, played headless, each
checked by what the game reports (horizon-ctl), its log and its screenshots.

    python3 tools/java_bridge/regress.py --jars ~/hz-test/jars          # all cases
    python3 tools/java_bridge/regress.py --jars ~/hz-test/jars pot rei  # some
    python3 tools/java_bridge/regress.py --java-only ...                # after a Java-only change
    python3 tools/java_bridge/regress.py --list

    python3 tools/java_bridge/regress.py --pack ~/hz-test/mods-java bigpack  # a whole pack

--jars is one folder of Modrinth mod jars; each case takes the ones it needs by name and is
skipped when one is missing (the repo's fixture mods are built here). Screenshots and logs go
to --out (default /tmp/hz-regress/<case>/). Exit status 1 when a case fails. --pack is a
whole pack's mods folder (a config/ beside it is the mods' configs), for `bigpack`; its content,
made once per set of jars (minutes), is kept in --out/pack-cache.
"""
import argparse
import fnmatch
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
TOOLS = ROOT / "tools" / "java_bridge"

# Mod jars by name, as Modrinth names them (case-insensitive globs).
MODS = {
    "fd": ["farmers*delight*"],
    "toms": ["toms*storage*", "toms_storage*"],
    "jade": ["jade*"],
    "appleskin": ["appleskin*"],
    "rei": ["rei.jar", "roughlyenoughitems*", "rei-*"],
    "jei": ["jei-*fabric*", "jei.jar"],
    "architectury": ["architectury*"],
    "cloth": ["cloth-config*", "cloth_config*"],
    "bop": ["biomes*o*plenty*", "biomesoplenty*"],
    "terrablender": ["terrablender*"],
    "glitchcore": ["glitchcore*"],
    "fabric-api": ["fabric-api*"],
    "skinlayers": ["skinlayers3d*"],
    "veinminer": ["veinminer-fabric*"],
    "chunky": ["Chunky-Fabric*", "chunky-fabric*"],
    "rch": ["rightclickharvest*"],
    "jamlib": ["jamlib*"],
    "carryon": ["carryon*"],
    "seasons": ["sereneseasons*"],
    "clumps": ["clumps*"],
    "bettercombat": ["bettercombat*"],
    "playeranim": ["playeranimationlib*", "player-animation*"],
    "kotlin": ["fabric-language-kotlin*"],
    "oneblock": ["oneblock*"],
    "codxlib": ["codxlib*"],
    "gridgen": ["gridgen*"],
    "skyblock": ["skyblock-infinite*"],
    "simplyoneblock": ["simply_oneblock*", "simply-oneblock*"],
    "buildcraft": ["bcrefabricated*", "buildcraft*"],
    "fluidtank": ["largefluidtank*"],
    "scala": ["scalable-cats-force*"],
    "geckolib": ["geckolib*"],
    "xaero": ["xaerominimap*", "xaeros-minimap*", "xaeros_minimap*"],
    "trinkets": ["trinkets*"],
    "journeymap": ["journeymap*"],
    "voxelmap": ["voxelmap*"],
    "diorama": ["diorama*"],
    "xaeroworldmap": ["xaeroworldmap*", "xaeros_world_map*", "xaeros-world-map*"],
    "xaeroplus": ["xaeroplus*"],
    "malilib": ["malilib*"],
    "minihud": ["minihud*"],
    "dungeons": ["dungeons-and-taverns*", "dungeons_and_taverns*"],
    "xnet": ["xnet-*"],
    "mcjtylib": ["mcjtylib*"],
    "rftoolsbase": ["rftoolsbase*"],
    "fcap": ["forgeconfigapiport*"],
    "tacz": ["tacz-refabricated*", "tacz*refabricated*"],
    "cc": ["cc-tweaked*"],
    "skills": ["puffish_skills*"],
    "legendarytooltips": ["legendarytooltips*"],
    "iceberg": ["iceberg*"],
    "prism-lib": ["prism-*"],
    "ali": ["advancedlootinfo*"],
    "aci": ["advancedcoreinfo*"],
    "compressedblocks": ["compressedblocks*"],
    "sift": ["mielon*the*sift*", "mielons-the-sift*"],
    "sodium": ["sodium-fabric*"],
    "iris": ["iris-fabric*"],
    "create": ["create-fly*"],
    "asyncparticles": ["asyncparticles*"],
    "immersivehotbar": ["immersive-hotbar*"],
    "yacl": ["yet_another_config_lib*", "yet-another-config-lib*"],
    "emf": ["entity_model_features*", "entity-model-features*"],
    "etf": ["entity_texture_features*", "entity-texture-features*"],
    "shulkerboxtooltip": ["shulkerboxtooltip*"],
    "fancymenu": ["fancymenu*"],
    "konkrete": ["konkrete*"],
    "melody": ["melody*"],
    "continuity": ["continuity*"],
    "mezzconfig": ["mezz_config*"],
    "litematica": ["litematica*"],
    "replaymod": ["replaymod*"],
    "flashback": ["flashback*"],

    "tt": ["t_and_t*", "towns*and*towers*"],
    "cristellib": ["cristellib*", "cristel*lib*"],
    "faf": ["friendsandfoes*"],
    "emd": ["everyminingdimension*"],
    "resourcefullib": ["resourcefullib*"],

    "create": ["create-fly*"],
}


def check_file(name, pattern, what):
    """The scratch file `name` (a step wrote it) matches `pattern`."""
    return ("file", name, pattern, what)


def check_log(pattern, what):
    """The game's log has a line matching `pattern`."""
    return ("log", None, pattern, what)


def check_json(name, path, expect, what):
    """JSON file `name`'s value at `path` (keys and indexes) equals `expect`."""
    return ("json", name, (path, expect), what)


def screen_has(slot, item):
    """The open container's slot holds `item` (horizon-ctl screen)."""
    return rf'"item":"{re.escape(item)}"[^}}]*"slot":{slot}\b|"slot":{slot}[^}}]*"item":"{re.escape(item)}"'


CASES = {
    "pot": dict(
        about="Farmer's Delight: the cooking pot's screen opens; beef moved into it",
        mods=["fd"], scene="pot",
        steps=['$C click-at 141 187', '$C click-at 163 62', '$C state > $S/state.json; $C screen > $S/screen.json',
               # Closed and opened again 20 times, quickly and slowly (a tester's pot sometimes
               # came back as Horizon's one-row stand-in): FD's own screen each time.
               'ok=0; for i in $(seq 20); do $C key escape tap >/dev/null; sleep 0.$((i % 4 * 3 + 1)); '
               '$C mouse right click >/dev/null; sleep 1.5; '
               '$C state | python3 -c "import json,sys; s=json.load(sys.stdin); '
               'print(s[\'screen\'], s.get(\'java_screen\'))" >> $S/reopen.log; '
               'tail -1 $S/reopen.log | grep -q "^container [0-9]" && ok=$((ok+1)); done; '
               'echo "own screen $ok/20" > $S/reopen.txt'],
        checks=[check_json("state.json", ["screen"], "container", "the pot's screen is open"),
                check_json("state.json", ["looking_at", "block", "block"], "farmersdelight:cooking_pot", "looking at the pot"),
                check_file("screen.json", screen_has(300, "minecraft:beef"), "beef is in the pot's first slot"),
                check_file("reopen.txt", r"own screen 20/20", "closed and opened 20 times: FD's own screen each time")],
    ),
    "terminal": dict(
        about="Tom's Storage: the terminal lists the chest beside its connector",
        mods=["toms"], scene="terminal",
        steps=['sleep 2; $C state > $S/state.json'],
        checks=[check_json("state.json", ["screen"], "container", "the terminal's screen is open"),
                check_log(r"Java mods: commands", "the mods started")],
        look="the terminal shows 12 diamonds and 40 oak logs",
    ),
    "placed": dict(
        about="A mod block placed by the player's right-click (Farmer's Delight's stove, placed by the "
              "mod's BlockItem in the mirror) is in the player's game from the click on: the click's "
              "correction no longer sends air before the mod's block is in the level (it vanished)",
        mods=["fd"], scene="none",
        steps=['"$R/tools/java_bridge/around.sh" \'q "give @s farmersdelight:stove"\' '
               '\'$C select 1 >/dev/null; $C look-at-block $((X+2)) $((Y-1)) $Z up >/dev/null; sleep 0.5; '
               '$C mouse right click >/dev/null; for i in $(seq 40); do $C block $((X+2)) $Y $Z; '
               'sleep 0.05; done > $S/placed.txt; $C look-at-block $((X+2)) $Y $Z >/dev/null; sleep 2\''],
        checks=[check_file("placed.txt", r'"block":"farmersdelight:stove"', "the stove is placed"),
                check_file("placed.txt", r'\A(?:[^\n]*"minecraft:air"[^\n]*\n)*'
                           r'(?:[^\n]*"block":"farmersdelight:stove"[^\n]*\n)+\Z',
                           "once there, the stove never turns back to air in the player's game")],
        look="the stove in front of the player",
    ),
    "kinetic": dict(
        about="Create's client patterns (kineticmod fixture): a block entity renderer whose helper is made at the end "
              "of the level renderer's reload (LevelExtractor.allChanged, from a Mixin plugin's list) draws a turning "
              "shaft; the client tick reads the player's first-person hands and its class's synced data",
        fixtures=["kineticmod"], scene="none", env={"HORIZON_JAVA_DEBUG": "ber"},
        steps=['"$R/tools/java_bridge/around.sh" \'q "setblock $((X+2)) $Y $Z horizon_kinetic:shaft"\' '
               '\'$C look-at $(p $X+2.5) $((Y+1)) $(p $Z+0.5) >/dev/null; sleep 6\''],
        checks=[check_log(r"mod injection\(s\) at RETURN of LevelExtractor\.allChanged", "the level renderer's reload ran the mod's injection"),
                check_log(r"horizon_kinetic: client ready \(hands FirstPersonHandsAndItems, data for LocalPlayer, reload ended true\)",
                          "the client tick reads the hands and the player's data; the resource reload's end was told"),
                check_log(r"ber horizon_kinetic:shaft at .*: [1-9]\d* model\(s\).*Speed:16", "the shaft's renderer draws it, turning")],
        forbid=[r"drawing horizon_kinetic", r"no synced data accessor", r"failed in the client's tick"],
        look="a turning wooden cube and a small orange one beside it, above the shaft",
    ),
    "create": dict(
        about="Create (Create Fly's unofficial 26.3 port; Flywheel inside): a creative motor turns a shaft and a "
              "cogwheel. Flywheel's model helpers are made at the level renderer's reload (its backend off), so "
              "Create's block entity renderers draw the turning parts; the network's speed is read in Java",
        mods=["create"], optional=["fabric-api"], scene="none", env={"HORIZON_JAVA_DEBUG": "ber"}, timeout=900,
        steps=['"$R/tools/java_bridge/around.sh" '
               '\'q "setblock $((X+2)) $Y $((Z-1)) create:creative_motor[facing=south]"\' '
               '\'q "setblock $((X+2)) $Y $Z create:shaft[axis=z]"\' '
               '\'q "setblock $((X+2)) $Y $((Z+1)) create:cogwheel[axis=z]"\' '
               '\'q "tp @s $(p $X-0.5) $Y $(p $Z+0.5)"\' '
               '\'$C look-at $(p $X+2.5) $(p $Y+0.5) $(p $Z+0.5) >/dev/null; sleep 8\''],
        checks=[check_log(r"mod injection\(s\) at RETURN of LevelExtractor\.allChanged", "Flywheel's level renderer reload ran"),
                check_log(r"ber create:motor at .*: [1-9]\d* model\(s\)", "the motor's renderer draws its shaft"),
                check_log(r"ber create:bracketed_kinetic at .*: [1-9]\d* model\(s\).*Speed:-?[1-9]",
                          "the shaft and cogwheel draw, turning (their Speed in Java isn't 0)")],
        forbid=[r"drawing create:", r"ModelRenderHelper", r"firstPersonHandsAndItems",
                r"SynchedEntityData\.get"],
        look="the motor, shaft and cogwheel in front of the player, turned (two screenshots differ)",
    ),
    "ber": dict(
        about="Farmer's Delight's block entity renderers: beef on the stove, a carrot on the board",
        mods=["fd"], scene="ber",
        steps=[],
        checks=[check_log(r"Java mods: commands", "the mods started")],
        look="beef on the stove's grill, a carrot on the cutting board",
    ),
    "scaffold": dict(
        about="Create's andesite scaffolding (a vanilla ScaffoldingBlock subclass, no cached collision "
              "shape): placed by hand, walked into, stood on. It stopped the game with \"unsupported block "
              "collision state\"; now it follows vanilla scaffolding's rule (its template's), and a mod state "
              "with no rule at all collides as a full block",
        mods=["create"], optional=["fabric-api"], scene="none", timeout=900,
        steps=['"$R/tools/java_bridge/around.sh" '
               '\'q "gamemode survival"\' '
               '\'q "give @s create:andesite_scaffolding 8"\' '
               '\'$C select create:andesite_scaffolding >/dev/null; $C place $((X+2)) $Y $Z > $S/place.json\' '
               # Walking east into it: vanilla's scaffolding is empty from the side, so the player
               # passes through its block.
               '\'$C look -90 0 >/dev/null; $C move forward 1.5 >/dev/null; $C state | python3 -c "'
               'import json,sys; x=json.load(sys.stdin)[\\"player\\"][\\"position\\"][0]; '
               'print(\\"through\\" if x > $X+3 else \\"stopped at %s\\" % x)" > $S/inside.txt\' '
               # Standing on it: its top is a floor.
               '\'q "tp @s $(p $X+2.5) $((Y+1)) $(p $Z+0.5)"; sleep 2; $C state | python3 -c "'
               'import json,sys; p=json.load(sys.stdin)[\\"player\\"]; '
               'print(\\"on top\\" if p[\\"position\\"][1] == $Y+1 and p[\\"on_ground\\"] else \\"fell to %s\\" % p[\\"position\\"])" > $S/ontop.txt\' '
               '\'$C look 90 35 >/dev/null\''],
        checks=[check_file("place.json", r'"block":"create:andesite_scaffolding"', "the scaffolding is placed by hand"),
                check_file("inside.txt", r"^through", "the player walks through it from the side"),
                check_file("ontop.txt", r"^on top", "the player stands on it")],
        forbid=[r"unsupported block collision state", r"Horizon crashed"],
        look="none-1: the view from on top of the scaffolding, looking down at the stone floor",
    ),
    "createwhole": dict(
        about="Create's shafts and cogwheel drawn only by their renderers (their block models give nothing in "
              "a world: no still copy under the turning one), in their sprites' own UVs (Flywheel's "
              "SuperByteBuffers draw in the block atlas's)",
        mods=["create"], optional=["fabric-api"], scene="create", env={"HORIZON_JAVA_DEBUG": "ber"},
        steps=['sleep 4'],
        checks=[check_log(r"drawn create:bracketed_kinetic Block\{create:cogwheel\}\S*: .*create:textures/block/cogwheel\S* \([1-9].*drawn whole by its renderer",
                          "the cogwheel's renderer draws it, in its sprites, whole"),
                check_log(r"drawn create:bracketed_kinetic Block\{create:shaft\}\S*: .*drawn whole by its renderer", "the shaft is drawn whole"),
                check_log(r"drawn create:motor .*create:textures/block/axis\S* \([1-9]", "the motor's shaft is drawn")],
        forbid=[r"Java mods: drawing create:", r"ModelRenderHelper"],
        look="create-0: the purple motor, a shaft, a wooden cogwheel and a shaft in a row; create-1: the cog turned a little",
    ),
    "createmachines": dict(
        about="Create's machines (scene machines): a press makes iron sheets of the ingots a click put on its "
              "depot (the click's off hand is the mod's use's: Horizon's client can't know a mod block took it), "
              "a millstone grinds the wheat a hopper feeds it, a deployer places the cobblestone a hopper gives it, "
              "a belt made between two shafts with its item (its first shaft's cooldown runs out: mirrored "
              "players' item cooldowns tick), its connector's client particles drawn as packets",
        mods=["create"], optional=["fabric-api"], scene="machines", env={"HORIZON_JAVA_DEBUG": "ber"}, timeout=900,
        steps=['read X Y Z < $S/pos; $C command "give @s minecraft:iron_ingot 4" >/dev/null; sleep 0.5; '
               '$C look-at block $((X+3)) $Y $((Z-2)) up >/dev/null; sleep 0.5; $C mouse right click >/dev/null; sleep 1; '
               '$C command "clear @s" >/dev/null; $C command "give @s create:belt_connector 4" >/dev/null; sleep 0.5; '
               '$C look-at-block $((X-2)) $Y $((Z-3)) >/dev/null; sleep 0.5; $C mouse right click >/dev/null; sleep 1; '
               '$C look-at-block $((X-2)) $Y $((Z+1)) >/dev/null; sleep 0.5; $C mouse right click >/dev/null; sleep 1; '
               '$C command "clear @s" >/dev/null; sleep 1',
               'read X Y Z < $S/pos; $C command "tp @s $((X-1)) $Y $Z -90 12" >/dev/null; sleep 12',
               'read X Y Z < $S/pos; $C block $((X+3)) $Y $((Z+3)) > $S/deployed.json; '
               '$C command "tp @s $((X+1)) $Y $((Z-1)) 90 35" >/dev/null; sleep 3'],
        checks=[check_log(r"ber create:depot at .*create:iron_sheet", "the press made iron sheets on the depot"),
                check_log(r"ber create:millstone at .*create:wheat_flour", "the millstone ground wheat to flour"),
                check_log(r"ber create:belt at .*Length:5", "the belt was made between the shafts"),
                check_file("deployed.json", r"minecraft:cobblestone", "the deployer placed its cobblestone")],
        forbid=[r"Java mods: drawing create:", r"ParticleEngine\.createParticle", r"failed to tick"],
        look="createmachines-2: the press over its depot, the millstone under its hopper, the deployer and its "
             "cobblestone; createmachines-3: the belt between its shafts",
    ),
    "contraption": dict(
        about="A Create contraption: a mechanical bearing on a creative motor lifts the planks on it into a "
              "contraption entity (the mirror's levels have no neighbour updater: the shape updates are "
              "Horizon's) and turns it; its renderer gets the client's block models",
        mods=["create"], optional=["fabric-api"], scene="contraption", timeout=900,
        # The bearing's side away from its middle: the middle is its value box (Create's client
        # part takes that click: the rotation mode's settings, as in a vanilla client).
        steps=['read X Y Z < $S/pos; $C command "clear @s" >/dev/null; $C look-at $(awk "BEGIN{print $X+3.01}") $(awk "BEGIN{print $Y+1.12}") $(awk "BEGIN{print $Z+0.5}") >/dev/null; '
               'sleep 0.5; $C mouse right click >/dev/null; sleep 3; $C block $((X+3)) $((Y+2)) $Z > $S/lifted.json',
               'sleep 2'],
        checks=[check_file("lifted.json", r"minecraft:air", "the planks left the world: they are the contraption's")],
        forbid=[r"no renderer for create:", r"Cannot invoke \"net\.minecraft\.world\.level\.redstone\.CollectingNeighborUpdater", r"failed to tick"],
        look="contraption-1 and contraption-2: the planks on the bearing, turned by different angles",
    ),
    "earlyclient": dict(
        about="A tester's client mods (Async Particles, Immersive Hotbar, EMF/ETF, ShulkerBoxTooltip, FancyMenu, "
              "Continuity, JEI's mezz_config, Xaero's World Map): the GPU device stays when a mod fails at its "
              "start (Async Particles cast it to a Vulkan device), options.txt loads, the client has a particle "
              "engine and models from its start, receivers registered at a connection's INIT, Continuity's "
              "reloaders run with the atlases",
        mods=["asyncparticles", "immersivehotbar", "yacl", "emf", "etf", "shulkerboxtooltip", "fancymenu", "konkrete",
              "melody", "continuity", "jei", "mezzconfig", "xaeroworldmap", "fabric-api"], scene="none", timeout=900,
        steps=['sleep 20; $C command "give @s minecraft:diamond_sword" >/dev/null; sleep 3'],
        checks=[check_log(r"Horizon's client: GPU ", "the GPU device is up"),
                check_log(r"block and item models loaded", "the client's models loaded")],
        forbid=[r"Failed to load options", r"GPU device failed to start", r"\"mc\.particleEngine\" is null",
                r"Cannot register receiver while not in game", r"Invalid atlas id", r"Unknown element id",
                r"ModelManager\.\w+\(\)\" because .* is null", r"Fallback pipeline cache not yet set",
                r"\"stack\" is null", r"pendingActions"],
        look="earlyclient-1: Xaero's minimap, the hotbar with the sword, no red failure lines in chat",
    ),
    "creategoggles": dict(
        about="Create's client behaviours on the client's own block entities: goggles worn, looking at a creative "
              "motor shows its stress capacity (a HUD overlay); its value box's hover tip shows",
        mods=["create"], optional=["fabric-api"], scene="none", env={"HORIZON_JAVA_DEBUG": "hud,cbe"}, timeout=900,
        steps=['sleep 8; "$R/tools/java_bridge/around.sh" '
               '\'q "setblock $((X+2)) $Y $Z create:creative_motor[facing=up]"\' '
               '\'q "setblock $((X+2)) $((Y+1)) $Z create:shaft[axis=y]"\' '
               '\'q "item replace entity @s armor.head with create:goggles"\' '
               '\'$C look-at $(p $X+2.5) $(p $Y+0.4) $(p $Z+0.5) >/dev/null; sleep 4\''],
        checks=[check_log(r"cbe: client copy of com\.zurrtum\.create\.content\.kinetics\.motor\.CreativeMotorBlockEntity",
                          "the client has its own copy of the motor's block entity"),
                check_log(r"hud: [1-9]\d* ops kept.*creative_motor.*wearing 1 create:goggles", "the goggles' overlay draws")],
        forbid=[r"failed in its client tick"],
        look="creategoggles-1: the motor, a dark tooltip box \"Generator Stats: Kinetic Stress Capacity\" beside the crosshair",
    ),
    "createvalue": dict(
        about="Create's value box (a creative motor's speed): right-click held on it opens its settings screen "
              "(the client's useItemOn injections run before the server's use; the use key is down while the "
              "click lasts), a right-click on the board sets the value (its packet type to the server's handler)",
        mods=["create"], optional=["fabric-api"], scene="none", env={"HORIZON_JAVA_DEBUG": "ber,loop,frames"}, timeout=900,
        steps=['sleep 8; "$R/tools/java_bridge/around.sh" '
               '\'q "setblock $((X+2)) $Y $Z create:creative_motor[facing=up]"\' '
               '\'q "setblock $((X+2)) $((Y+1)) $Z create:shaft[axis=y]"\' '
               '\'$C look-at-block $((X+2)) $Y $Z west >/dev/null\'; sleep 2',
               '$C mouse right down >/dev/null; sleep 1.5; $C state > $S/open.json',
               '$C click-at 360 125 right >/dev/null; sleep 3; $C mouse right up >/dev/null 2>&1; $C state > $S/closed.json'],
        checks=[check_json("open.json", ["java_screen"], -1, "the settings screen is open while the click is held"),
                check_log(r"ValueSettingsScreen", "it is Create's value settings screen"),
                check_json("closed.json", ["screen"], "playing", "the release on the board closed it"),
                check_log(r"loop: client to server packet com\.zurrtum\.create\.infrastructure\.packet\.c2s\.ValueSettingsPacket",
                          "its packet reached the server"),
                check_log(r"ber create:motor at .*ScrollValue:2[0-9][0-9]", "the motor's speed changed (from 16)")],
        look="createvalue-1: the board \"Generated Speed in RPM\", \"Release Right Button to Confirm\"",
    ),
    "createtoolbox": dict(
        about="Create's toolbox opens its screen: Create's own packet type (OpenScreenPacket) reaches the player's "
              "client connection, whose packet processor runs it; its menu reads the client's toolbox",
        mods=["create"], optional=["fabric-api"], scene="none", env={"HORIZON_JAVA_DEBUG": "loop,frames"}, timeout=900,
        steps=['sleep 8; "$R/tools/java_bridge/around.sh" '
               '\'q "setblock $((X+2)) $Y $Z create:white_toolbox"\' '
               '\'$C look-at-block $((X+2)) $Y $Z >/dev/null; sleep 0.5; $C mouse right click >/dev/null; sleep 3; $C state > $S/state.json\''],
        checks=[check_log(r"loop: server to client packet com\.zurrtum\.create\.infrastructure\.packet\.s2c\.OpenScreenPacket",
                          "Create's opening reached the client"),
                check_json("state.json", ["java_screen"], -1, "the toolbox's screen is open")],
        forbid=[r"PacketProcessor\.isSameThread", r"A mod sent a player a OpenScreenPacket"],
        look="createtoolbox-1: the toolbox's eight compartments over the inventory",
    ),
    "createsounds": dict(
        about="Mods' client sounds (SoundManager.play from client code: Create's frogport opening, in the client "
              "half of its use, from its client behaviour) go to the player's game",
        mods=["create"], optional=["fabric-api"], scene="none", env={"HORIZON_JAVA_DEBUG": "sounds"}, timeout=900,
        steps=['sleep 8; "$R/tools/java_bridge/around.sh" '
               '\'q "setblock $((X+2)) $Y $Z create:package_frogport"\' '
               '\'$C look-at-block $((X+2)) $Y $Z >/dev/null; sleep 1; $C mouse right click >/dev/null; sleep 3\''],
        checks=[check_log(r"sounds: create:frogport_open for ", "the frogport's opening sound is played")],
        forbid=[r"FrogportAudioBehaviour", r"failed in using a block"],
        look="createsounds-1: the frogport's screen",
    ),
    "modscroll": dict(
        about="Mods' injections into MouseHandler.onScroll while playing (kineticmod, as Create's: before the hotbar, "
              "the scaled offset as a local): a held stick takes the wheel (the hotbar is put back), else it moves",
        fixtures=["kineticmod"], scene="none", env={"HORIZON_JAVA_DEBUG": "screens"},
        steps=['sleep 3; $C command "clear @s" >/dev/null; $C command "give @s minecraft:stick" >/dev/null; '
               '$C command "give @s minecraft:dirt" >/dev/null; sleep 1; $C select 1 >/dev/null; sleep 0.5; '
               '$C scroll-at 213 120 -1 >/dev/null; sleep 1.5; $C state > $S/taken.json; '
               '$C select 2 >/dev/null; sleep 0.5; $C scroll-at 213 120 -1 >/dev/null; sleep 1.5; $C state > $S/passed.json'],
        checks=[check_log(r"horizon_kinetic: scroll -1\.0 taken", "the mod took the wheel over the stick"),
                check_json("taken.json", ["selected"], 1, "the hotbar stayed on the stick"),
                check_log(r"horizon_kinetic: scroll -1\.0 passed", "the mod let it pass over dirt"),
                check_json("passed.json", ["selected"], 3, "the hotbar moved")],
    ),
    "createclick": dict(
        about="Create's clicks a block takes first (bridge-mixins-11): a mechanical arm's point chosen by "
              "right-clicking a depot (Create's client code ends the click before its packet: the client's "
              "half runs first), the arm placed asks the client for it (a mod's own packets, both ways); "
              "train tracks placed with their item (onPlace joins them to the railway graph), a station "
              "bound to the track it targets (no 'Right-click the targeted train track first'); two chain "
              "conveyors joined with a chain; sandpaper held on the rose quartz in the other hand polishes it "
              "(a mod item's use over time runs in the mirror while the button is held)",
        mods=["create"], optional=["fabric-api"], scene="createclick", env={"HORIZON_JAVA_DEBUG": "ber"}, timeout=900,
        steps=['read X Y Z < $S/pos; $C command "give @s create:mechanical_arm" >/dev/null; sleep 1; '
               '$C look-at block $((X+2)) $Y $((Z-2)) up >/dev/null; sleep 0.5; $C mouse right click >/dev/null; sleep 1; '
               '$C messages 1 > $S/arm_msg.json; '
               '$C look-at block $((X+1)) $((Y-1)) $((Z-1)) up >/dev/null; sleep 0.5; $C mouse right click >/dev/null; sleep 2',
               'read X Y Z < $S/pos; $C command "clear @s" >/dev/null; $C command "give @s create:track 16" >/dev/null; sleep 1; '
               'for dx in -3 -2 -1 0 1 2 3; do $C command "tp @s $((X+dx)) $Y $((Z+1)) -90 70" >/dev/null; sleep 0.4; '
               '$C look-at block $((X+dx+1)) $((Y-1)) $((Z+2)) up >/dev/null; sleep 0.3; $C mouse right click >/dev/null; sleep 0.6; done; '
               '$C command "tp @s $X $Y $Z 0 30" >/dev/null; $C command "clear @s" >/dev/null; '
               '$C command "give @s create:track_station" >/dev/null; sleep 1; '
               '$C look-at block $((X+1)) $Y $((Z+2)) up >/dev/null; sleep 0.5; $C mouse right click >/dev/null; sleep 1; '
               '$C messages 1 > $S/station1.json; '
               '$C look-at block $((X+1)) $((Y-1)) $((Z+1)) up >/dev/null; sleep 0.5; $C mouse right click >/dev/null; sleep 2; '
               '$C messages 1 > $S/station2.json; $C block $((X+1)) $Y $((Z+1)) > $S/station.json',
               'read X Y Z < $S/pos; $C command "clear @s" >/dev/null; $C command "give @s minecraft:iron_chain 32" >/dev/null; sleep 1; '
               '$C look-at block $((X-3)) $Y $((Z-3)) up >/dev/null; sleep 0.5; $C mouse right click >/dev/null; sleep 1; '
               '$C look-at block $((X+3)) $Y $((Z-3)) up >/dev/null; sleep 0.5; $C mouse right click >/dev/null; sleep 2',
               'read X Y Z < $S/pos; $C command "clear @s" >/dev/null; $C command "give @s create:sand_paper" >/dev/null; '
               '$C command "item replace entity @s weapon.offhand with create:rose_quartz 1" >/dev/null; '
               '$C command "tp @s $X $Y $Z 0 -60" >/dev/null; sleep 1; '
               '$C mouse right down >/dev/null; sleep 3; $C mouse right up >/dev/null; sleep 1; $C inventory > $S/sand.json'],
        checks=[check_file("arm_msg.json", r'"actionbar":"Take items from Depot"', "the arm's client chose the depot"),
                check_log(r'ber create:mechanical_arm at .*InteractionPoints:\[\{Mode:"take",Pos:\[[^]]*\],Type:"create:depot"',
                          "the placed arm has the depot as its point (its client sent it)"),
                check_file("station1.json", r'"actionbar":"Targeted track selected"', "the station's item took the track"),
                check_file("station2.json", r'"actionbar":"Successfully bound to targeted track"', "the station was bound"),
                check_file("station.json", r"create:track_station", "the station stands"),
                check_log(r"ber create:chain_conveyor at .*Connections:\[\[I;-?6,0,0\]\]", "the conveyors are joined"),
                check_file("sand.json", r'"create:polished_rose_quartz":1\b', "the sandpaper polished the rose quartz")],
        forbid=[r"Right-click the targeted train track first", r"failed to tick", r"PacketProcessor"],
        look="createclick-0..3: the arm by its depot, the station on its track, the chain between the conveyors",
    ),
    "createitemsin": dict(
        about="Horizon's items into Create's blocks (bridge-mixins-11: they stand in the mirror, HorizonItems): "
              "iron ingots thrown on a depot go onto it, wheat on a turning millstone into it, cobblestone "
              "between crushing wheels is pulled in (held: the mirror moves it, the controller's shape lets it "
              "through) and crushed, raw iron blown by a fan through lava is smelted",
        mods=["create"], optional=["fabric-api"], scene="createitemsin", env={"HORIZON_JAVA_DEBUG": "ber"}, timeout=900,
        steps=["read X Y Z < $S/pos; at(){ awk \"BEGIN{print $1}\"; }; "
               "$C command \"summon item $(at $X+2.5) $((Y+2)) $(at $Z-1.5) {Item:{id:'minecraft:iron_ingot',count:3},PickupDelay:200}\" >/dev/null; "
               "$C command \"summon item $(at $X+3.5) $((Y+3)) $(at $Z+1.5) {Item:{id:'minecraft:wheat',count:2},PickupDelay:200}\" >/dev/null; "
               "$C command \"summon item $(at $X-1.5) $((Y+3)) $(at $Z-1.5) {Item:{id:'minecraft:cobblestone',count:2},PickupDelay:200}\" >/dev/null; "
               "$C command \"summon item $(at $X-1.5) $Y $(at $Z+1.5) {Item:{id:'minecraft:raw_iron',count:1},PickupDelay:200}\" >/dev/null; "
               "sleep 15; $C entities 12 > $S/items.json; $C look-at block $((X+2)) $Y $((Z-2)) up >/dev/null; sleep 1"],
        checks=[check_log(r'ber create:depot at .*Item:\{count:3,id:"minecraft:iron_ingot"\}', "the depot holds the thrown ingots"),
                check_log(r"ber create:millstone at .*(minecraft:wheat|create:wheat_flour)", "the millstone took the wheat"),
                check_file("items.json", r'"minecraft:gravel"', "the crushing wheels made gravel of the cobblestone"),
                check_file("items.json", r'"minecraft:iron_ingot"', "the fan smelted the raw iron")],
        forbid=[r"failed to tick", r"Horizon's items can't stand in the mirror"],
        look="createitemsin-1: the ingots on the depot",
    ),
    "createfluids": dict(
        about="Lava's interactions are Create's when it patches them (bridge-mixins-11: LiquidBlock."
              "shouldSpreadLiquid on the mirror): lava over still water makes obsidian, cobblestone and stone "
              "once and stays (no loop); flowing lava by honey makes limestone, by chocolate scoria, a lava "
              "source by honey obsidian",
        mods=["create"], optional=["fabric-api"], scene="createfluids", timeout=900,
        steps=['read X Y Z < $S/pos; $C command "setblock $((X-4)) $((Y-1)) $((Z-3)) lava" >/dev/null; sleep 1; '
               '$C command "setblock $((X-2)) $((Y+1)) $((Z-2)) lava" >/dev/null; sleep 16; '
               '$C blocks $((X-4)) $((Y-1)) $((Z-4)) $((X-1)) $((Y+1)) $((Z-1)) > $S/lava1.json; sleep 10; '
               '$C blocks $((X-4)) $((Y-1)) $((Z-4)) $((X-1)) $((Y+1)) $((Z-1)) > $S/lava2.json; '
               '(cmp -s $S/lava1.json $S/lava2.json && echo same || echo changed) > $S/stable.txt',
               'read X Y Z < $S/pos; $C command "setblock $((X+2)) $Y $((Z-3)) create:honey" >/dev/null; '
               '$C command "setblock $((X+3)) $Y $((Z-3)) create:chocolate" >/dev/null; sleep 0.3; '
               '$C command "setblock $((X+2)) $Y $((Z-2)) minecraft:lava[level=2]" >/dev/null; '
               '$C command "setblock $((X+3)) $Y $((Z-2)) minecraft:lava[level=2]" >/dev/null; '
               '$C command "setblock $((X+1)) $Y $((Z-3)) minecraft:lava" >/dev/null; sleep 3; '
               '$C blocks $((X+1)) $Y $((Z-3)) $((X+3)) $Y $((Z-2)) > $S/honey.json',
               'read X Y Z < $S/pos; $C command "tp @s $X $((Y+3)) $((Z+2)) 180 50" >/dev/null; sleep 2'],
        checks=[check_file("lava1.json", r"minecraft:obsidian", "lava by water made obsidian"),
                check_file("stable.txt", r"^same", "and nothing changed after (no loop)"),
                check_file("lava2.json", r"minecraft:obsidian", "the obsidian stays"),
                check_file("honey.json", r'"create:limestone"', "flowing lava by honey made limestone"),
                check_file("honey.json", r'"create:scoria"', "flowing lava by chocolate made scoria"),
                check_file("honey.json", r'"minecraft:obsidian"', "a lava source by honey made obsidian")],
        look="createfluids-3: the pool (obsidian, cobblestone under the lava) on the left, limestone and scoria on the right",
    ),
    "createparts": dict(
        about="Create's parts their renderers draw, and its blocks beside others (scene createparts): the saw's "
              "blade (its renderer reads the client's sprites from its context), the mixer's head (a client "
              "behaviour: renderers draw the client's copy of a block entity, made from its update packet and "
              "ticked as a client's), the blaze burner's head; no pale square under the basin or the burner "
              "(a mod block's own occlusion, not its stand-in vanilla state's full cube); a hand crank pops "
              "off when its cogwheel goes (Horizon's neighbour updates reach the mods' blocks)",
        mods=["create"], optional=["fabric-api"], scene="createparts", env={"HORIZON_JAVA_DEBUG": "ber"}, timeout=900,
        steps=['sleep 6',
               'read X Y Z < $S/pos; $C command "setblock $((X+2)) $Y $((Z+2)) air" >/dev/null; sleep 4; '
               '$C block $((X+2)) $((Y+1)) $((Z+2)) > $S/crank.json; '
               '$C command "tp @s $((X+1)) $((Y+1)) $((Z-1)) -60 30" >/dev/null; sleep 3'],
        checks=[check_log(r"ber create:saw at .*: [1-9]\d* model\(s\)", "the saw's renderer draws its blade"),
                check_log(r"ber create:mechanical_mixer at .*: [1-9]\d* model\(s\)", "the mixer's renderer draws (its client copy)"),
                check_log(r"ber create:blaze_heater at .*: [1-9]\d* model\(s\)", "the blaze burner's head draws"),
                check_file("crank.json", r'"block":"minecraft:air"', "the hand crank broke with its cogwheel")],
        forbid=[r"no renderer for create:saw", r"no client copy of create:", r"Java mods: drawing create:mechanical_mixer"],
        look="createparts-0: the burner under its basin, the mixer over it, the press over a basin, the saw "
             "with its blade, no pale squares on the floor under them; createparts-2 from closer",
    ),
    "jeifluids": dict(
        about="Fluids in screens: JEI's list draws water from its still sprite (vanilla's blitTiledSprite, "
              "through the client's real block atlas), tinted, not as a black square",
        mods=["jei"], optional=["fabric-api", "create"], scene="none", env={"HORIZON_JAVA_DEBUG": "tiled"}, timeout=900,
        steps=['$C command "gamemode survival" >/dev/null; sleep 10; $C key e tap >/dev/null; sleep 12; '
               '$C click-at 357 223 >/dev/null; sleep 1; for k in w a t e r; do $C key $k tap >/dev/null; sleep 0.4; done; sleep 4'],
        checks=[check_log(r"gui: a tiled sprite, minecraft:textures/block/water_still\.png tinted [0-9a-f]*3f76e4",
                          "JEI's water is its still sprite, tinted (water's colour)")],
        look="jeifluids-1: JEI's list for \"water\": the water fluid a blue square beside the bucket and potions",
    ),
    "createitems": dict(
        about="Create's items drawn by its own item model types (the wrench, the extendo grip, the backtanks' "
              "OBJ with `map_Kd 0`): the quads its Java models make reach Horizon's item renderer; the "
              "creative tab's first row isn't blank, the wrench draws in hand",
        mods=["create"], optional=["fabric-api"], scene="none", timeout=900,
        steps=['sleep 8; $C command "give @s create:wrench" >/dev/null; sleep 2',
               '$C key e tap >/dev/null; sleep 2; $C click-at 301 12 >/dev/null; sleep 2'],
        checks=[check_log(r"Java mods: [1-9]\d* item\(s\) drawn with their Java models' quads", "the game takes Java's item models")],
        forbid=[r"item create:\w+'s model couldn't be read"],
        look="createitems-1: the wrench in hand; createitems-2: Create's creative tab, no blank slots in its first rows",
    ),
    "fluids": dict(
        about="Mods' fluids drawn with their own sprites (Create's honey and chocolate: their client "
              "fluid models, read once the models load), not as lava",
        mods=["create"], optional=["fabric-api"], scene="fluids",
        steps=[],
        checks=[check_log(r"2 mod fluid\(s\) drawn with their own sprites: .*create:block/honey_still", "Java reads honey's sprites"),
                check_log(r"Java mods: 2 fluid\(s\) drawn with their own sprites \(32 states\)", "the game takes them")],
        look="fluids-0: two holes in the floor, golden honey on the left, brown chocolate on the right (not lava)",
    ),
    "skinlayers": dict(
        about="3D Skin Layers: the player's outer skin layers as 3D voxels (its meshes on the "
              "PlayerModel's parts, rendered by the bridge as custom geometry)",
        mods=["skinlayers", "fabric-api"], scene="none",
        steps=['sleep 4; $C key f5 tap >/dev/null; $C key f5 tap >/dev/null; $C look 0 10 >/dev/null; sleep 4'],
        checks=[check_log(r"mods draw [1-9][0-9]* quads of a player's model parts their own way", "the layers' meshes are drawn")],
        forbid=[r"the player's model parts mods draw couldn't be drawn"],
        look="none-1: the player from the front; jacket, sleeves and trousers stand out in 3D",
    ),
    "hud": dict(
        about="Jade's tooltip and AppleSkin on Horizon's HUD",
        mods=["fd", "jade", "appleskin"], scene="none", env={"HORIZON_JAVA_DEBUG": "hud"},
        steps=['$C command "gamemode survival" >/dev/null; $C look 0 80 >/dev/null; sleep 4'],
        checks=[check_log(r"hud element jade:main: [1-9]", "Jade draws its tooltip"),
                check_log(r"hud: [1-9][0-9]* ops kept", "mods' HUD parts reach the game")],
        forbid=[r"HUD's \S+ failed"],
        look="Jade's box at the top: the block below, its mod in blue",
    ),
    "rei": dict(
        about="REI's recipe view from its own keys over Horizon's inventory: R over a slot (the slot hovered in the screen "
              "under the overlay), for a vanilla and a Farmer's Delight item; Escape back to the inventory",
        mods=["rei", "architectury", "cloth", "fd"], scene="none",
        steps=['$C command "clear @s" >/dev/null; $C command "give @s minecraft:crafting_table" >/dev/null; '
               '$C command "give @s farmersdelight:beef_stew" >/dev/null; $C command "gamemode survival" >/dev/null; '
               'sleep 8; $C key e tap >/dev/null; sleep 8; $C ui > $S/ui.json',
               # The mouse over hotbar slot 1 (a click picks the item up, the next puts it back), then R.
               'P=$(python3 -c "import json,sys; b={w[\'id\']: w[\'bounds\'] for w in json.load(open(sys.argv[1]))[\'widgets\']}[\'slot-0\']; '
               'print(int(b[0] + 8), int(b[1] + 8))" $S/ui.json); $C click-at $P >/dev/null; sleep 1; $C click-at $P >/dev/null; sleep 1; '
               '$C key r tap >/dev/null; sleep 5; $C state > $S/table.json',
               '$C key escape tap >/dev/null; sleep 3; $C state > $S/back.json; '
               'P=$(python3 -c "import json,sys; b={w[\'id\']: w[\'bounds\'] for w in json.load(open(sys.argv[1]))[\'widgets\']}[\'slot-1\']; '
               'print(int(b[0] + 8), int(b[1] + 8))" $S/ui.json); $C click-at $P >/dev/null; sleep 1; $C click-at $P >/dev/null; sleep 1; '
               '$C key r tap >/dev/null; sleep 5; $C state > $S/stew.json'],
        checks=[check_log(r"client got the server's recipes \([0-9]{4,}", "the client's join gets the server's recipes (REI reloads there)"),
                check_file("table.json", r'"screen":"java-screen"', "R over the crafting table opens REI's recipes"),
                check_file("back.json", r'"screen":"inventory"', "Escape goes back to the inventory"),
                check_file("stew.json", r'"screen":"java-screen"', "R over beef stew opens its recipe")],
        forbid=[r"wrap failed", r"no recipes for the client's join"],
        look="none-1.png: the crafting table's recipe; none-2.png: beef stew's (Farmer's Delight's cooking pot)",
    ),
    "jei": dict(
        about="JEI over Horizon's inventory: started by the client's join (the server's recipes, as a joining client gets them), "
              "its item list, its search box typed in (E too: a mod's text box has the keyboard), a right click on an entry "
              "(its uses screen), Escape back to the inventory, its R and U over Horizon's slots (a vanilla and a mod's item)",
        mods=["jei", "fd"], optional=["fabric-api"], scene="none",
        steps=['$C command "gamemode survival" >/dev/null; $C command "clear @s" >/dev/null; '
               '$C command "give @s minecraft:crafting_table" >/dev/null; $C command "give @s farmersdelight:tomato" >/dev/null; '
               'sleep 8; $C key e tap >/dev/null; sleep 12; $C ui > $S/ui.json; $C screenshot $S/full.png >/dev/null',
               # JEI's search box (bottom right), then "beef" typed: the keys go to the overlay.
               '$C click-at 357 223 >/dev/null; sleep 1; for k in b e e f; do $C key $k tap >/dev/null; sleep 0.4; done; sleep 3; '
               '$C state > $S/typed.json; $C screenshot $S/beef.png >/dev/null; '
               # Its list: item icons are sharp edges right of the inventory; few once filtered.
               'python3 -c "import sys; from PIL import Image, ImageFilter; '
               'e=lambda p: sum(Image.open(p).convert(\'L\').filter(ImageFilter.FIND_EDGES).crop((930, 90, 1280, 620)).getdata()); '
               'f, b = e(sys.argv[1]), e(sys.argv[2]); print(\'full\', \'yes\' if f > 2700000 else \'no\', \'filtered\', \'yes\' if b * 5 < f * 3 else \'no\', f, b)" '
               '$S/full.png $S/beef.png > $S/list.txt',
               # Each Backspace a key to the overlay too: the list full again; a right click on its first entry.
               'for k in 1 2 3 4; do $C key backspace tap >/dev/null; sleep 0.3; done; sleep 2; '
               '$C click-at 322 38 right >/dev/null; sleep 4; $C state > $S/uses.json',
               '$C key escape tap >/dev/null; sleep 3; $C state > $S/back.json',
               'P=$(python3 -c "import json,sys; b={w[\'id\']: w[\'bounds\'] for w in json.load(open(sys.argv[1]))[\'widgets\']}[sys.argv[2]]; '
               'print(int(b[0] + 8), int(b[1] + 8))" $S/ui.json slot-0); $C click-at $P >/dev/null; sleep 1; $C click-at $P >/dev/null; sleep 1; '
               '$C key r tap >/dev/null; sleep 4; $C state > $S/table.json; $C key escape tap >/dev/null; sleep 3; '
               'P=$(python3 -c "import json,sys; b={w[\'id\']: w[\'bounds\'] for w in json.load(open(sys.argv[1]))[\'widgets\']}[sys.argv[2]]; '
               'print(int(b[0] + 8), int(b[1] + 8))" $S/ui.json slot-1); $C click-at $P >/dev/null; sleep 1; $C click-at $P >/dev/null; sleep 1; '
               '$C key u tap >/dev/null; sleep 4; $C state > $S/tomato.json'],
        checks=[check_log(r"client got the server's recipes \([0-9]{4,}", "the client's join gets the server's recipes"),
                check_log(r"Registering recipes: farmersdelight:jei_plugin", "JEI starts with them (Farmer's Delight's plugin too)"),
                check_file("list.txt", r"full yes", "its item list over the inventory holds the items"),
                check_file("list.txt", r"filtered yes", "typing in its search box filters it"),
                check_file("typed.json", r'"screen":"inventory"', "E typed in its search box doesn't close the inventory"),
                check_file("uses.json", r'"screen":"java-screen"', "a right click on an entry opens its uses screen"),
                check_file("back.json", r'"screen":"inventory"', "Escape there goes back to the inventory"),
                check_file("table.json", r'"screen":"java-screen"', "R over the crafting table opens JEI's recipes"),
                check_file("tomato.json", r'"screen":"java-screen"', "U over a tomato opens a mod item's uses")],
        forbid=[r"no recipes for the client's join", r"wrap failed", r"screen event handler failed"],
        look="none-1.png: beef's items, 'beef' in the box; none-2.png: an entry's uses; none-4.png: the crafting table's recipe; none-5.png: tomato's uses",
    ),
    "sodium": dict(
        about="Sodium and Iris in mods-java: left out of Fabric's mods, the player told they aren't needed (no failure)",
        mods=["sodium", "iris", "fd"], scene="pot",
        steps=['sleep 2; $C messages 30 > $S/messages.json; $C state > $S/state.json'],
        checks=[check_log(r"Sodium isn't needed in Horizon", "the log says Sodium isn't needed"),
                check_file("messages.json", r"Iris isn't needed in Horizon: Horizon has its own renderer", "the player is told Iris isn't needed"),
                check_json("state.json", ["screen"], "container", "the other mods run (the pot's screen opens)")],
        forbid=[r"Your Java mods couldn't start", r"Mixin apply .* failed"],
        look="sodium-0.png: the pot's screen; the chat's gray [Java mods] lines",
    ),
    "replaymod": dict(
        about="ReplayMod records through the client's connection (HorizonPacketTap: the window's player's packets "
              "through a vanilla client pipeline); the .mcpr saved as the world closes plays in Horizon's replay "
              "viewer (--replay: spectating, the gold blocks, the pig and sheep, the recorder's body)",
        mods=["replaymod"], scene="none",
        steps=['$C command "tp @s ~ ~ ~ 0 20" >/dev/null; for i in 1 2 3 4 5; do '
               '$C command "setblock ~$i ~ ~4 minecraft:gold_block" >/dev/null; sleep 1; done; '
               '$C command "summon minecraft:pig ~2 ~ ~3" >/dev/null; $C command "summon minecraft:sheep ~4 ~ ~3" >/dev/null; '
               'sleep 4; $C messages 30 > $S/messages.json',
               # Save and quit: the recording is saved as the world closes.
               '$C quit >/dev/null; for i in $(seq 90); do ls $S/replay_recordings/*/*.mcpr >/dev/null 2>&1 && break; sleep 1; done; '
               'ls $S/replay_recordings/*/ > $S/files.txt',
               'S=$S OUT=$OUT $R/tools/java_bridge/replay_view.sh "" '
               '"sleep 8; \\$V entities > \\$S/view-entities.json; \\$V state > \\$S/view.json" '
               '"sleep 4" '
               '"sleep 6; \\$V command pause >/dev/null; sleep 1; \\$V messages 30 > \\$S/view-messages.json"'],
        checks=[check_file("messages.json", r"Recording started", "ReplayMod tells the player it records (its .lang translated)"),
                check_file("files.txt", r"\.mcpr", "the recording is saved as the world closes"),
                check_json("view.json", ["gamemode"], "spectator", "the viewer spectates"),
                check_file("view-entities.json", r"minecraft:pig", "the recorded pig is in the replay"),
                check_file("view-entities.json", r"minecraft:sheep", "the recorded sheep is in the replay"),
                check_file("view-messages.json", r"Replay paused", "chat lines control the replay")],
        forbid=[r"the client's connection goes on", r"failed in the client's"],
        timeout=900,
        look="none-1.png: RECORDING on the HUD, the gold blocks; view-2.png: the replay from behind the recorder's body",
    ),
    "flashback": dict(
        about="Flashback: its client command runs (/flashback: mods' client commands), and the player is told it "
              "can't record or replay here yet (its snapshot reads the vanilla client's own world)",
        mods=["flashback"], scene="none",
        steps=['sleep 3; $C messages 30 > $S/messages.json; $C command "flashback config" > $S/command.json'],
        checks=[check_file("messages.json", r"Flashback can't record or replay in Horizon yet", "the player is told Flashback's limits"),
                check_file("command.json", r'"ok":true', "its client command is known and runs"),
                check_log(r"mods' client commands: \[/flashback\]", "the client commands are registered")],
        forbid=[r"Your Java mods couldn't start"],
        look="none-1.png: the gray [Java mods] lines",
    ),
    "litematica": dict(
        about="Litematica's main menu on M (MaLiLib's hotkeys: every key while playing goes to the mods' KeyboardHandler.keyPress handlers)",
        mods=["litematica", "malilib"], scene="none",
        steps=['sleep 3; $C key m tap >/dev/null; sleep 3; $C state > $S/menu.json',
               # Its Configuration menu button (top right).
               '$C click-at 250 40 >/dev/null; sleep 3; $C state > $S/config.json'],
        checks=[check_file("menu.json", r'"screen":"java-screen"', "M opens Litematica's main menu"),
                check_file("config.json", r'"screen":"java-screen"', "its Configuration menu button opens a screen")],
        forbid=[r"key press failed"],
        look="none-0.png: Litematica's main menu (Schematic Placements, Load Schematics...); none-1.png: its configuration screen",
    ),
    "create-client": dict(
        about="Create (Create Fly, unofficial 26.3 port): its client tick and HUD run on Horizon's client player "
              "(firstPersonHandsAndItems, its synced data by the player's class, the HUD's contextual bar and waypoints)",
        mods=["create"], scene="none",
        steps=['$C command "setblock ~2 ~ ~ create:creative_motor[facing=west]" >/dev/null; '
               '$C command "setblock ~3 ~ ~ create:shaft[axis=x]" >/dev/null; sleep 6; $C state > $S/state.json'],
        checks=[check_json("state.json", ["screen"], "playing", "the player plays"),
                check_log(r"Fabric loaded .*create", "Create loads")],
        forbid=[r"firstPersonHandsAndItems", r'"accessor" is null', r"contextualInfoBar", r"ClientWaypointManager", r"HUD's \S+ failed"],
        look="none-0.png: a creative motor and a shaft beside the player (their moving parts need Flywheel: gpu chat)",
    ),
    "pot-jei": dict(
        about="JEI's uses screen opened from the cooking pot's screen, then Escape: back to FD's own screen "
              "(it came back as Horizon's one-row stand-in)",
        mods=["jei", "fd"], optional=["fabric-api"], scene="pot",
        steps=['sleep 12; $C click-at 322 38 right >/dev/null; sleep 4; $C state > $S/uses.json',
               '$C key escape tap >/dev/null; sleep 3; $C state > $S/back.json'],
        checks=[check_file("uses.json", r'"screen":"java-screen"', "a right click on JEI's list opens its uses screen over the pot"),
                check_file("back.json", r'"screen":"container"', "Escape goes back to the pot's window"),
                check_file("back.json", r'"java_screen":[0-9]+', "the pot's own screen, not the stand-in")],
        look="pot-jei-1.png: FD's cooking pot screen (grid, arrow, bowl slot) with JEI's list beside it",
    ),
    "jei-lan": dict(
        about="JEI per player: the host types in its search box; the guest's list stays full, its box empty and not typed in",
        mods=["jei", "fd"], optional=["fabric-api"], scene="none", lan=True,
        steps=['$H command "gamemode survival @a" >/dev/null; sleep 1; $H key e tap; sleep 12; $G key e tap; sleep 6',
               '$H click-at 357 223; sleep 1; for k in b e e f; do $H key $k tap >/dev/null; sleep 0.3; done; sleep 3; '
               '$G scroll-at 340 100 -1; sleep 3; $H screenshot $S/h.png >/dev/null; $G screenshot $S/g.png >/dev/null; '
               'python3 -c "import sys; from PIL import Image, ImageFilter; e=lambda p: sum(Image.open(p).convert(\'L\').filter(ImageFilter.FIND_EDGES).crop((930, 90, 1280, 620)).getdata()); h, g = e(sys.argv[1]), e(sys.argv[2]); '
               'print(\'host\', \'filtered\' if h < 2700000 else \'full\', \'guest\', \'filtered\' if g < 2700000 else \'full\', h, g)" '
               '$S/h.png $S/g.png > $S/lists.txt'],
        checks=[check_file("lists.txt", r"host filtered", "the host's search filters the host's list"),
                check_file("lists.txt", r"guest full", "the guest's list is still full (JEI's search text per player: not kept any more, see java-bridge.md)")],
        forbid=[r"JEI: could not start", r"search can't be kept per player"],
        look="lan-2-host.png: beef's items, 'beef' in the box; lan-2-guest.png: the whole list, the box empty",
    ),
    "reiscreen": dict(
        about="REI's item list over a mod's screen (Farmer's Delight's pot), opened before the player's client joined "
              "(it loads its models first): the screen is laid out again at the join, REI makes its overlay there and "
              "searches again at its reload's end; opened again later, the list is there too",
        mods=["rei", "architectury", "cloth", "fd"], scene="pot",
        steps=['sleep 14; $C screenshot $S/first.png >/dev/null; $C key escape tap >/dev/null; sleep 2; '
               '$C mouse right click >/dev/null; sleep 5; $C state > $S/pot.json; $C screenshot $S/again.png >/dev/null; '
               'python3 -c "import sys; from PIL import Image, ImageFilter; e=lambda p: sum(Image.open(p).convert(\'L\').filter(ImageFilter.FIND_EDGES).crop((930, 90, 1280, 620)).getdata()); '
               'print(\'first\', \'yes\' if e(sys.argv[1]) > 2700000 else \'no\', \'again\', \'yes\' if e(sys.argv[2]) > 2700000 else \'no\')" '
               '$S/first.png $S/again.png > $S/list.txt'],
        checks=[check_log(r"client got the server's recipes \([0-9]{4,}", "the client's join gets the server's recipes (REI reloads there)"),
                check_file("list.txt", r"first yes", "REI's item list beside the pot opened before the join"),
                check_file("pot.json", r'"java_screen":[0-9]+', "the pot's own screen opens again"),
                check_file("list.txt", r"again yes", "REI's item list beside it again")],
        forbid=[r"wrap failed", r"laid out again at the join"],
        look="pot-0.png: REI's item list and search box beside the pot's screen",
    ),
    "join": dict(
        about="Client join hooks (the joinmod fixture): handleLogin's injections and Fabric's JOIN run, with a connection",
        fixtures=["joinmod"], mods=["fd"], scene="none",
        steps=['sleep 2; $C messages > $S/messages.json'],
        checks=[check_file("messages.json", r"horizon_join: handleLogin .* network true", "handleLogin's injection ran, getConnection().getConnection() there"),
                check_file("messages.json", r"horizon_join: fabric JOIN .* connection true", "Fabric's ClientPlayConnectionEvents.JOIN ran"),
                check_file("messages.json", r"horizon_join: fabric JOIN .* waypoints true", "the connection has a waypoint manager (none tracked)"),
                check_file("messages.json", r"horizon_join: hud waypoints \S+ true", "the HUD reads the connection's waypoints")],
        forbid=[r"failed at the client's", r"ClientWaypointManager", r"HUD's \S+ failed"],
    ),
    "join-lan": dict(
        about="A LAN guest's client connection as the host's: its join hooks run, it has a waypoint manager its HUD reads "
              "(a tester's \"ClientWaypointManager.hasWaypoints()\" NPE)",
        fixtures=["joinmod"], mods=["fd"], scene="none", lan=True,
        steps=['sleep 4; $H messages > $S/host.json; $G messages > $S/guest.json'],
        checks=[check_file("host.json", r"horizon_join: hud waypoints \S+ true", "the host's HUD reads its waypoints"),
                check_file("guest.json", r"horizon_join: fabric JOIN Guest connection true waypoints true", "the guest's JOIN: its connection, its waypoints"),
                check_file("guest.json", r"horizon_join: hud waypoints Guest true", "the guest's HUD reads its waypoints")],
        forbid=[r"failed at the client's", r"ClientWaypointManager", r"HUD's \S+ failed"],
    ),
    "join-server": dict(
        about="A dedicated server's player: its client connection has a waypoint manager its HUD reads",
        fixtures=["joinmod"], mods=["fd"], scene="none", server=True,
        steps=['sleep 4; $G messages > $S/player.json'],
        checks=[check_file("player.json", r"horizon_join: fabric JOIN Player connection true waypoints true", "the player's JOIN: its connection, its waypoints"),
                check_file("player.json", r"horizon_join: hud waypoints Player true", "the player's HUD reads its waypoints")],
        forbid=[r"failed at the client's", r"ClientWaypointManager", r"HUD's \S+ failed"],
    ),
    "modsettings": dict(
        about="Options > Mods lists Java mods' settings screens (Mod Menu's API made without Mod Menu); Jade's opens",
        mods=["jade", "fd"], scene="none",
        steps=['$C options mods >/dev/null; sleep 1; $C ui > $S/ui.json',
               '$C click jade >/dev/null; sleep 4; $C state > $S/state.json'],
        checks=[check_file("ui.json", r'"label":"Jade: Settings"', "the Mods page lists Jade"),
                check_file("state.json", r'"screen":"java-screen"', "Jade's settings screen opens")],
        forbid=[r"settings screen failed", r"Mod Menu's API couldn't be made"],
        look="Jade's settings screen",
    ),
    "failmsg": dict(
        about="A mod's failure told in chat: which mod, where, what (the failmod fixture throws in its ticks)",
        fixtures=["failmod"], mods=["fd"], scene="none",
        steps=['sleep 25; $C messages > $S/messages.json'],
        checks=[check_file("messages.json", r"\[Java mods\] Horizon Fail failed in ServerTickEvents.END_SERVER_TICK: IllegalStateException", "the server tick's failure is told"),
                check_file("messages.json", r"\[Java mods\] Horizon Fail failed in ClientTickEvents.END_CLIENT_TICK: NullPointerException", "the client tick's failure is told")],
        look="the two red lines in chat",
    ),
    "keys": dict(
        about="A mod's key binding (the keysmod fixture, G) reaches the mod",
        fixtures=["keysmod"], scene="none",
        steps=['$C key g tap >/dev/null; sleep 1; $C key g tap >/dev/null; sleep 1; $C messages > $S/messages.json'],
        checks=[check_file("messages.json", r"key G pressed by", "the mod saw G")],
    ),
    "entity": dict(
        about="A mod's mob (the crittermod fixture): summoned, ticked, drawn",
        fixtures=["crittermod"], scene="entity",
        steps=['$C state > $S/state.json; $C entities 16 > $S/entities.json'],
        checks=[check_log(r"Summoned new Critter", "the mod's mob is summoned"),
                check_file("entities.json", r'"kind":"java"[^}]*"type":"horizon_critters:critter"|"type":"horizon_critters:critter"[^}]*"kind":"java"',
                           "horizon-ctl sees the critters")],
        look="two critters ahead",
    ),
    "hostile": dict(
        about="A hostile mod's mob (crittermod's biter) hunts a player in survival and bites: Horizon's player is hurt (Natives.hurtPlayer)",
        fixtures=["crittermod"], scene="hostile",
        steps=['sleep 3; $C state > $S/state.json'],
        checks=[check_file("state.json", r'"player":\{[^}]*"health":(1?[0-9])\.', "the player lost health to the bites"),
                check_log(r"Summoned new Biter", "the biter is summoned")],
        look="a biter at the player, the hearts down",
    ),
    "mount": dict(
        about="Riding a mod's mob (crittermod's mount): a use gets on, the movement keys steer it on the mirror, sneaking gets off",
        fixtures=["crittermod"], scene="mount",
        steps=['$C look-at $(cat $S/mount.pos) >/dev/null; sleep 0.3; $C mouse right click >/dev/null; sleep 1.5; $C state > $S/on.json',
               '$C key w down >/dev/null; sleep 0.8; $C key w up >/dev/null; sleep 0.5; $C state > $S/drove.json; $C key f5 tap >/dev/null',
               '$C key f5 tap >/dev/null; $C key f5 tap >/dev/null; $C key shift down >/dev/null; sleep 1; $C key shift up >/dev/null; sleep 1; $C state > $S/off.json; '
               'python3 $R/tools/java_bridge/ride_check.py $S/floor.y $S/on.json $S/drove.json $S/off.json > $S/ride.txt'],
        checks=[check_file("ride.txt", r"(?m)^seated", "a use put the player on the mount"),
                check_file("ride.txt", r"(?m)^drove", "holding forward drove it (the player on it)"),
                check_file("ride.txt", r"(?m)^off", "sneaking got the player off")],
        look="the player on the mount, further east (mount-2.png: third person)",
    ),
    "mountjump": dict(
        about="A mod's horse-style mob (PlayerRideableJumping): the jump key held charges, its release jumps it (the mirror charges as LocalPlayer does)",
        fixtures=["crittermod"], scene="mount",
        steps=['$C look-at $(cat $S/mount.pos) >/dev/null; sleep 0.3; $C mouse right click >/dev/null; sleep 1.5; '
               '$C key space down >/dev/null; sleep 0.4; $C screenshot $S/bar.png >/dev/null; sleep 0.4; $C key space up >/dev/null; '
               'for i in $(seq 15); do $C state; echo; sleep 0.07; done > $S/jump.jsonl; '
               'python3 -c "import json,sys; f=float(open(sys.argv[1]).read()); '
               'ys=[json.loads(l)[\'player\'][\'position\'][1] for l in open(sys.argv[2]) if l.strip()]; '
               'print(\'rose\', round(max(ys)-f,2))" $S/floor.y $S/jump.jsonl > $S/rose.txt'],
        checks=[check_log(r"Mount jumps ([6-9][0-9]|100)\b", "released after ~0.8 s: a strong jump (power 60-100)"),
                check_file("rose.txt", r"rose (1\.[2-9]|[2-9])", "the player went up with the mount (over 1.2 above the floor)")],
        look="the jump bar over the hotbar (bar.png in the scratch folder)",
    ),
    "neighbours": dict(
        about="A mod's mob sees Horizon's own (HorizonNeighbours): crittermod's hunter finds Horizon's sheep and bites it; Horizon's sheep is hurt",
        fixtures=["crittermod"], scene="neighbours",
        steps=['$C entities 16 > $S/before.json; $C command "summon horizon_critters:hunter $(cat $S/hunter.pos)" >/dev/null; '
               'sleep 1; $C screenshot $S/chase.png >/dev/null; sleep 6; $C entities 16 > $S/after.json; '
               'python3 "$R/tools/java_bridge/sheep_check.py" $S/before.json $S/after.json > $S/sheep.txt'],
        checks=[check_file("sheep.txt", r"(?m)^there", "Horizon's sheep stood there at first (8 health)"),
                check_file("sheep.txt", r"(?m)^hurt", "then took the hunter's bites (less health, or killed by them)"),
                check_log(r"Summoned new Hunter", "the hunter is summoned")],
        look="the hunter at the sheep",
    ),
    "golem": dict(
        about="Horizon's mobs see mods' mobs (HorizonNeighbours.seen): Horizon's iron golem hunts crittermod's biter (an Enemy) and kills it on the mirror",
        fixtures=["crittermod"], scene="golem",
        steps=['$C command "summon horizon_critters:biter $(cat $S/critter.pos)" >/dev/null; sleep 1; $C entities 16 > $S/before.json; '
               'sleep 2; $C screenshot $S/fight.png >/dev/null; sleep 10; $C entities 16 > $S/after.json; '
               'python3 "$R/tools/java_bridge/gone_check.py" horizon_critters:biter $S/before.json $S/after.json minecraft:iron_golem > $S/gone.txt'],
        checks=[check_file("gone.txt", r"(?m)^there", "the biter stood in the pen"),
                check_file("gone.txt", r"(?m)^gone", "Horizon's golem killed it"),
                check_file("gone.txt", r"(?m)^kept", "the golem is still there")],
        look="the golem at the biter",
    ),
    "tacz": dict(
        about="TACZ's guns fire: an AK-47 held down (its shoot key on the left button) kills a zombie at night; the ammo count drops (generic: mouse-bound key mappings, mixins' initializers, players' tick injections)",
        mods=["tacz", "fabric-api", "playeranim", "fcap"], scene="range",
        steps=['$C command "item replace entity @s hotbar.0 with tacz:modern_kinetic_gun[minecraft:custom_data={GunId:\\"tacz:ak47\\",GunFireMode:\\"AUTO\\",GunCurrentAmmoCount:30,HasBulletInBarrel:1b}]" >/dev/null; $C select 1 >/dev/null; sleep 4; $C command "time set midnight" >/dev/null; '
               '$C command "summon minecraft:zombie $(cat $S/critter.pos) {NoAI:1b,PersistenceRequired:1b}" >/dev/null; sleep 1.5; $C entities 16 > $S/before.json; '
               'at=$(python3 -c "import json,sys; b=[e[\'pos\'] for e in json.load(open(sys.argv[1])).get(\'entities\', []) if e.get(\'type\') == \'minecraft:zombie\']; print(\' \'.join(str(v + d) for v, d in zip(b[0], (0, 1.2, 0))) if b else \'\')" $S/before.json); '
               '$C look-at $at >/dev/null; sleep 0.3; $C mouse left down >/dev/null; sleep 1.35; $C mouse left up >/dev/null; sleep 1; '
               '$C entities 16 > $S/after.json; python3 "$R/tools/java_bridge/gone_check.py" minecraft:zombie $S/before.json $S/after.json > $S/gone.txt; '
               '$C command "data get entity @s SelectedItem" > $S/item.json; $C screenshot $S/shot.png >/dev/null'],
        checks=[check_file("gone.txt", r"(?m)^there", "the zombie stood in the pen"),
                check_file("gone.txt", r"(?m)^gone", "the AK's bullets killed it"),
                check_file("item.json", r"GunCurrentAmmoCount: ?(1?[0-9]|2[0-8])\b", "the gun's magazine emptied (its count back in Horizon's slot)"),
                check_log(r"Mixin fields: com/tacz/guns/mixin/client/LocalPlayerMixin's initializer ran", "TACZ's client player state was made")],
        forbid=[r"A mod failed in ClientTickEvents", r"Error executing task on null", r"getTickTimesNanos"],
        look="the AK in first person; the zombie gone",
    ),
    "tacz-model": dict(
        about="TACZ's gun models: the AK-47 in first person (deferred custom geometry, its gun pack's texture sent as an image) and in third person (a mod-drawn item model resolved by the client's own resolver)",
        mods=["tacz", "fabric-api", "playeranim", "fcap"], scene="range",
        steps=['$C command "item replace entity @s hotbar.0 with tacz:modern_kinetic_gun[minecraft:custom_data={GunId:\\"tacz:ak47\\",GunFireMode:\\"AUTO\\",GunCurrentAmmoCount:30,HasBulletInBarrel:1b}]" >/dev/null; $C select 1 >/dev/null; sleep 4; $C screenshot $S/first.png >/dev/null',
               '$C key f5 tap >/dev/null; sleep 1; $C key f5 tap >/dev/null; sleep 2; $C screenshot $S/third.png >/dev/null; $C key f5 tap >/dev/null'],
        checks=[check_log(r"tacz:textures/gun/uv/ak47\.png \(256x256\) is sent to the players' games", "the AK's texture went to the game"),
                check_log(r"tacz:modern_kinetic_gun's item model is drawn by its mod", "the gun's item model is the mod's")],
        forbid=[r"custom geometry couldn't be read", r"A mod failed"],
        look="first.png: the AK-47 held in first person, arms on it; third.png: the player facing the camera, the AK in the right hand",
    ),
    "arrows": dict(
        about="Horizon's projectiles hit mods' mobs: the player's bow shots kill crittermod's biter on the mirror (HorizonNeighbours.hurtByProjectile)",
        fixtures=["crittermod"], scene="range",
        steps=['$C command "item replace entity @s hotbar.0 with minecraft:bow" >/dev/null; $C select 1 >/dev/null; '
               '$C command "summon horizon_critters:biter $(cat $S/critter.pos)" >/dev/null; sleep 1.5; $C entities 16 > $S/before.json; '
               'for i in 1 2 3 4; do at=$(python3 -c "import json,sys; b=[e[\'pos\'] for e in json.load(open(sys.argv[1])).get(\'entities\', []) if e.get(\'type\') == \'horizon_critters:biter\']; print(\' \'.join(str(v + d) for v, d in zip(b[0], (0, 0.45, 0))) if b else \'\')" $S/$([ $i = 1 ] && echo before || echo now).json); '
               '[ -z "$at" ] && break; $C look-at $at >/dev/null; $C use-item --hold 1.2 >/dev/null; sleep 1; '
               '[ $i = 1 ] && $C screenshot $S/shot.png >/dev/null; $C entities 16 > $S/now.json; '
               'grep -q horizon_critters:biter $S/now.json || break; done; sleep 1; $C entities 16 > $S/after.json; '
               'python3 "$R/tools/java_bridge/gone_check.py" horizon_critters:biter $S/before.json $S/after.json > $S/gone.txt'],
        checks=[check_file("gone.txt", r"(?m)^there", "the biter stood in the pen"),
                check_file("gone.txt", r"(?m)^gone", "the player's arrows killed it")],
        forbid=[r"projectile's hit on a mod entity failed", r"no minecraft:arrow for a projectile"],
        look="an arrow in flight at (or stuck beside) the biter",
    ),
    "spit": dict(
        about="A mod's own projectile hits players: crittermod's spitter throws Pebbles (2 damage each) at the player in survival",
        fixtures=["crittermod"], scene="range",
        steps=['$C command "gamerule natural_health_regeneration false" >/dev/null; $C command "gamemode survival" >/dev/null; '
               '$C command "summon horizon_critters:spitter $(cat $S/critter.pos)" >/dev/null; sleep 3; $C screenshot $S/spit.png >/dev/null; sleep 3; '
               '$C status > $S/status.json; python3 -c "import json,sys; h=json.load(open(sys.argv[1]))[\'player\'][\'health\']; print(\'hurt\' if h < 20 else \'whole\', h)" $S/status.json > $S/hurt.txt'],
        checks=[check_file("hurt.txt", r"^hurt", "the Pebbles took some of the player's health")],
        look="the spitter facing the player, hearts below 10",
    ),
    "spitsheep": dict(
        about="A mod's own projectile hits Horizon's mobs: crittermod's spitter (the player in creative: not its target) throws Pebbles at Horizon's sheep until it dies",
        fixtures=["crittermod"], scene="neighbours",
        steps=['$C entities 16 > $S/before.json; $C command "summon horizon_critters:spitter $(cat $S/hunter.pos)" >/dev/null; sleep 4; '
               '$C screenshot $S/spit.png >/dev/null; sleep 14; $C entities 16 > $S/after.json; '
               'python3 "$R/tools/java_bridge/gone_check.py" minecraft:sheep $S/before.json $S/after.json horizon_critters:spitter > $S/gone.txt'],
        checks=[check_file("gone.txt", r"(?m)^there", "Horizon's sheep stood there"),
                check_file("gone.txt", r"(?m)^gone", "the Pebbles killed it"),
                check_file("gone.txt", r"(?m)^kept", "the spitter is still there")],
        look="the spitter turned to the sheep",
    ),
    "strikeback": dict(
        about="Horizon's mob strikes back at a mod's mob that hurts it: crittermod's hunter (no Enemy: golems leave it be) bites Horizon's iron golem, which kills it",
        fixtures=["crittermod"], scene="golem",
        steps=['$C command "summon horizon_critters:hunter $(cat $S/critter.pos)" >/dev/null; sleep 1; $C entities 16 > $S/before.json; '
               'sleep 2; $C screenshot $S/fight.png >/dev/null; sleep 12; $C entities 16 > $S/after.json; '
               'python3 "$R/tools/java_bridge/gone_check.py" horizon_critters:hunter $S/before.json $S/after.json minecraft:iron_golem > $S/gone.txt'],
        checks=[check_file("gone.txt", r"(?m)^there", "the hunter stood in the pen"),
                check_file("gone.txt", r"(?m)^gone", "the golem it bit killed it"),
                check_file("gone.txt", r"(?m)^kept", "the golem is still there")],
        look="the golem at the hunter",
    ),
    "mounthud": dict(
        about="Riding a mod's mob in survival: its hearts on the HUD where the food bar was (as a vanilla mount's)",
        fixtures=["crittermod"], scene="mount",
        steps=['$C command "gamemode survival" >/dev/null; sleep 1; $C screenshot $S/walk.png >/dev/null; '
               '$C look-at $(cat $S/mount.pos) >/dev/null; sleep 0.3; $C mouse right click >/dev/null; sleep 2; '
               '$C screenshot $S/ride.png >/dev/null; $C status > $S/status.json; '
               'python3 "$R/tools/java_bridge/mount_hearts.py" $S/walk.png $S/ride.png > $S/hearts.txt; '
               'python3 -c "import sys; a,b=[int(l.split()[-1]) for l in open(sys.argv[1])]; print(\'hearts\' if b > a + 100 else \'none\', a, b)" $S/hearts.txt >> $S/hearts.txt'],
        checks=[check_file("status.json", r'"riding_java":\[true,\[20\.0,20\.0\]\]', "the game knows its mount's health (20 of 20)"),
                check_file("hearts.txt", r"(?m)^hearts", "its hearts drawn on the right, where the food bar was")],
        look="ten orange mount hearts right of the hotbar (ride.png in the scratch folder)",
    ),
    "gecko": dict(
        about="GeckoLib (geckomod fixture, built against the GeckoLib jar): a mob's GeckoLib model drawn, its looping animation moving it",
        fixtures=["geckomod"], mods=["geckolib"], scene="entity", env={"ENTITY": "horizon_gecko:bobber"},
        steps=['sleep 2; $C screenshot $S/a.png >/dev/null; sleep 0.7; $C screenshot $S/b.png >/dev/null; '
               '$C entities 8 > $S/entities.json; python3 "$R/tools/java_bridge/changed.py" $S/a.png $S/b.png > $S/moved.txt'],
        checks=[check_file("entities.json", r'"type":"horizon_gecko:bobber"', "horizon-ctl sees the bobbers"),
                check_file("moved.txt", r"^changed", "the animation moves them between two shots")],
        forbid=[r"bobber.*(failed|couldn't)"],
        look="two bobbers ahead, their tops turned",
    ),
    "worldgen": dict(
        about="Biomes O' Plenty's biomes: /locate finds one",
        mods=["bop", "terrablender", "glitchcore"], scene="none",
        steps=['$C command "locate biome biomesoplenty:redwood_forest" >/dev/null; sleep 6; $C messages > $S/messages.json'],
        checks=[check_file("messages.json", r"redwood_forest[^\"]*is at|nearest[^\"]*redwood", "a redwood forest is found")],
        forbid=[r"features failed"],
    ),
    "veinminer": dict(
        about="Veinminer: sneak and dig once, the whole 7-ore iron vein breaks and drops",
        mods=["veinminer", "kotlin"], scene="vein",
        steps=['$C key shift down >/dev/null; sleep 0.3; $C mouse left down >/dev/null; sleep 1; '
               '$C mouse left up >/dev/null; sleep 3; $C key shift up >/dev/null; '
               '$C blocks $(cat $S/vein.pos) > $S/vein.json; $C state > $S/state.json; '
               '$C entities 8 minecraft:item > $S/drops.json'],
        checks=[check_file("vein.json", r'"counts":\{\}', "every ore of the vein is gone"),
                check_json("state.json", ["held", "damage"], 7, "the pickaxe wore once per ore"),
                check_file("drops.json", r"minecraft:raw_iron", "raw iron dropped")],
        look="the vein gone, raw iron on the ground",
    ),
    "harvest": dict(
        about="Right Click Harvest: a ripe wheat right-clicked is replanted and drops",
        mods=["rch", "jamlib"], scene="harvest",
        steps=['$C mouse right click >/dev/null; sleep 2; $C block $(cat $S/crop.pos) > $S/crop.json; '
               '{ $C entities 6 minecraft:item; $C inventory; } > $S/drops.json'],
        checks=[check_json("crop.json", ["state"], "minecraft:wheat[age=0]", "the wheat is replanted"),
                check_file("drops.json", r"minecraft:wheat\b", "wheat dropped (on the ground or picked up)")],
    ),
    "carry": dict(
        about="Carry On: sneak-right-click picks a cow up, a right-click on the ground puts it down",
        mods=["carryon"], scene="carry",
        # The carry key is Carry On's own key binding (shift): its client sends it once the
        # game knows the mods' keys, a few seconds after joining.
        steps=['sleep 5; $C key shift down >/dev/null; sleep 1.5; $C mouse right click >/dev/null; sleep 1; '
               '$C key shift up >/dev/null; sleep 1; $C entities 8 minecraft:cow > $S/carried.json',
               '$C turn 180 0 >/dev/null; $C look 90 50 >/dev/null; sleep 0.5; $C mouse right click >/dev/null; '
               'sleep 2; $C entities 8 minecraft:cow > $S/placed.json'],
        checks=[check_file("carried.json", r'"entities":\[\]', "the cow is carried (gone from the world)"),
                check_file("placed.json", r"minecraft:cow", "the cow is put down again")],
        look="the cow put down in front",
    ),
    "seasons": dict(
        about="Serene Seasons: wheat grows in summer, not in winter (its random-tick patch)",
        mods=["seasons", "glitchcore"], scene="seasons",
        # Out of season, crops don't grow at all (the default lets one tick in six through).
        config={"sereneseasons/fertility.toml": "[general]\n\tseasonal_crops = true\n\tout_of_season_crop_behavior = 1\n"},
        steps=['$C command "season set mid_summer" >/dev/null; $C command "gamerule random_tick_speed 1000" >/dev/null; '
               'sleep 6; $C command "gamerule random_tick_speed 0" >/dev/null; $C block $(cat $S/crop.pos) > $S/summer.json',
               '$C command "setblock $(cat $S/crop.pos) minecraft:wheat[age=0]" >/dev/null; '
               '$C command "season set early_winter" >/dev/null; $C command "gamerule random_tick_speed 1000" >/dev/null; '
               'sleep 6; $C command "gamerule random_tick_speed 3" >/dev/null; $C block $(cat $S/crop.pos) > $S/winter.json'],
        checks=[check_file("summer.json", r'wheat\[age=[1-7]\]', "the wheat grew in summer"),
                check_json("winter.json", ["state"], "minecraft:wheat[age=0]", "the wheat didn't grow in winter"),
                check_log(r"Random ticks of \d+ vanilla blocks go to the mods", "crop ticks go to the mods")],
    ),
    "orbs": dict(
        about="Clumps: experience orbs' pickup goes through its playerTouch",
        mods=["clumps"], scene="none",
        steps=['$C command "gamemode survival" >/dev/null; for i in 1 2 3 4 5 6; do '
               '$C command "summon experience_orb ~ ~0.5 ~ {Value:7}" >/dev/null; done; sleep 2; $C state > $S/state.json'],
        checks=[check_json("state.json", ["player", "xp", "total"], 42, "all six orbs' points (42)"),
                check_log(r"experience_orbs(, \w+)* go to the mods", "orb pickup goes to the mods")],
    ),
    "templates": dict(
        about="A mod's override of vanilla's structure templates (The Sift's ancient city centres) in the city Horizon generates",
        mods=["sift", "fabric-api", "geckolib"], scene="none",
        steps=['$C command "gamemode spectator" >/dev/null; $C command "locate structure minecraft:ancient_city" >/dev/null; '
               'sleep 5; $C messages > $S/locate.json',
               # "… is at [X, ~, Z]": the start's chunk, where the city centre is (start_height -27).
               'xz=$(grep -o "at \\[-\\?[0-9]*, ~, -\\?[0-9]*\\]" $S/locate.json | tail -1 | tr -d "at[]~ " | tr "," " "); '
               'set -- $xz; $C command "tp @s $1 -20 $2" >/dev/null; $C wait-for loaded --timeout 400 >/dev/null; sleep 60; '
               '$C find-block the_sift:sonorous_deepslate 64 > $S/found.json; $C look-at $1 -40 $(($2 + 20)) >/dev/null'],
        checks=[check_file("locate.json", r"ancient_city is at", "/locate found an ancient city"),
                check_file("found.json", r"\"count\":([89]|[1-9][0-9]+)\b", "the city centre is The Sift's: its 8 sonorous deepslate")],
        forbid=[r"panicked"],
        look="the ancient city's centre, The Sift's frame",
        timeout=900,
    ),
    "dimensions": dict(
        about="Mods' dimensions are levels: /execute in The Sift's, generated by the mods; Every Mining "
              "Dimension's teleporter block (mod code) to its flat mining level",
        mods=["sift", "emd", "fabric-api", "geckolib"], scene="none",
        steps=['$C command "execute in the_sift:the_sift run tp @s 0.5 140 0.5" >/dev/null; sleep 45; '
               '$C state > $S/sift.json; $C command "tp @s ~ ~ ~ 30 20" >/dev/null; sleep 5',
               '$C command "execute in minecraft:overworld run tp @s 0.5 100 0.5" >/dev/null; sleep 30; '
               '"$R/tools/java_bridge/around.sh" \'q "setblock $((X+2)) $Y $Z everyminingdimension:teleport_overworld_mining_dimension"\' '
               '\'$C look-at-block $((X+2)) $Y $Z >/dev/null\' \'$C mouse right click >/dev/null\' \'sleep 30\'; $C state > $S/emd.json'],
        checks=[check_log(r"the dimension the_sift:the_sift is a level", "The Sift's dimension is a level"),
                check_json("sift.json", ["dimension"], "the_sift:the_sift", "/execute in took the player there"),
                check_log(r"the_sift:the_sift generates through the mods", "the mods generate it"),
                check_json("emd.json", ["dimension"], "everyminingdimension:overworld_mining_dimension",
                           "the teleporter block took the player to the mining level")],
        forbid=[r"panicked", r"ConcurrentModificationException", r"already been modified"],
        look="the mining level's flat grass and the return teleporter",
        timeout=900,
    ),
    "rift": dict(
        about="The Sift's rift (a mod entity touched; the mod finds its main portal in The Sift, a level "
              "Horizon hasn't loaded: stand-in chunks) takes the player into The Sift",
        mods=["sift", "fabric-api", "geckolib"], scene="none",
        steps=['"$R/tools/java_bridge/around.sh" \'q "give @s the_sift:sift_rift"\' \'q "tp @s ~ ~ ~ -90 0"\' '
               '\'$C look-at-block $((X+3)) $((Y-1)) $Z >/dev/null\' \'$C mouse right click >/dev/null\' \'sleep 3\' '
               '\'q "tp @s ~ ~ ~ -90 0"\' \'$C move forward 2 >/dev/null\' \'sleep 40\'; $C state > $S/state.json'],
        checks=[check_json("state.json", ["dimension"], "the_sift:the_sift", "the rift took the player into The Sift"),
                check_log(r"Main Sift portal ready", "the mod placed its main portal in The Sift")],
        forbid=[r"Rift transfer failed", r"panicked"],
        look="The Sift's main portal (the player stands in it)",
        timeout=900,
    ),
    "newstructures": dict(
        about="A structures-only mod (Towns and Towers) in an overworld Horizon generates: /locate finds its "
              "village (Horizon's generator knows its structures and sets), the village is there (its bell)",
        mods=["tt", "cristellib", "cloth", "fabric-api"], scene="none",
        steps=['$C command "gamemode spectator" >/dev/null; $C command "locate structure #towns_and_towers:town" >/dev/null; '
               'sleep 10; $C messages > $S/locate.json',
               # "… is at [X, ~, Z]": the village's start chunk, its town centre (with its bell) near.
               'xz=$(grep -o "at \\[-\\?[0-9]*, ~, -\\?[0-9]*\\]" $S/locate.json | tail -1 | tr -d "at[]~ " | tr "," " "); '
               'set -- $xz; $C command "tp @s $1 130 $2" >/dev/null; $C wait-for loaded --timeout 400 >/dev/null; sleep 45; '
               '$C find-block bell 64 > $S/found.json; '
               'b=$(grep -o "\\"pos\\":\\[[^]]*" $S/found.json | head -1 | tr -d "\\"pos:[" | tr "," " "); '
               '[ -n "$b" ] && set -- $b && $C command "tp @s $(($1 + 12)) $(($2 + 10)) $(($3 + 12))" >/dev/null '
               '&& sleep 3 && $C look-at $1 $2 $3 >/dev/null; sleep 5'],
        checks=[check_log(r"overworld generates as vanilla's \(Horizon's generator\)", "the overworld is Horizon's generator"),
                check_file("locate.json", r'locate\.structure\.success","with":\[\{"text":"#towns_and_towers:town '
                           r'\(towns_and_towers:village_', "/locate found a Towns and Towers village"),
                check_file("found.json", r"\"count\":[1-9]", "the village is generated (its bell)")],
        forbid=[r"panicked", r"worldgen: proto failed"],
        look="a Towns and Towers village (Romanian birch, Swiss meadow...), its bell in view",
        timeout=900,
    ),
    "structuretypes": dict(
        about="A structures-only mod whose structures are of its own types (Friends & Foes' illusioner shack): "
              "Horizon's generator can't make them, so the mods generate the overworld; the shack is there",
        mods=["faf", "resourcefullib", "fabric-api"], scene="none",
        steps=['$C command "gamemode spectator" >/dev/null; $C command "locate structure friendsandfoes:illusioner_shack" >/dev/null; '
               'sleep 30; $C messages > $S/locate.json',
               'xz=$(grep -o "at \\[-\\?[0-9]*, ~, -\\?[0-9]*\\]" $S/locate.json | tail -1 | tr -d "at[]~ " | tr "," " "); '
               'set -- $xz; $C command "tp @s $1 140 $2" >/dev/null; $C wait-for loaded --timeout 400 >/dev/null; sleep 60; '
               # The shack's brewing stand (its own processor fills it).
               '$C find-block brewing_stand 64 > $S/found.json; '
               'b=$(grep -o "\\"pos\\":\\[[^]]*" $S/found.json | head -1 | tr -d "\\"pos:[" | tr "," " "); '
               '[ -n "$b" ] && set -- $b && $C command "tp @s $(($1 + 10)) $(($2 + 7)) $(($3 + 9))" >/dev/null '
               '&& sleep 3 && $C look-at $1 $2 $3 >/dev/null; sleep 5'],
        checks=[check_log(r"has friendsandfoes:illusioner_shack \(friendsandfoes:illusioner_shack_structure\), "
                          r"a structure Horizon's generator can't make", "Horizon says it can't make the shack"),
                check_log(r"overworld generates through the mods \(friendsandfoes\)", "so the mods generate the overworld"),
                check_file("locate.json", r'locate\.structure\.success","with":\[\{"text":"friendsandfoes:illusioner_shack"',
                           "/locate found a shack"),
                check_file("found.json", r"\"count\":[1-9]", "the shack is generated (its brewing stand)")],
        forbid=[r"panicked", r"Java mods' worldgen: chunk -?\d+ -?\d+ failed"],
        look="Friends & Foes' illusioner shack (a dark oak hut, a scarecrow)",
        timeout=900,
    ),
    "preset": dict(
        about="A mod's world type (One Block) picked in Create New World: its block, its gameplay",
        mods=["oneblock", "codxlib"], scene="none", env={"WORLD": "none"},
        steps=['"$R/tools/java_bridge/new_world.sh" "One Block"',
               '$C block 0 64 0 > $S/block.json; $C look-at-block 0 64 0 >/dev/null; sleep 2; $C messages > $S/messages.json'],
        checks=[check_file("block.json", r"grass_block", "the one block at 0 64 0"),
                check_file("messages.json", r"Your island is at", "One Block's own join message")],
        forbid=[r"worldgen: proto failed"],
        look="the grass block below, nothing else",
    ),
    "skyblock": dict(
        about="Skyblock Infinite: a void world; its load function places the island and moves the player onto it",
        mods=["skyblock"], scene="none",
        steps=['sleep 25; $C state > $S/state.json'],
        checks=[check_log(r"generates through the mods \(mr_skyblock_infinite\)", "the overworld is the pack's void"),
                check_log(r"the world spawn moves to 6, 64, 6", "the island function set the spawn"),
                check_file("state.json", r'"block_pos":\[[0-9]+,6[0-9],[0-9]+\]', "the player stands on the island")],
        forbid=[r"fell out of the world", r"functions stopped"],
        look="the player on a small island in the void",
    ),
    "oneblock": dict(
        about="Simply OneBlock: the first player is placed after its load function, on its block",
        mods=["simplyoneblock"], scene="none",
        steps=['sleep 25; $C state > $S/state.json'],
        checks=[check_log(r"the world spawn moves to 0, 64, 0", "the load function set the spawn"),
                check_file("state.json", r'"block_pos":\[-?[01],6[45],-?[01]\]', "the player stands on the block")],
        forbid=[r"fell out of the world", r"functions stopped"],
    ),
    "attack": dict(
        about="A mod patching Player.attack's inside (mixinmod: ten times the damage): the attack is the mods'",
        fixtures=["mixinmod"], scene="fight",
        steps=['$C mouse left click >/dev/null; sleep 1.5; $C entities 6 minecraft:cow > $S/cow.json; '
               '$C look-at-entity $(cat $S/pig.id) >/dev/null; sleep 1; $C mouse left click >/dev/null; sleep 1; '
               '$C entities 6 minecraft:pig > $S/pig.json'],
        checks=[check_file("cow.json", r'"entities":\[\]', "the cow died to one hit (60 damage)"),
                check_file("pig.json", r'"health":10\.0', "the pig was spared (Player.attack's head cancels it)"),
                check_log(r"Players' attacks go to the mods", "attacks go to the mods")],
    ),
    "bettercombat": dict(
        about="Better Combat: the click's client half (Minecraft.startAttack's head) is its own attack; the cow takes the sword's hit",
        mods=["bettercombat", "cloth", "playeranim"], scene="fight", env={"HORIZON_JAVA_DEBUG": "combat"},
        # Within the iron sword's own reach (Better Combat's 2.5 blocks; the scene's cow is 2.57 away).
        # The hit lands after the upswing: past 1.5 s on a loaded 2-core box (merge batch 12).
        steps=['$C command "tp @s ^ ^ ^0.4" >/dev/null; sleep 1; $C mouse left click >/dev/null; sleep 4; '
               '$C entities 6 minecraft:cow > $S/cow.json'],
        checks=[check_file("cow.json", r'"health":[0-4]\.', "the cow lost 6 health"),
                check_log(r"taken by a mod's client attack", "Better Combat's client took the click")],
    ),
    "breaksound": dict(
        about="A routed break (mixinmod patches destroyBlock's inside): the breaker hears its dirt and its log break once, "
              "not once from its own game and again from the mods' level event",
        fixtures=["mixinmod"], scene="dig",
        steps=['heard(){ $C status | python3 -c "import json,sys;h=json.load(sys.stdin).get(\'heard\',{});'
               'print(h.get(\'minecraft:block.$1.break\',0))"; }; '
               '$C mouse left down >/dev/null; sleep 2; $C mouse left up >/dev/null; sleep 2; heard gravel > $S/dirt.txt; '
               'read a b c x y z < $S/dig.pos; $C look-at-block $x $y $z >/dev/null; sleep 0.5; '
               '$C mouse left down >/dev/null; sleep 5; $C mouse left up >/dev/null; sleep 2; heard wood > $S/log.txt; '
               '$C blocks $(cat $S/dig.pos) > $S/blocks.json'],
        checks=[check_file("dirt.txt", r"^1$", "the dirt's break sound played once"),
                check_file("log.txt", r"^1$", "the log's break sound played once"),
                check_file("blocks.json", r'"counts":\{\}', "both broke"),
                check_log(r"Breaking minecraft:dirt goes to the mods", "the break was the mods'")],
        look="the dirt and the log gone",
    ),
    "chunky": dict(
        about="Chunky pregenerates through the mirror's chunk system: its tickets and chunk futures load Horizon's "
              "chunks (81 around 2000,2000, saved full); a second run reads Horizon's region files and loads none",
        mods=["chunky", "fabric-api"], scene="none", env={"HORIZON_JAVA_DEBUG": "chunks"},
        steps=['$C command "chunky center 2000 2000" >/dev/null; $C command "chunky radius 64" >/dev/null; '
               '$C command "chunky start" >/dev/null; '
               'for i in $(seq 90); do sleep 2; grep -q "Chunky\\] Task finished" $S/home/logs/latest.log && break; done; '
               'sleep 8; python3 "$R/tools/java_bridge/region_check.py" $S/saves/W/dimensions/minecraft/overworld 121 121 129 129 > $S/region.txt; '
               '$C command "chunky start" >/dev/null; sleep 10; grep -c "chunks: .* asked for" $S/home/logs/latest.log > $S/asked.txt'],
        checks=[check_log(r"\[Chunky\] Task finished for minecraft:overworld\. Processed: 81 chunks \(100\.00%\)", "Chunky finished its 81 chunks"),
                check_file("region.txt", r"saved 81 full 81 of 81", "all 81 are saved, full, in Horizon's region files"),
                check_file("asked.txt", r"^81$", "the second run loaded none (they were generated)"),
                check_log(r"chunk tickets keep 0 chunk", "its tickets were let go")],
        forbid=[r"given up after", r"a chunk's load"],
        timeout=600,
    ),
    "ride": dict(
        about="Entity.startRiding's head (mixinmod: no minecarts): the cart refuses, the boat takes the player",
        fixtures=["mixinmod"], scene="ride",
        steps=['$C look-at $(cat $S/cart.pos) >/dev/null; sleep 0.5; $C mouse right click >/dev/null; sleep 1; $C status > $S/cart.json',
               '$C look-at $(cat $S/boat.pos) >/dev/null; sleep 0.5; $C mouse right click >/dev/null; sleep 1; $C status > $S/boat.json'],
        checks=[check_json("cart.json", ["riding"], None, "not in the minecart"),
                check_file("boat.json", r'"riding":\{|"riding":[0-9]', "in the boat")],
    ),
    "dismount": dict(
        about="Entity.removeVehicle's head (mixinmod: not with a stick in hand): sneaking keeps the player in the boat, then lets go",
        fixtures=["mixinmod"], scene="ride",
        steps=['$C look-at $(cat $S/boat.pos) >/dev/null; sleep 0.5; $C mouse right click >/dev/null; sleep 1; $C status > $S/in.json; '
               '$C command "give @s minecraft:stick" >/dev/null; sleep 0.5; '
               '$C key shift down >/dev/null; sleep 0.5; $C key shift up >/dev/null; sleep 1; $C status > $S/kept.json',
               '$C command "clear @s minecraft:stick" >/dev/null; sleep 0.5; '
               '$C key shift down >/dev/null; sleep 0.5; $C key shift up >/dev/null; sleep 1; $C status > $S/off.json'],
        checks=[check_file("in.json", r'"riding":\{|"riding":[0-9]', "in the boat"),
                check_file("kept.json", r'"riding":\{|"riding":[0-9]', "still in the boat with a stick"),
                check_json("off.json", ["riding"], None, "off the boat with empty hands")],
    ),
    "sweep": dict(
        about="Better Combat's sweep: one swing of an iron sword hurts more than one of three zombies in a row",
        mods=["bettercombat", "cloth", "playeranim"], scene="sweep", env={"HORIZON_JAVA_DEBUG": "combat"},
        steps=['sleep 1; $C mouse left click >/dev/null; sleep 2; $C entities 6 minecraft:zombie | '
               'python3 -c "import json,sys;print(sum(e[\'health\']<20 for e in json.load(sys.stdin)[\'entities\']))" > $S/hurt.txt'],
        checks=[check_file("hurt.txt", r"^[23]$", "two or three zombies hurt")],
    ),
    "fall": dict(
        about="LivingEntity.causeFallDamage (mixinmod: a feather cancels it, a slime ball halves the distance): 10-block falls",
        fixtures=["mixinmod"], scene="none",
        steps=['$C command "gamemode survival" >/dev/null; $C command "gamerule natural_health_regeneration false" >/dev/null; sleep 0.5; '
               '$C command "item replace entity @s weapon.mainhand with minecraft:feather" >/dev/null; sleep 0.5; $C command "tp @s ~ ~10 ~" >/dev/null; sleep 3; $C status > $S/feather.json',
               '$C command "item replace entity @s weapon.mainhand with minecraft:slime_ball" >/dev/null; sleep 0.5; $C command "tp @s ~ ~10 ~" >/dev/null; sleep 3; $C status > $S/slime.json',
               '$C command "item replace entity @s weapon.mainhand with minecraft:air" >/dev/null; sleep 0.5; $C command "tp @s ~ ~10 ~" >/dev/null; sleep 3; $C status > $S/hand.json'],
        checks=[check_json("feather.json", ["player", "health"], 20.0, "no damage with a feather"),
                check_json("slime.json", ["player", "health"], 18.0, "2 with a slime ball (5 blocks: 5 - 3)"),
                check_json("hand.json", ["player", "health"], 11.0, "7 more with empty hands (10 - 3)"),
                check_log(r"Fall damage goes to the mods", "fall damage routed")],
    ),
    "effects": dict(
        about="LivingEntity.addEffect's head (mixinmod: players are immune to poison): /effect gives speed, not poison",
        fixtures=["mixinmod"], scene="none",
        steps=['$C command "gamemode survival" >/dev/null; $C command "effect give @s minecraft:poison 30" >/dev/null; '
               '$C command "effect give @s minecraft:speed 30" >/dev/null; sleep 1.5; '
               '$C state | python3 -c "import json,sys;print(sorted(e[\'id\'] for e in json.load(sys.stdin)[\'player\'][\'effects\']))" > $S/effects.txt'],
        checks=[check_file("effects.txt", r"^\['speed'\]$", "speed, no poison")],
    ),
    "effects2": dict(
        about="Effects Horizon adds itself ask LivingEntity.addEffect's head (mixinmod refuses haste and Bad Omen): a beacon, an ominous bottle",
        fixtures=["mixinmod"], scene="beacon",
        steps=['$C command "setblock $(cat $S/beacon.pos) minecraft:beacon{primary_effect:\\"minecraft:haste\\"}" >/dev/null; sleep 6; '
               '$C state | python3 -c "import json,sys;print(sorted(e[\'id\'] for e in json.load(sys.stdin)[\'player\'][\'effects\']))" > $S/haste.txt',
               '$C command "setblock $(cat $S/beacon.pos) minecraft:beacon{primary_effect:\\"minecraft:speed\\"}" >/dev/null; sleep 6; '
               '$C state | python3 -c "import json,sys;print(sorted(e[\'id\'] for e in json.load(sys.stdin)[\'player\'][\'effects\']))" > $S/speed.txt',
               '$C command "item replace entity @s weapon.mainhand with minecraft:ominous_bottle" >/dev/null; $C look 0 -90 >/dev/null; sleep 0.5; '
               '$C mouse right down >/dev/null; sleep 2.5; $C mouse right up >/dev/null; sleep 1; '
               '$C state | python3 -c "import json,sys;print(sorted(e[\'id\'] for e in json.load(sys.stdin)[\'player\'][\'effects\']))" > $S/omen.txt; $C inventory > $S/bottle.json'],
        checks=[check_file("haste.txt", r"^\[\]$", "no haste from the beacon"),
                check_file("speed.txt", r"'speed'", "speed from the beacon (the control)"),
                check_file("omen.txt", r"\A(?!.*bad_omen)", "no Bad Omen from the bottle"),
                check_file("bottle.json", r'"held":null|"held":\{"count":0', "the bottle drunk")],
    ),
    "windfall": dict(
        about="A routed fall after a wind charge's burst (mixinmod routes falls): the burst's height doesn't count, no damage",
        fixtures=["mixinmod"], scene="none", env={"HORIZON_JAVA_DEBUG": "entities"},
        steps=['$C command "gamemode survival" >/dev/null; $C command "gamerule natural_health_regeneration false" >/dev/null; '
               '$C command "item replace entity @s weapon.mainhand with minecraft:wind_charge 4" >/dev/null; $C look 0 90 >/dev/null; sleep 1; '
               '$C mouse right click >/dev/null; sleep 0.4; $C status > $S/up.json; sleep 4; $C status > $S/landed.json'],
        checks=[check_file("up.json", r'"on_ground":false', "the burst lifted the player"),
                check_json("landed.json", ["player", "health"], 20.0, "no damage on landing"),
                check_log(r"entities: \S+ falls ", "the landing went to the mods")],
    ),
    "effects3": dict(
        about="The turtle helmet's water breathing and Bad Omen turning into Trial Omen ask addEffect's head (mixinmod refuses both)",
        fixtures=["mixinmod"], scene="omen",
        steps=['$C command "item replace entity @s armor.head with minecraft:turtle_helmet" >/dev/null; sleep 2; $C state | python3 -c "import json,sys;print(sorted(e[\'id\'] for e in json.load(sys.stdin)[\'player\'][\'effects\']))" > $S/turtle.txt',
               '$C command "effect give @s minecraft:bad_omen 600 1" >/dev/null; sleep 1; $C state | python3 -c "import json,sys;print(sorted(e[\'id\'] for e in json.load(sys.stdin)[\'player\'][\'effects\']))" > $S/omen.txt; '
               '$C command "setblock $(cat $S/spawner.pos) minecraft:trial_spawner{spawn_data:{entity:{id:\\"minecraft:zombie\\"}}}" >/dev/null; '
               'sleep 5; $C state | python3 -c "import json,sys;print(sorted(e[\'id\'] for e in json.load(sys.stdin)[\'player\'][\'effects\']))" > $S/trial.txt; $C block $(cat $S/spawner.pos) > $S/spawner.json'],
        checks=[check_file("turtle.txt", r"\A(?!.*water_breathing)", "no water breathing from the helmet"),
                check_file("omen.txt", r"'bad_omen'", "Bad Omen II taken (the control)"),
                check_file("spawner.json", r"ominous=true", "the spawner saw the Bad Omen"),
                check_file("trial.txt", r"\A\[\]$", "the Bad Omen is gone; no Trial Omen")],
    ),
    "jump": dict(
        about="A player's jump reaches LivingEntity.jumpFromGround's end on the server (mixinmod counts jumps)",
        fixtures=["mixinmod"], scene="omen",
        steps=['$C key space down >/dev/null; sleep 0.3; $C key space up >/dev/null; sleep 1.5; $C key space down >/dev/null; sleep 0.3; $C key space up >/dev/null; sleep 1.5'],
        checks=[check_log(r"\[mixinmod\] jump 2", "two jumps heard")],
    ),
    "teleport": dict(
        about="A player's teleport (/tp) reaches ServerPlayer.teleport's end, at the new place (mixinmod tells where)",
        fixtures=["mixinmod"], scene="none",
        steps=['$C command "tp @s 100 120 200" >/dev/null; sleep 2'],
        checks=[check_log(r"\[mixinmod\] teleported to 100, 120, 200", "the mod heard the teleport, at its end")],
    ),
    "jump2": dict(
        about="LivingEntity.jumpFromGround's head cancels (mixinmod: not holding a slime ball): no jump on the server, its end not reached",
        fixtures=["mixinmod"], scene="omen",
        steps=['$C command "item replace entity @s weapon.mainhand with slime_ball" >/dev/null; sleep 0.5; '
               '$C key space down >/dev/null; sleep 0.3; $C key space up >/dev/null; sleep 1.5; '
               '$C key space down >/dev/null; sleep 0.3; $C key space up >/dev/null; sleep 1.5; '
               '$C command "item replace entity @s weapon.mainhand with air" >/dev/null; sleep 0.5; '
               '$C key space down >/dev/null; sleep 0.3; $C key space up >/dev/null; sleep 1.5'],
        checks=[check_log(r"\[mixinmod\] no jumping", "the jump was refused"),
                check_log(r"\[mixinmod\] jump 1 after 2 refused", "only the allowed jump reached the end")],
    ),
    "teleport2": dict(
        about="ServerPlayer.teleport's head cancels (mixinmod: not to x 666), in the level and to another; a teleport to the Nether is heard there",
        fixtures=["mixinmod"], scene="none",
        steps=['$C command "tp @s 10 120 10" >/dev/null; sleep 1; $C command "tp @s 666 120 0" >/dev/null; sleep 1; '
               '$C command "execute in minecraft:the_nether run tp @s 666 100 0" >/dev/null; sleep 2; $C state > $S/state1.json; '
               '$C command "execute in minecraft:the_nether run tp @s 20 100 20" >/dev/null; sleep 4; $C state > $S/state2.json'],
        checks=[check_log(r"\[mixinmod\] no teleport to 666 in minecraft:overworld", "refused in the level"),
                check_log(r"\[mixinmod\] no teleport to 666 in minecraft:the_nether", "refused to the Nether"),
                check_file("state1.json", r'"dimension":\s*"minecraft:overworld"', "still in the Overworld after the refusal"),
                check_file("state1.json", r'"position":\s*\[\s*10\.5', "still where the last teleport put it"),
                check_log(r"\[mixinmod\] teleported to 20, 100, 20 in minecraft:the_nether", "the Nether teleport heard at its end"),
                check_file("state2.json", r'"dimension":\s*"minecraft:the_nether"', "in the Nether")],
    ),
    "advancement": dict(
        about="PlayerAdvancements.award runs on the mirror: mixinmod hears one inside it (before the rewards) and refuses another at the head",
        fixtures=["mixinmod"], scene="none",
        steps=['$C command "advancement grant @s only minecraft:story/smelt_iron" >/dev/null; sleep 1; '
               '$C command "advancement grant @s only minecraft:story/smelt_iron" >/dev/null; sleep 1; '
               '$C command "advancement grant @s only minecraft:story/mine_stone" >/dev/null; sleep 1'],
        checks=[check_log(r"\[mixinmod\] refused minecraft:story/smelt_iron 2", "refused twice: Horizon didn't award it"),
                check_log(r"\[mixinmod\] earned minecraft:story/mine_stone", "heard inside award")],
    ),
    "slot": dict(
        about="Inventory.setSelectedSlot's head (mixinmod: not the ninth): the slot stays",
        fixtures=["mixinmod"], scene="none",
        steps=['$C key 3 tap >/dev/null; sleep 0.5; $C key 9 tap >/dev/null; sleep 1; $C state > $S/state.json'],
        checks=[check_json("state.json", ["selected"], 3, "still on the third slot")],
    ),
    "snow": dict(
        about="Serene Seasons' snow: in winter rain, the plains' floor gets snow (the mods answer precipitation)",
        mods=["seasons", "glitchcore"], scene="snow",
        steps=['$C command "season set mid_winter" >/dev/null; $C command "gamerule random_tick_speed 300" >/dev/null; sleep 12; '
               '$C command "gamerule random_tick_speed 3" >/dev/null; '
               'read a b c d e f < $S/floor.pos; $C blocks $a $b $c $d $e $f > $S/floor.json'],
        checks=[check_file("floor.json", r'"minecraft:snow":\s*[1-9]', "snow on the floor"),
                check_log(r"Precipitation goes to the mods", "precipitation goes to the mods")],
        look="snow layers on the stone",
    ),
    "punch": dict(
        about="Fabric's AttackBlockCallback, asked where native mods' left clicks are (eventsmod: poppies can't be punched)",
        fixtures=["eventsmod"], scene="punch",
        # Since upstream sync 4 the first left click after the scene is sometimes lost (no
        # break, no event; cause not found, see the sync 4 report): click again, up to 3 times,
        # each retry noted in $S/retries.txt.
        steps=['for t in 1 2 3; do aim $S/poppy.pos || echo poppy >> $S/missed.txt; sleep 0.5; $C mouse left click >/dev/null; sleep 1.5; '
               'grep -q "leave the poppy alone" $S/home/logs/latest.log && break; echo $t >> $S/retries.txt; done; $C block $(cat $S/poppy.pos) > $S/poppy.json',
               'aim $S/dandelion.pos || echo dandelion >> $S/missed.txt; sleep 0.5; $C mouse left click >/dev/null; sleep 1.5; $C block $(cat $S/dandelion.pos) > $S/dandelion.json'],
        checks=[check_json("poppy.json", ["state"], "minecraft:poppy", "the poppy stays (on the client too)"),
                check_log(r"leave the poppy alone", "the mod took the click"),
                check_json("dandelion.json", ["state"], "minecraft:air", "the dandelion breaks")],
    ),
    "hurt": dict(
        about="Fabric's ALLOW_DAMAGE, asked where native mods hear a hit (eventsmod: magma doesn't hurt); other damage does",
        fixtures=["eventsmod"], scene="none",
        steps=['$C command "gamemode survival" >/dev/null; $C command "gamerule natural_health_regeneration false" >/dev/null; '
               '$C command "setblock ~ ~-1 ~ minecraft:magma_block" >/dev/null; sleep 3; $C status > $S/magma.json',
               '$C command "damage @s 4 minecraft:generic" >/dev/null; sleep 1; $C status > $S/generic.json'],
        checks=[check_json("magma.json", ["player", "health"], 20.0, "no harm standing on magma"),
                check_json("generic.json", ["player", "health"], 16.0, "4 from /damage")],
    ),
    "death": dict(
        about="A player's death reaches LivingEntity.die's head with its cause, asked where native mods hear deaths (eventsmod's DeathMixin)",
        fixtures=["eventsmod"], scene="none",
        steps=['$C command "gamemode survival" >/dev/null; sleep 0.5; $C command "damage @s 40 minecraft:lava" >/dev/null; sleep 2; '
               '$C respawn >/dev/null; sleep 3',
               '$C command "tp @s ~ ~40 ~" >/dev/null; sleep 5'],
        checks=[check_log(r"a mixin saw \S+ die of minecraft:lava", "the lava death, by its message"),
                check_log(r"a mixin saw \S+ die of minecraft:fall", "the fall death, by the combat tracker")],
    ),
    "mixins1": dict(
        about="Round one's actions (mixinmod): a broken block also drops a diamond, dirt isn't picked up, wheat grows in dim light, cows can't be milked",
        fixtures=["mixinmod"], scene="mixins1",
        steps=['$C mouse left down >/dev/null; sleep 2; $C mouse left up >/dev/null; sleep 1; '
               '$C command "tp @s ~2 ~ ~" >/dev/null; sleep 3; $C inventory > $S/inv.json; $C entities 4 minecraft:item > $S/ground.json',
               # Up to 20 s for a growth (a busy full run ticks slower than one case alone).
               '$C command "gamerule random_tick_speed 1000" >/dev/null; '
               'for i in $(seq 20); do sleep 1; $C block $(cat $S/crop.pos) > $S/crop.json; grep -q "age=[1-7]" $S/crop.json && break; done; '
               '$C command "gamerule random_tick_speed 3" >/dev/null',
               '$C command "item replace entity @s weapon.mainhand with minecraft:bucket" >/dev/null; '
               '$C command "summon minecraft:cow ~2 ~ ~ {NoAI:1b}" >/dev/null; sleep 1; '
               '$C look-at-entity $($C entities 6 minecraft:cow | python3 -c "import json,sys;print(json.load(sys.stdin)[\'entities\'][0][\'id\'])") >/dev/null; '
               'sleep 0.5; $C mouse right click >/dev/null; sleep 1; $C inventory > $S/milk.json'],
        checks=[check_file("inv.json", r'"minecraft:diamond":\s*1', "a diamond with the dirt (picked up)"),
                check_file("ground.json", r"minecraft:dirt", "the dirt left on the ground"),
                check_file("crop.json", r"wheat\[age=[1-7]\]", "the wheat grew in the dim light"),
                check_file("milk.json", r'"minecraft:bucket":\s*1', "the cow wasn't milked (Player.interactOn's head)"),
                check_log(r"Random ticks of \d+ vanilla blocks go to the mods", "crop ticks go to the mods")],
    ),
    "mixins2": dict(
        about="Round two's actions (mixinmod): no snowball throws, an emerald with each craft",
        fixtures=["mixinmod"], scene="none",
        steps=['$C command "gamemode survival" >/dev/null; $C command "give @s minecraft:snowball 4" >/dev/null; sleep 0.5; '
               '$C select minecraft:snowball >/dev/null; $C look 0 -10 >/dev/null; $C mouse right click >/dev/null; sleep 1; '
               '$C inventory > $S/snow.json',
               '$C command "clear @s" >/dev/null; $C command "give @s minecraft:oak_log 1" >/dev/null; sleep 0.5; '
               '$C open-inventory >/dev/null; sleep 0.5; $C click-slot 0 >/dev/null; sleep 0.3; $C click-slot 100 >/dev/null; '
               'sleep 0.3; $C click-slot 200 >/dev/null; sleep 0.3; $C click-slot 1 >/dev/null; sleep 1; '
               '$C key escape tap >/dev/null; sleep 1; $C inventory > $S/craft.json'],
        checks=[check_file("snow.json", r'"minecraft:snowball":\s*4', "the snowballs stayed (not thrown)"),
                check_file("craft.json", r'"minecraft:emerald":\s*1', "an emerald with the planks")],
    ),
    "lost": dict(
        about="Lost blocks: 36 gold blocks set right after spawn, chunks still generating, all stay",
        mods=["fd"], scene="none",
        steps=['$R/tools/java_bridge/lost_blocks.sh "$C" $S/lost.txt 25'],
        checks=[check_file("lost.txt", r"^lost 0 of (3[0-9]) set", "every gold block stays (30+ set)")],
        forbid=[r"generated again while loaded"],
    ),
    "maps": dict(
        about="Xaero's minimap with Trinkets: the models load (Yumi's client init, Trinkets' reloader) and the map draws",
        mods=["xaero", "trinkets"], scene="none", env={"HORIZON_JAVA_DEBUG": "gpu"},
        steps=['sleep 30'],
        checks=[check_log(r"block and item models loaded", "the models' reload ends"),
                check_log(r"gpu: a blit of horizon:dynamic/gpu", "the minimap's picture reaches the HUD")],
        forbid=[r"models' reload didn't end", r"handler\$\w+\$xaerominimap\$\w+\): java"],
        look="Xaero's minimap at the top left: land in its colours, N/E/S/W",
    ),
    "journeymap": dict(
        about="JourneyMap (with Trinkets): it maps the chunks around (its task thread reads the client's chunks)",
        mods=["journeymap", "trinkets"], scene="none",
        steps=['sleep 90; find $S/journeymap/data -name "c.*.jmc" > $S/jm.txt'],
        checks=[check_log(r"block and item models loaded", "the models' reload ends"),
                check_file("jm.txt", r"\.jmc", "JourneyMap saved mapped chunks")],
        look="JourneyMap's round minimap at the top right, filled in",
    ),
    "voxelmap": dict(
        about="VoxelMap: its minimap draws through the feature dispatcher",
        mods=["voxelmap"], scene="none", env={"HORIZON_JAVA_DEBUG": "gpu"},
        steps=['sleep 40'],
        checks=[check_log(r"gpu: a blit of horizon:dynamic/gpu", "VoxelMap's HUD target reaches the HUD")],
        forbid=[r"voxelmap\$onTick\): java"],
        look="VoxelMap's round minimap at the top right: land, arrow, coordinates",
    ),
    "diorama": dict(
        about="Diorama: told of its chunks (ClientChunkEvents), it bakes map tiles and its minimap draws them",
        mods=["diorama"], scene="none", env={"HORIZON_JAVA_DEBUG": "gpu"},
        steps=['sleep 50'],
        checks=[check_log(r"gpu: a blit of horizon:dynamic/gpu\d+ 512x512", "a baked 512 px tile reaches the HUD")],
        forbid=[r"\[dev\.diorama\.\S+\]: java", r"\$diorama\$\w+\): java"],
        look="Diorama's square minimap at the top right: the land around, N/E/S/W, the arrow",
    ),
    "xaeroplus": dict(
        about="XaeroPlus over Xaero's maps: the minimap, and M's World Map draws (XaeroPlus clears its target as it adds it to the GUI)",
        mods=["xaero", "xaeroworldmap", "xaeroplus"], scene="none", env={"HORIZON_JAVA_DEBUG": "gpu"},
        steps=['sleep 40', '$C key m tap; sleep 15'],
        checks=[check_log(r"gpu: horizon:dynamic/gpu\d+ 447x267, \d+ pixels not transparent, [1-9]\d{3,} not black", "the minimap draws"),
                check_log(r"gpu: horizon:dynamic/gpu\d+ 427x240, \d+ pixels not transparent, [1-9]\d{4,} not black", "the World Map's target (shrunk) has the map")],
        forbid=[r"handler\$\w+\$xaero\w*\$\w+\): java", r"Failed to get data directory"],
        look="the World Map: the explored land in its colours, the red arrow, X/Y/Z at the top",
    ),
    "worldmap-lazy": dict(
        about="Xaero's Minimap and World Map with the mirror's models baked on first use (a big pack's don't fit: HORIZON_JAVA_MODELS=off): both draw, no frame hook fails on a missing ModelManager",
        mods=["xaero", "xaeroworldmap"], scene="none", env={"HORIZON_JAVA_DEBUG": "gpu", "HORIZON_JAVA_MODELS": "off"},
        steps=['sleep 40', '$C key m tap; sleep 25'],
        checks=[check_log(r"models are baked on first use", "the models are lazy"),
                check_log(r"models baked on first use \(BlockState\)", "map mods baked the states they read"),
                check_log(r"gpu: horizon:dynamic/gpu\d+ 447x267, \d+ pixels not transparent, [1-9]\d{3,} not black", "the minimap draws"),
                check_log(r"gpu: horizon:dynamic/gpu\d+ 427x240, \d+ pixels not transparent, [1-9]\d{4,} not black", "the World Map's target (shrunk) has the map")],
        forbid=[r"handler\$\w+\$xaero\w*\$\w+\): java", r"ModelManager\.getBlockStateModelSet\(\)", r"model baked on first use failed"],
        look="the World Map in its colours (plains, forest, water), the red arrow, X/Y/Z at the top",
    ),
    "minihud": dict(
        about="MiniHUD (MaLiLib): its info lines draw at the end of the GUI's frame (Gui.extractRenderState TAIL)",
        mods=["malilib", "minihud"], scene="none",
        steps=['sleep 30'],
        checks=[check_log(r"mod injection\(s\) run at the end of the GUI's frame", "MaLiLib's GUI hook is found"),
                check_log(r"run each client <start at its end>", "MaLiLib's hooks at the end of the client's start are found")],
        forbid=[r"A mod failed in the GUI's frame", r"A mod failed in the client's"],
        look="MiniHUD's lines at the top left: the date, coordinates, facing",
    ),
    "computer": dict(
        about="CC: Tweaked: a computer's screen (its client menu made from Fabric's opening data) draws its terminal, and typing reaches the computer",
        mods=["cc"], scene="computer",
        steps=['$C type "mkdir typed"; sleep 1; $C key enter tap; sleep 3; '
               'find $S -path "*computercraft/computer*" -name typed > $S/typed.txt; $C state > $S/state.json'],
        checks=[check_json("state.json", ["screen"], "container", "the computer's screen is open"),
                check_file("typed.txt", r"computer/0/typed", "the typed command ran on the computer")],
        forbid=[r"Cannot update terminal", r"client's menu of \S+ couldn't be made", r"screen of computercraft:\S+ could not open"],
        look="the advanced computer's terminal: CraftOS 1.9, '> mkdir typed', a new prompt",
    ),
    "skills": dict(
        about="Pufferfish's Skills: K opens its skill tree (a client-only screen): item icons drawn as oversized items on the headless GPU, lines between skills at their angle; its messages carried by their codecs (one class both ways), the client joined before them",
        mods=["skills"], scene="none", env={"HORIZON_JAVA_DEBUG": "gpu"},
        config={
            "puffish_skills/config.json": '{"version": 3, "categories": ["test"]}',
            "puffish_skills/categories/test/category.json":
                '{"unlocked_by_default": true, "title": "Test", "icon": {"type": "item", "data": {"item": "diamond_sword"}}, "background": "textures/block/stone.png"}',
            "puffish_skills/categories/test/definitions.json":
                '{"sword": {"title": "Sword", "icon": {"type": "item", "data": {"item": "iron_sword"}}, "rewards": []},'
                ' "heart": {"title": "Heart", "icon": {"type": "effect", "data": {"effect": "health_boost"}}, "rewards": []}}',
            "puffish_skills/categories/test/skills.json":
                '{"a": {"x": 0, "y": 0, "definition": "sword", "root": true}, "b": {"x": 64, "y": 48, "definition": "heart"},'
                ' "c": {"x": -64, "y": 48, "definition": "sword"}}',
            "puffish_skills/categories/test/connections.json": '{"normal": {"bidirectional": [["a", "b"], ["a", "c"]]}}',
            "puffish_skills/categories/test/experience.json": '{"experience_per_level": {"type": "expression", "data": {"expression": "10"}}, "sources": []}',
        },
        steps=['sleep 20', '$C key k tap; sleep 30'],
        checks=[check_log(r"gpu: a blit of horizon:dynamic/gpu\d+ 48x48", "the skills' item icons draw (oversized items on the GPU)")],
        forbid=[r"A mod failed in", r"the client of puffish_skills:\S+ failed", r"puffish_skills\S*: java"],
        look="none-0: the skill tree: an iron sword skill in the middle, slanted white lines to a heart and another sword, the Test tab with a diamond sword (the background stays black: not drawn yet)",
    ),
    "tooltips": dict(
        about="Legendary Tooltips (Iceberg, Prism) over a mod screen: tooltips draw through vanilla's path, where its mixins run (centred title, the border in the item's rarity colour)",
        mods=["legendarytooltips", "iceberg", "prism-lib", "fd"], scene="pot", env={"HORIZON_JAVA_DEBUG": "gpu"},
        steps=['$C click-at 141 187; sleep 1; $C click-at 141 187; sleep 3',
               # An iron sword's tooltip: Iceberg draws its turning 3D model on the headless GPU.
               '$C command "give @s minecraft:iron_sword"; sleep 2; $C click-at 177 187; sleep 1; $C click-at 177 187; sleep 3'],
        checks=[check_log(r"Java screens' tooltips draw as vanilla's", "tooltips take vanilla's path"),
                check_log(r"gpu: horizon:dynamic/gpu\d+ 96x96, [1-9]\d* pixels not transparent", "the sword's 3D model drew (Iceberg's target)"),
                check_log(r"mod injection\(s\) run each client renderFrame", "Iceberg's render tick runs (the model turns)")],
        forbid=[r"A mod failed in", r"iceberg\S*: java", r"a draw skipped: \S+: uniform \d+ unbound"],
        look="pot-0: Raw Beef's tooltip: title centred, a white border (common rarity), not vanilla's purple; pot-1: the Iron Sword's tooltip with the sword's 3D model in its box",
    ),
    "maps-lan": dict(
        about="JourneyMap per player, as in vanilla: the LAN guest's own game runs every mod's client half "
              "(HORIZON_JAVA_OWN_CLIENT), so its minimap maps its own world around it, 40 blocks east of the host's, "
              "its map data and waypoints are in its own game folder under the server it joined, and its client leaves with it",
        mods=["journeymap"], scene="none", lan=True, env={"HORIZON_JAVA_OWN_CLIENT": "1"},
        steps=['$H command "tp Guest ~40 ~ ~" >/dev/null; sleep 100; $G state > $S/guest.json; '
               '$H screenshot $S/h.png >/dev/null; $G screenshot $S/g.png >/dev/null; '
               # The minimap's square (top right): mapped (colours, not black) on both, each its own.
               'python3 -c "import sys; from PIL import Image, ImageStat; c=lambda p: Image.open(p).convert(\'RGB\').crop((1070, 100, 1260, 230)); '
               'm=lambda i: (sum(ImageStat.Stat(i).mean) / 3, sum(ImageStat.Stat(i).stddev) / 3); h, g = c(sys.argv[1]), c(sys.argv[2]); '
               'd=sum(ImageStat.Stat(__import__(\'PIL.ImageChops\', fromlist=[\'x\']).difference(h, g)).mean) / 3; '
               'print(\'host\', \'mapped\' if m(h)[0] > 40 and m(h)[1] > 15 else \'empty\', \'guest\', \'mapped\' if m(g)[0] > 40 and m(g)[1] > 15 else \'empty\', '
               '\'own\' if d > 12 else \'same\', m(h), m(g), d)" $S/h.png $S/g.png > $S/maps.txt; '
               'ls -d $GS/journeymap/data/mp/*/waypoints > $S/guest-folders.txt 2>&1; '
               'grep -c "every mod.s client half runs here for this player" $GS/home/logs/latest.log > $S/guest-own.txt',
               '$G disconnect >/dev/null; sleep 10; grep -c "own client left" $GS/home/logs/latest.log > $S/guest-left.txt'],
        checks=[check_file("guest.json", r'"screen":"playing"', "the guest plays"),
                check_log(r"Guest\S* joined the game", "the guest joined"),
                check_file("guest-own.txt", r"^[1-9]", "the guest's own game runs its mods' client half"),
                check_file("maps.txt", r"host mapped", "the host's minimap is mapped"),
                check_file("maps.txt", r"guest mapped", "the guest's minimap is mapped (its own game's chunks)"),
                check_file("maps.txt", r" own ", "each minimap is its own player's (they differ)"),
                check_file("guest-folders.txt", r"journeymap/data/mp/localhost[^/]*/waypoints", "the guest's map data and waypoints: its own folder, by the server it joined"),
                check_file("guest-left.txt", r"^[1-9]", "the guest's client leaves with it")],
        forbid=[r"StackOverflowError", r"this game's own client has no mirror"],
        look="lan-1-host.png and lan-1-guest.png: each minimap centred on its own player (the guest's coordinates 40 more in x)",
    ),
    "server": dict(
        about="A dedicated server with Java mods starts, its player joins at once and gets the HUD mods (Jade, AppleSkin) and Xaero's minimap",
        mods=["jade", "appleskin", "xaero"], scene="none", server=True,
        steps=['sv "gamemode survival Player"; sv "give Player minecraft:bread"; $G look 0 80 >/dev/null; sleep 6; $G state > $S/player.json'],
        checks=[check_log(r"Done \(", "the server starts"),
                check_file("player.json", r'"screen":"playing"', "the player plays"),
                check_log(r"Gave 1 \[Bread\] to Player", "the player was in the world for the console's commands")],
        forbid=[r"waited two minutes for the Java mods"],
        look="Xaero's minimap top left, Jade's box at the top, AppleSkin's hunger outline",
    ),
    "server-log": dict(
        about="A dedicated server's logs/latest.log has one writer: Java's log4j writes no file of its own there "
              "(it overwrote Horizon's lines); mods' log lines go into Horizon's log",
        mods=["fd"], scene="none", server=True,
        steps=['sleep 4; ls $S/logs > $S/log-files.txt'],
        checks=[check_log(r"Done \(", "the server's start line is there"),
                check_log(r"\[java\] \[\w+/(INFO|WARN|ERROR)\] ", "mods' and the game's log4j lines are in Horizon's log"),
                check_file("log-files.txt", r"\A(?![\s\S]*\.log\.gz)[\s\S]*latest\.log", "no log4j roll-over of latest.log on Java's start")],
        look="server-log: the server's latest.log, read whole",
    ),
    "maps-server": dict(
        about="Map mods for a dedicated server's player: JourneyMap's minimap and full-screen map, Xaero's minimap and World Map (M), no JourneyMap server-tick failure",
        mods=["journeymap", "xaero", "xaeroworldmap"], scene="none", server=True, env={"HORIZON_JAVA_DEBUG": "gpu"},
        steps=['sleep 70', '$G key m tap; sleep 15; $G key escape tap; sleep 2; $G state > $S/player.json'],
        checks=[check_log(r"Done \(", "the server starts"),
                check_file("player.json", r'"screen":"playing"', "the player plays"),
                check_log(r"gpu: horizon:dynamic/gpu\d+ 427x240, \d+ pixels not transparent, [1-9]\d{4,} not black", "the World Map's target has the map")],
        forbid=[r"journeymap.common.event.FabricServerEvents\]: java", r"handler\$\w+\$xaero\w*\$\w+\): java"],
        look="server-1.png: JourneyMap's minimap top right, Xaero's top left; server-2.png: Xaero's World Map",
    ),
    "rei-lan": dict(
        about="REI per player, as in vanilla (each game runs its own client: HORIZON_JAVA_OWN_CLIENT): a LAN guest with 4 oak logs turns on its craftable-only filter and sees what it can craft; the host's stays off, its list full",
        mods=["rei", "architectury", "cloth", "fd"], scene="none", lan=True, env={"HORIZON_JAVA_OWN_CLIENT": "1"},
        steps=[# The guest's own client (its JVM starts as it joins) and its REI reload at its join.
               'for i in $(seq 240); do awk \'/client half runs here/{f=1} f&&/REIClientPlugin finished post-reload for END/{e=1} END{exit !e}\' '
               '$GS/home/logs/latest.log && break; sleep 1; done; sleep 3; '
               'grep -c "every mod.s client half runs here for this player" $GS/home/logs/latest.log > $S/guest-own.txt; '
               '$H command "gamemode survival @a" >/dev/null; $H command "give Guest minecraft:oak_log 4" >/dev/null; sleep 1; '
               '$G key e tap; sleep 20; $G key escape tap; sleep 2; $G key e tap; sleep 5',
               # The filter button beside REI's search box (GUI 270 227): green when on.
               '$G click-at 270 227; sleep 5; $G screenshot $S/g.png >/dev/null; $H key e tap; sleep 8; $H screenshot $S/h.png >/dev/null; '
               'python3 -c "import sys; from PIL import Image, ImageFilter; on=lambda p: (lambda c: c[1] > c[0] + 20)(Image.open(p).convert(\'RGB\').getpixel((812, 680))); '
               'e=lambda p: sum(Image.open(p).convert(\'L\').filter(ImageFilter.FIND_EDGES).crop((930, 90, 1280, 620)).getdata()); print(\'guest\', \'on\' if on(sys.argv[1]) else \'off\', \'few\' if e(sys.argv[1]) < 2700000 else \'full\', '
               '\'host\', \'on\' if on(sys.argv[2]) else \'off\', \'few\' if e(sys.argv[2]) < 2700000 else \'full\')" '
               '$S/g.png $S/h.png > $S/filters.txt'],
        checks=[check_file("guest-own.txt", r"^[1-9]", "the guest's own game runs its mods' client half"),
                check_file("filters.txt", r"guest on", "the guest's filter is on"),
                check_file("filters.txt", r"host off", "the host's filter is off"),
                check_file("filters.txt", r"guest \w+ few", "the guest's list: what 4 oak logs craft"),
                check_file("filters.txt", r"host \w+ full", "the host's list: everything")],
        forbid=[r"settings can't be kept per player", r"search failed"],
        look="lan-2-guest.png: the filter green, planks; lan-3-host.png: red, the whole list",
    ),
    "lootinfo": dict(
        about="Advanced Loot Info's REI plugin asks the server for its loot data from REI's reload threads (REI's reload at the client's join): the request is queued for the loop and answered",
        mods=["ali", "aci", "rei", "architectury", "cloth"], scene="none", env={"HORIZON_JAVA_DEBUG": "loop"},
        steps=['sleep 20'],
        checks=[check_log(r"client to server ali:\S+ from \S+ \(off the loop's thread: queued\)", "the plugin's request from REI's thread is queued, not dropped"),
                check_log(r"server to client ali:\S+ for ", "the server answers it"),
                # Unanswered, the plugin waits 3 x 30 s and REI's reload with it.
                check_log(r"REIClientPlugin finished post-reload for END in [0-9.]+ m?s, totaling [0-9.]+ m?s", "REI's join reload ends, not held by the plugin's timeouts")],
        forbid=[r"was sent with no player's client running", r"Incomplete data", r"Missing chunk with index"],
        look="REI reloads at the join with Advanced Loot Info's data (R over diamond ore: its block loot page)",
    ),
    "cloth": dict(
        about="Cloth Config's own settings screen (its demo, from Options > Mods): its icons drawn (a hyphenated mod id's "
              "textures carried), a list expanded, the list scrolled (screens get the frame's delta ticks, as vanilla's Gui "
              "passes), its search box filtering; a fast mouse's moves draw at most every 40 ms",
        mods=["cloth"], optional=["fabric-api"], scene="none", env={"HORIZON_JAVA_DEBUG": "frames"},
        steps=['$C options mods >/dev/null; sleep 1; $C click cloth-config-v26-3 >/dev/null; sleep 5; $C state > $S/open.json',
               # "A list of Doubles"' arrow: the list opens (1.0, 2.0, 3.0 under it).
               '$C click-at 45 152 >/dev/null; sleep 2',
               '$C scroll-at 200 150 -10 >/dev/null; sleep 3',
               '$C scroll-at 200 150 40 >/dev/null; sleep 2; $C click-at 200 49 >/dev/null; sleep 0.5; '
               '$C type color >/dev/null; sleep 2',
               # 150 moves in ~1.5 s (drag-at moves through each point): the server keeps up.
               'pts=$(for i in $(seq 150); do echo -n "$((100 + i)) $((120 + i % 7)) "; done); '
               't0=$(date +%s%N); $C drag-at $pts >/dev/null; t1=$(date +%s%N); '
               'echo "moves took $(( (t1 - t0) / 1000000 )) ms" > $S/moves.txt'],
        checks=[check_json("open.json", ["screen"], "java-screen", "Cloth Config's settings screen opens"),
                check_log(r"content: cloth-config: [1-9][0-9]* files", "its textures are carried (mod id cloth-config)"),
                check_log(r"frame of me\.shedaniel\.clothconfig2\.\S+: [^\n]*cloth-config2:textures/gui/cloth_config\.png",
                          "its icons are drawn from its own texture"),
                check_log(r"frame of me\.shedaniel\.clothconfig2\.\S+: [^\n]*A list of Doubles@\d+[ |@\d]*1\.0@\d+[ |@\d]*2\.0@",
                          "the list opens"),
                check_log(r"frame of me\.shedaniel\.clothconfig2\.\S+: [^\n]*Config Demo@-\d+",
                          "scrolled: its top went up out of the list"),
                check_log(r"(?m)frame of me\.shedaniel\.clothconfig2\.\S+: [^\n]*text color@(?:(?!Cool Key)[^\n])*A color field(?:(?!Cool Key)[^\n])*$",
                          "its search box filters the entries"),
                check_file("moves.txt", r"moves took ([0-9]|[0-9]{2}|[0-9]{3}|[0-4][0-9]{3}) ms", "150 mouse moves pass in under 5 s")],
        forbid=[r"settings screen failed", r"a screen failed to draw", r"a screen's \S+ failed"],
        look="none-0.png: Cloth's demo with its arrows and +/- icons; none-1.png: a list open; none-2.png: scrolled; "
             "none-3.png: searching \"color\" (the colour fields)",
    ),
    "modsettings2": dict(
        about="Options > Mods: a mod's own settings screen and one it gives another mod (getProvidedConfigScreenFactories, as Forge Config API Port does) open with a parent and go back to the Mods page on Done; a broken \"modmenu\" entrypoint (Compressed Blocks') is skipped, as Mod Menu does",
        fixtures=["configmod"], scene="none",
        steps=['$C options mods >/dev/null; sleep 1; $C ui > $S/ui.json',
               '$C click horizon-config-extra >/dev/null; sleep 3; $C state > $S/provided.json',
               '$C click-at 213 120 >/dev/null; sleep 2; $C state > $S/back.json',
               '$C click horizon-config >/dev/null; sleep 3; $C messages > $S/messages.json'],
        checks=[check_file("ui.json", r'"id":"horizon-config"', "the Mods page lists the mod's own screen"),
                check_file("ui.json", r'"id":"horizon-config-extra"', "and the one given to another mod"),
                check_file("provided.json", r'"screen":"java-screen"', "the given screen opens"),
                check_file("back.json", r'"screen":"options"', "Done goes back to the Mods page"),
                check_file("messages.json", r"horizon_config: provided screen, parent HorizonModsPage", "the given screen has a parent"),
                check_file("messages.json", r"horizon_config: own screen, parent HorizonModsPage", "the own screen has a parent"),
                check_log(r"horizon_config's \"modmenu\" entrypoint isn't a ModMenuApi", "the broken entrypoint is skipped")],
        forbid=[r"settings screen failed", r"settings screen can't be made"],
        look="the fixture's own settings screen (a Done button)",
    ),
    "config-rejoin": dict(
        about="A LAN guest's kept mod settings (config/horizon-players/<uuid>/) are theirs at their next join: REI's item list on the left for the guest, on the right for the host",
        mods=["rei", "architectury", "cloth"], scene="none", lan=True,
        steps=[# The guest's kept REI config, as their saved settings left it (only the panel's side).
               'K=$S/config/horizon-players/404198aa-cc64-3021-9fdf-8766c0d0096c/roughlyenoughitems; mkdir -p $K; '
               'echo \'{"advanced": {"accessibility": {"displayPanelLocation": "LEFT"}}}\' > $K/config.json5; '
               '$G disconnect >/dev/null; sleep 4; $G connect "localhost:$PORT" Guest >/dev/null; $G wait-ready >/dev/null 2>&1; sleep 5',
               '$H command "gamemode survival @a" >/dev/null; sleep 1; '
               '$G key e tap; sleep 20; $G key escape tap; sleep 2; $G key e tap; sleep 5; $G screenshot $S/g.png >/dev/null; '
               '$H key e tap; sleep 8; $H screenshot $S/h.png >/dev/null; '
               # Item icons are sharp over the blurred world: which side has more edges.
               'python3 -c "import sys; from PIL import Image, ImageFilter; '
               'e=lambda p, x0: sum(Image.open(p).convert(\'L\').filter(ImageFilter.FIND_EDGES).crop((x0, 100, x0 + 300, 600)).getdata()); '
               'side=lambda p: \'left\' if e(p, 0) > e(p, 980) else \'right\'; '
               'print(\'guest\', side(sys.argv[1]), \'host\', side(sys.argv[2]))" $S/g.png $S/h.png > $S/sides.txt'],
        checks=[check_file("sides.txt", r"guest left", "the guest's REI list is on the left (their setting)"),
                check_file("sides.txt", r"host right", "the host's stays on the right")],
        forbid=[r"settings can't be kept per player", r"kept config files"],
        look="lan-2-guest.png: REI's list left of the inventory; lan-2-host.png: right",
    ),
    # client r6-3's case, VoxelMap alone (with JourneyMap it breaks VoxelMap: the two register the same packet).
    "voxelmap-server": dict(
        about="VoxelMap for a dedicated server's player",
        mods=["voxelmap"], scene="none", server=True,
        steps=['sleep 90; $G state > $S/player.json'],
        checks=[check_log(r"Done \(", "the server starts"),
                check_file("player.json", r'"screen":"playing"', "the player plays")],
        forbid=[r"per-player state couldn't be swapped", r"couldn't change player", r"StackOverflowError",
                r"JourneyMap waypoints couldn't load", r"JourneyMap's settings can't be kept",
                r"waited two minutes for the Java mods"],
        look="server-1.png: VoxelMap's minimap top right",
    ),
    "bigpack": dict(
        about="A whole pack (--pack): Java starts, the registries load, a player joins in time, the mods' worldgen runs",
        pack=True, scene="none", timeout=2700,
        steps=['sleep 20; $C state > $S/state.json'],
        checks=[check_log(r"Fabric started in", "Java and Fabric start"),
                check_log(r"System chat: \S+ joined the game", "a player joins"),
                check_file("state.json", r'"block_pos":\[', "the player is in the world")],
        forbid=[r"Java mods: not started", r"Registry loading errors", r"their content was not made",
                r"Java mods' worldgen: chunk -?\d+ -?\d+ failed", r"control socket did not open",
                r"fell out of the world", r"OutOfMemoryError", r"exceeding packet rate limit"],
        look="the pack's world, its HUD (minimap) drawn",
    ),
    "fixer": dict(
        about="Dungeons and Taverns' structures (templates of ~80 old DataVersions): the data fixer's "
              "rule chains are let go, structures still generate after (a teleport to new land)",
        mods=["dungeons"], scene="none",
        steps=['sleep 30; $C command "tp @s 2000 120 2000" > /dev/null; sleep 30; '
               '$C command "time query gametime" > $S/after.json'],
        checks=[check_log(r"the data fixer's \d+ rule chains .* let go", "the fixer's chains are let go"),
                check_file("after.json", r"game time is", "the server still answers after new land generates")],
        forbid=[r"data fixer's caches can't be bounded", r"Java mods' worldgen: chunk -?\d+ -?\d+ failed",
                r"worldgen: proto failed", r"OutOfMemoryError"],
    ),
    "xnet": dict(
        about="XNet's controller (McJtyLib, whose machines keep their data as Fabric block entity "
              "attachments): placed, used, its screen opens",
        mods=["xnet", "mcjtylib", "rftoolsbase", "fcap"], scene="none",
        steps=['read X Y Z < <($C state | grep -o \'"block_pos":\\[[^]]*\' | head -1 | sed \'s/.*\\[//; s/,/ /g\'); '
               '$C command "fill ~-1 ~ ~1 ~1 ~2 ~3 minecraft:air" > /dev/null; '
               '$C command "setblock $X $Y $((Z + 2)) xnet:controller" > $S/place.json; sleep 2; '
               '$C interact $X $Y $((Z + 2)) north > /dev/null; sleep 3; $C screen > $S/screen.json'],
        checks=[check_file("place.json", r"Changed the block", "the controller is placed"),
                check_file("screen.json", r'"screen":"container"', "its screen is open")],
        forbid=[r"updatingChunkMap", r"McJtyLib failed"],
        look="XNet's controller screen",
    ),
    "hoppers": dict(
        about="Horizon's hoppers through a Java mod's blocks (Fabric's ItemStorage): into Farmer's Delight's "
              "cooking pot, and hopper -> oak cabinet -> hopper -> chest",
        mods=["fd"], scene="none",
        steps=['read X Y Z < <($C state | grep -o \'"block_pos":\\[[^]]*\' | head -1 | sed \'s/.*\\[//; s/,/ /g\'); X=$((X + 3)); '
               '$C command "fill $((X - 1)) $Y $((Z - 1)) $((X + 3)) $((Y + 4)) $((Z + 1)) minecraft:air" > /dev/null; '
               '$C command "setblock $X $Y $Z farmersdelight:cooking_pot" > /dev/null; '
               '$C command "setblock $X $((Y + 1)) $Z minecraft:hopper{Items:[{Slot:0b,id:\\"minecraft:beef\\",count:3}]}" > /dev/null; '
               'C2=$((X + 3)); $C command "setblock $C2 $Y $Z minecraft:chest" > /dev/null; '
               '$C command "setblock $C2 $((Y + 1)) $Z minecraft:hopper" > /dev/null; '
               '$C command "setblock $C2 $((Y + 2)) $Z farmersdelight:oak_cabinet" > /dev/null; '
               '$C command "setblock $C2 $((Y + 3)) $Z minecraft:hopper{Items:[{Slot:0b,id:\\"minecraft:beef\\",count:5},'
               '{Slot:1b,id:\\"minecraft:carrot\\",count:2}]}" > /dev/null; sleep 25; '
               '$C command "data get block $X $((Y + 1)) $Z Items" > $S/pot_hopper.json; '
               '$C command "data get block $C2 $Y $Z Items" > $S/chest.json; '
               '$C command "data get block $C2 $((Y + 3)) $Z Items" > $S/top_hopper.json'],
        checks=[check_file("pot_hopper.json", r"block data: \[\]", "the hopper pushed its beef into the cooking pot"),
                check_file("chest.json", r"count: 5, Slot: 0b, id: \\+\"minecraft:beef", "five beef came out of the cabinet"),
                check_file("chest.json", r"count: 2, Slot: 1b, id: \\+\"minecraft:carrot", "and the two carrots"),
                check_file("top_hopper.json", r"block data: \[\]", "the top hopper emptied into the cabinet")],
        forbid=[r"a hopper's (push|pull)"],
    ),
    "droppers": dict(
        about="Horizon's droppers into a Java mod's block (Fabric's DropperBlockMixin: three carrots into Farmer's "
              "Delight's oak cabinet, a hopper under it into a chest), and its hoppers into and out of a mod's "
              "container entity (the cartmod fixture's barrel cart: hopper -> cart -> hopper -> chest)",
        fixtures=["cartmod"], mods=["fd"], scene="none",
        steps=['read X Y Z < <($C state | python3 -c "import json,sys,math;p=json.load(sys.stdin)[\'player\'][\'position\'];'
               'print(math.floor(p[0]),math.floor(p[1]),math.floor(p[2]))"); '
               '$C command "fill $((X - 6)) $((Y - 3)) $((Z - 2)) $((X + 6)) $((Y + 4)) $((Z + 2)) minecraft:air" > /dev/null; '
               '$C command "setblock $((X - 3)) $Y $Z minecraft:dropper[facing=west]{Items:[{Slot:0b,id:\\"minecraft:carrot\\",count:3}]}" > /dev/null; '
               '$C command "setblock $((X - 4)) $Y $Z farmersdelight:oak_cabinet" > /dev/null; '
               '$C command "setblock $((X - 4)) $((Y - 1)) $Z minecraft:hopper[facing=down]" > /dev/null; '
               '$C command "setblock $((X - 4)) $((Y - 2)) $Z minecraft:chest" > /dev/null; '
               '$C command "setblock $((X + 3)) $((Y + 2)) $Z minecraft:hopper[facing=down]{Items:[{Slot:0b,id:\\"minecraft:beef\\",count:4}]}" > /dev/null; '
               '$C command "setblock $((X + 3)) $Y $Z minecraft:hopper[facing=east]" > /dev/null; '
               '$C command "setblock $((X + 4)) $Y $Z minecraft:chest" > /dev/null; '
               '$C command "summon horizon_carts:barrel_cart $(awk "BEGIN{print $X + 3.5}") $((Y + 1)) $(awk "BEGIN{print $Z + 0.5}") {NoGravity:1b}" > /dev/null; '
               'for i in 1 2 3; do $C command "setblock $((X - 3)) $((Y + 1)) $Z minecraft:redstone_block" > /dev/null; sleep 0.6; '
               '$C command "setblock $((X - 3)) $((Y + 1)) $Z minecraft:air" > /dev/null; sleep 0.6; done; '
               '$C look-at-block $((X + 3)) $Y $Z > /dev/null; sleep 15; '
               '$C command "data get block $((X - 3)) $Y $Z Items" > $S/dropper.json; '
               '$C command "data get block $((X - 4)) $((Y - 2)) $Z Items" > $S/cabinet_chest.json; '
               '$C command "data get block $((X + 3)) $((Y + 2)) $Z Items" > $S/top_hopper.json; '
               '$C command "data get block $((X + 4)) $Y $Z Items" > $S/cart_chest.json'],
        checks=[check_file("dropper.json", r"block data: \[\]", "the dropper gave its three carrots"),
                check_file("cabinet_chest.json", r"count: 3, Slot: 0b, id: \\+\"minecraft:carrot", "through the cabinet into the chest"),
                check_file("top_hopper.json", r"block data: \[\]", "the hopper above emptied into the cart"),
                check_file("cart_chest.json", r"count: 4, Slot: 0b, id: \\+\"minecraft:beef", "the hopper under the cart took its four beef")],
        forbid=[r"a hopper's (push|pull)"],
    ),
    "fluids-flow": dict(
        about="A Java mod's fluids flow by their own code in the mirror (a beta tester's Create honey spread as "
              "lava): BuildCraft's oil spreads as oil, its gaseous fuel rises to a glass ceiling and spreads under it, "
              "water beside the oil makes no cobblestone",
        mods=["buildcraft"], scene="none",
        steps=['read X Y Z < <($C state | python3 -c "import json,sys,math;p=json.load(sys.stdin)[\'player\'][\'position\'];'
               'print(math.floor(p[0]),math.floor(p[1]),math.floor(p[2]))"); '
               '$C command "fill $((X - 8)) $((Y - 1)) $((Z - 8)) $((X + 8)) $((Y - 1)) $((Z + 8)) minecraft:stone" > /dev/null; '
               '$C command "fill $((X - 8)) $Y $((Z - 8)) $((X + 8)) $((Y + 12)) $((Z + 8)) minecraft:air" > /dev/null; '
               '$C command "fill $((X + 1)) $((Y + 8)) $((Z - 3)) $((X + 7)) $((Y + 8)) $((Z + 3)) minecraft:glass" > /dev/null; '
               '$C command "tp @s $((X - 4)) $((Y + 4)) $Z -90 0" > /dev/null; $C command "gamemode spectator" > /dev/null; '
               '$C command "setblock $((X + 4)) $Y $Z buildcraftenergy:fuel_gaseous" > /dev/null; '
               '$C command "setblock $((X + 4)) $Y $((Z + 6)) buildcraftenergy:oil" > /dev/null; sleep 12; '
               '$C block $((X + 4)) $((Y + 7)) $Z > $S/gas_top.json; $C block $((X + 8)) $Y $((Z + 6)) > $S/oil_far.json; '
               '$C command "setblock $((X - 2)) $Y $((Z + 6)) minecraft:water" > /dev/null; sleep 8; '
               '$C blocks $((X - 8)) $Y $((Z - 8)) $((X + 8)) $((Y + 7)) $((Z + 8)) > $S/blocks.json'],
        checks=[check_file("gas_top.json", r'"block":"buildcraftenergy:fuel_gaseous"', "the gas rose to the ceiling (BuildCraft's own physics)"),
                check_file("oil_far.json", r'"block":"buildcraftenergy:oil"', "the oil spread four blocks"),
                check_file("blocks.json", r'\A(?![\s\S]*minecraft:(lava|cobblestone|obsidian))', "no lava, cobblestone or obsidian"),
                check_file("blocks.json", r'"minecraft:water"', "the water flows too")],
        forbid=[r"failed its fluid tick"],
        look="nothing to see yet: mods' fluids aren't drawn (the gpu chat's half)",
    ),
    "eat": dict(
        about="Farmer's Delight's foods are eaten (a beta tester's bowl meal never was: the mirror started the "
              "use and nothing ticked it): a beef stew from hunger 0 feeds and leaves its bowl; cooked bacon feeds",
        mods=["fd"], scene="none",
        steps=['$C command "gamemode survival" >/dev/null; $C command "clear @s" >/dev/null; '
               '$C command "effect give @s minecraft:hunger 20 255" >/dev/null; sleep 8; '
               '$C command "effect clear @s" >/dev/null; $C state > $S/before.json',
               '$C command "give @s farmersdelight:beef_stew" >/dev/null; $C select 1 >/dev/null; $C look 0 -70 >/dev/null; '
               '$C mouse right down >/dev/null; sleep 3.5; $C mouse right up >/dev/null; sleep 1; '
               '$C state > $S/meal.json; $C inventory > $S/meal_inv.json',
               '$C command "give @s farmersdelight:cooked_bacon 3" >/dev/null; $C select 2 >/dev/null; '
               '$C mouse right down >/dev/null; sleep 1.2; $C mouse right up >/dev/null; sleep 1; '
               '$C state > $S/bacon.json; $C inventory > $S/bacon_inv.json'],
        checks=[check_json("before.json", ["player", "food"], 0, "starved first"),
                check_json("meal.json", ["player", "food"], 12, "the stew fed 12"),
                check_json("meal_inv.json", ["counts"], {"minecraft:bowl": 1}, "the stew is gone, its bowl left"),
                check_file("bacon_inv.json", r'"farmersdelight:cooked_bacon":2\b', "one bacon eaten"),
                check_file("bacon.json", r'"food":(1[3-9]|20)\b', "the bacon fed")],
        look="the bowl in hand",
    ),
    "tech": dict(
        about="BuildCraft's module blocks (buildcraftbuilders:quarry) and two Large Fluid Tanks placed; the game still answers",
        mods=["buildcraft", "fluidtank", "scala"], scene="none",
        steps=['$C command "setblock ~2 ~ ~ buildcraftbuilders:quarry" > $S/quarry.json; '
               '$C command "setblock ~2 ~ ~2 fluidtank:tank_iron" > $S/tank.json; '
               '$C command "setblock ~2 ~1 ~2 fluidtank:tank_iron" > /dev/null; sleep 5; '
               '$C command "time query gametime" > $S/after.json'],
        checks=[check_file("quarry.json", r"Changed the block", "BuildCraft's quarry is a block (a namespace besides its mod id)"),
                check_file("tank.json", r"Changed the block", "a Large Fluid Tank is placed"),
                check_file("after.json", r"game time is", "the server still answers after the tanks sync")],
        forbid=[r"Unknown block type"],
    ),
    "gridlod": dict(
        about="Grid World's far terrain: the grid's layers far away, not whole land",
        mods=["gridgen"], scene="none", env={"WORLD": "none", "GRAPHICS": str(TOOLS / "lod-graphics.txt")},
        steps=['"$R/tools/java_bridge/new_world.sh" "Grid World"',
               '$C command "gamemode spectator" >/dev/null; $C command "tp @s ~ 140 ~" >/dev/null; sleep 90; '
               '$C status > $S/status.json; $C look 30 25 >/dev/null; sleep 3'],
        checks=[check_file("status.json", r'"resident_quads":[1-9]', "far terrain is drawn"),
                check_log(r"the world's preset is gridgen:grid_world", "the world is Grid World's")],
        forbid=[r"far terrain's \S+ failed", r"worldgen: proto failed"],
        look="near: the grid; far: thin layers every 5 blocks, not solid land",
    ),
    "packs": dict(
        about="A pack's own resource packs (resourcepacks/ and options.txt beside mods-java) load: "
              "a checkered stone wall in the screenshot",
        mods=["fd"], optional=["fabric-api"], scene="none", resourcepacks=True,
        steps=['$C command "fill ~3 ~ ~-3 ~3 ~3 ~3 stone" >/dev/null; $C command "tp @s ~ ~ ~ -90 10" >/dev/null; sleep 2',
               'ls $S/.horizon-cache/resourcepacks/*/assets/minecraft/textures/block/stone.png > $S/packs.txt 2>&1; '
               '$C options-get maxFps > $S/fps.json'],
        checks=[check_file("packs.txt", r"resourcepacks/[0-9a-f]+/assets/minecraft/textures/block/stone\.png",
                           "the pack's stone is in the loaded resources"),
                check_json("fps.json", ["options", "maxFps", "value"], 30, "the test window is capped at 30 FPS")],
    ),
}


def test_pack(folder):
    """A pack's resource packs: `resourcepacks/Checkered.zip` (stone, dirt, sand... checkered
    magenta and yellow) in `folder`, selected in its `options.txt` as a launcher instance's is."""
    import struct, zipfile, zlib

    def png(a, b, n=16, cell=4):
        rows = b"".join(b"\0" + b"".join(bytes(a if (x // cell + y // cell) % 2 == 0 else b)
                                           for x in range(n)) for y in range(n))
        chunk = lambda t, d: struct.pack(">I", len(d)) + t + d + struct.pack(">I", zlib.crc32(t + d))
        return (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", n, n, 8, 6, 0, 0, 0))
                + chunk(b"IDAT", zlib.compress(rows)) + chunk(b"IEND", b""))
    (folder / "resourcepacks").mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(folder / "resourcepacks" / "Checkered.zip", "w") as z:
        z.writestr("pack.mcmeta", json.dumps({"pack": {"description": "Horizon test: checkered ground",
                                                       "min_format": 97, "max_format": 97}}))
        for name in ["stone", "dirt", "grass_block_side", "sand", "deepslate", "gravel", "andesite", "granite"]:
            z.writestr(f"assets/minecraft/textures/block/{name}.png", png((230, 0, 200, 255), (255, 220, 0, 255)))
    (folder / "options.txt").write_text('version:5023\nresourcePacks:["vanilla","fabric","file/Checkered.zip"]\n'
                                        'incompatibleResourcePacks:[]\n')


# ---- No code tied to mods' internals ---------------------------------------------------------
# Ludovic's condition (2026-11-04): the bridge's code names no mod's classes. A mod's package
# root (`me.shedaniel`, `journeymap`) in the bridge's code, outside comments, fails the run.
# Roots come from KNOWN_PACKAGES and from every jar in --jars/--pack (their fabric.mod.json
# entrypoints and mixin configs, nested jars too).

# Mods tested so far (so the check works with no jars at hand).
KNOWN_PACKAGES = [
    "me.shedaniel", "mezz.jei", "snownee", "squeek", "dev.architectury", "journeymap", "xaero",
    "xaeroplus", "com.mamiyaotaru", "fi.dy.masa", "com.simibubi", "vectorwing", "com.tom",
    "dan200.computercraft", "mcjty", "com.zurrtum", "dev.tr7zw", "net.bettercombat",
    "com.zigythebird", "dev.kosmx", "software.bernie", "biomesoplenty", "terrablender",
    "glitchcore", "com.replaymod", "com.moulberry.flashback", "org.popcraft.chunky",
    "dev.emi.trinkets", "com.tacz", "de.maxhenkel.voicechat", "com.unascribed",
    "tschipp.carryon", "sereneseasons", "com.blamejared.clumps", "com.direwolf20",
    "dev.lambdaurora", "com.terraformersmc", "net.puffish", "io.github.lgatodu47",
]
# Not mods: the platform the bridge implements, and its own code.
PLATFORM_PACKAGES = ("net.minecraft", "com.mojang", "net.fabricmc", "org.spongepowered",
                     "com.llamalad7", "net.neoforged", "dev.horizonlab", "java", "javax", "jdk",
                     "sun", "kotlin", "org.jetbrains", "com.google", "org.apache", "it.unimi",
                     "org.joml", "io.netty", "org.slf4j", "org.lwjgl", "org.objectweb")
# Mods' public APIs the bridge stands in for when the mod isn't installed (as it does for Fabric
# API): the API, never the mod's internals. Each needs a reason.
ALLOWED_APIS = {
    "com.terraformersmc.modmenu.api": "mods hand their settings screens to Mod Menu's API; Horizon's Mods page reads them",
}
TLDS = {"com", "net", "org", "dev", "io", "me", "de", "fi", "xyz", "uk", "ca", "top", "cn", "ru",
        "it", "nl", "fr", "jp", "br", "pl", "se", "no", "eu", "us", "gg", "info", "tech", "earth",
        "ml", "cc", "co", "tk", "at", "ch", "be", "dk", "au", "kr", "tw", "pro", "moe", "lol", "work"}
# The bridge's code: the JVM crate (not its test fixtures) and the game's java_* files.
BRIDGE_CODE = ["crates/horizon-jvm/src", "crates/horizon-jvm/build.rs", "crates/horizon-jvm/java/dev",
               "crates/horizon-jvm/java/platform", "crates/horizon-jvm/java/neoforge",
               "crates/horizon-client/src/**/java_*.rs", "crates/horizon-client-render/src/**/java_*.rs"]


def package_root(name):
    """`me.shedaniel.rei.Foo` -> `me.shedaniel`; `journeymap.client.X` -> `journeymap`;
    `io.github.someone.mod.X` -> `io.github.someone`."""
    parts = [p for p in re.split(r"[./$:]", name) if p]
    if len(parts) < 2:
        return None
    n = 1
    if parts[0] in TLDS:
        n = 3 if parts[1] in ("github", "gitlab", "codeberg") else 2
    if len(parts) <= n:  # a bare class in the root: keep what there is
        n = len(parts) - 1
    return ".".join(parts[:n]) if n else None


def jar_packages(jar):
    """Package roots of a mod jar's own code (entrypoints, mixin packages), nested jars too."""
    import io
    import zipfile
    roots = set()

    def walk(z):
        names = set(z.namelist())
        try:
            meta = json.loads(z.read("fabric.mod.json").decode("utf-8", "replace"), strict=False)
        except (KeyError, ValueError):
            meta = {}
        for points in (meta.get("entrypoints") or {}).values():
            for p in points if isinstance(points, list) else [points]:
                value = p.get("value") if isinstance(p, dict) else p
                if isinstance(value, str):
                    roots.add(package_root(value.split("::")[0]))
        for m in meta.get("mixins") or []:
            cfg = m.get("config") if isinstance(m, dict) else m
            if isinstance(cfg, str) and cfg in names:
                try:
                    pkg = json.loads(z.read(cfg).decode("utf-8", "replace"), strict=False).get("package")
                except ValueError:
                    pkg = None
                if pkg:
                    roots.add(package_root(pkg + ".X"))
        for n in names:
            if n.startswith("META-INF/jars/") and n.endswith(".jar"):
                try:
                    walk(zipfile.ZipFile(io.BytesIO(z.read(n))))
                except zipfile.BadZipFile:
                    pass

    try:
        walk(zipfile.ZipFile(jar))
    except (zipfile.BadZipFile, OSError):
        pass
    return {r for r in roots if r and not any(r == p or r.startswith(p + ".") for p in PLATFORM_PACKAGES)}


def strip_comments(text):
    """Java/Rust source without its comments (strings kept)."""
    out, i, n = [], 0, len(text)
    while i < n:
        c = text[i]
        if text.startswith("//", i):
            j = text.find("\n", i)
            i = n if j < 0 else j
        elif text.startswith("/*", i):
            j = text.find("*/", i + 2)
            out.append("\n" * text.count("\n", i, n if j < 0 else j))  # keep line numbers
            i = n if j < 0 else j + 2
        elif c == '"':
            j = i + 1
            while j < n and text[j] != '"':
                j += 2 if text[j] == "\\" else 1
            out.append(text[i:j + 1])
            i = j + 1
        elif c == "'" and re.match(r"'(\\.|[^\\'\n])'", text[i:i + 4]):  # a char literal ('"')
            m = re.match(r"'(\\.|[^\\'\n])'", text[i:i + 4])
            out.append(m.group(0))
            i += len(m.group(0))
        else:
            out.append(c)
            i += 1
    return "".join(out)


def mod_code_check(jar_folders):
    """(ok, lines): every place the bridge's code names a mod's package, outside comments."""
    roots = set(KNOWN_PACKAGES)
    for folder in jar_folders:
        for jar in sorted(Path(folder).glob("*.jar")) if folder else []:
            roots |= jar_packages(jar)
    pattern = re.compile(r"(?<![\w.$/])(" + "|".join(
        re.escape(r).replace(r"\.", "[./]") for r in sorted(roots, key=len, reverse=True)) + r")[./][A-Za-z_][\w./$]*")
    files = set()
    for spec in BRIDGE_CODE:
        for p in ROOT.glob(spec):
            files |= {p} if p.is_file() else {f for f in p.rglob("*") if f.is_file()}
    hits = []
    for f in sorted(files):
        if f.suffix not in (".java", ".rs", ".json", ".kt", ".txt", ".cfg", ".properties"):
            continue
        text = f.read_text(errors="replace")
        code = strip_comments(text) if f.suffix in (".java", ".rs", ".kt") else text
        for no, line in enumerate(code.splitlines(), 1):
            for m in pattern.finditer(line):
                full = m.group(0).replace("/", ".")
                if any(full == a or full.startswith(a + ".") for a in ALLOWED_APIS):
                    continue
                hits.append(f"{f.relative_to(ROOT)}:{no}: {m.group(1)}: {line.strip()[:140]}")
                break
    return not hits, hits, len(roots)


def find_jars(folder, names):
    """The jar of each name in `folder`; None when one is missing."""
    found = []
    for name in names:
        hit = None
        for jar in sorted(Path(folder).glob("*.jar")) if folder else []:
            if any(fnmatch.fnmatch(jar.name.lower(), g) for g in MODS[name]):
                hit = jar
                break
        if hit is None:
            return None, name
        found.append(hit)
    return found, None


def fixture(name, cache, against=()):
    """The fixture mod `name` built (once, again when its sources change), against the case's
    mod jars (`against`: a fixture that uses a library mod, GeckoLib)."""
    jar = cache / f"{name}.jar"
    # Made again when its sources changed since (a new mob in the critter fixture).
    sources = ROOT / "crates/horizon-jvm/java/fixtures" / name
    newest = max((f.stat().st_mtime for f in sources.rglob("*") if f.is_file()), default=0)
    if jar.exists() and jar.stat().st_mtime < newest:
        jar.unlink()
    if not jar.exists():
        env = dict(os.environ, FIXTURE_CP=":".join(str(j) for j in against))
        r = subprocess.run([str(TOOLS / "fixture_mod.sh"), name, str(jar)], capture_output=True, text=True, env=env)
    src = ROOT / "crates/horizon-jvm/java/fixtures" / name
    newest = max((f.stat().st_mtime for f in src.rglob("*") if f.is_file()), default=0)
    if not jar.exists() or jar.stat().st_mtime < newest:
        r = subprocess.run([str(TOOLS / "fixture_mod.sh"), name, str(jar)], capture_output=True, text=True)
        if r.returncode != 0 or not jar.exists():
            raise RuntimeError(f"fixture {name} did not build: {r.stderr[-400:]}")
    return jar


def build(java_only):
    env = os.environ.copy()
    if java_only:
        r = subprocess.run([str(TOOLS / "java.sh")], capture_output=True, text=True)
        if r.returncode != 0:
            sys.exit(f"Java build failed:\n{r.stderr[-2000:]}")
        return {"HORIZON_JAVA_JARS": r.stdout.strip().splitlines()[-1]}
    r = subprocess.run(["cargo", "build", "-q", "--profile", "fast", "-p", "horizon-client",
                        "-p", "horizon-client-lib", "--features", "horizon-client/java-mods"], cwd=ROOT, env=env,
                       capture_output=True, text=True)
    if r.returncode != 0:
        sys.exit(f"build failed:\n{r.stderr[-3000:]}")
    return {}


def read(path):
    return path.read_text(errors="replace") if path.exists() else ""


def run_game(case, name, jars, out, extra_env, pack=None, join_within=None):
    mods = Path(tempfile.mkdtemp(prefix="hz-regress-mods."))
    top = mods
    try:
        if case.get("resourcepacks"):
            # A pack's folder: mods-java/ with resourcepacks/ and options.txt beside it.
            test_pack(top)
            mods = top / "mods-java"
            mods.mkdir()
        for jar in jars:
            shutil.copy(jar, mods)
        config = {}
        if case.get("config"):
            # Mods' config files written before the game starts (launch.sh's CONFIG).
            folder = mods / "config"
            for rel, text in case["config"].items():
                (folder / rel).parent.mkdir(parents=True, exist_ok=True)
                (folder / rel).write_text(text)
            config = {"CONFIG": str(folder)}
        env = dict(os.environ, MODS=str(mods), SCENE=case["scene"], OUT=str(out), NO_BUILD="1", **extra_env,
                   **case.get("env", {}), **config)
        if pack is not None:
            # The pack's own folder (launch.sh takes its config/ too); its content kept between runs.
            env.update(MODS=str(pack), CACHE=str(out.parent / "pack-cache"),
                       HORIZON_CTL_LAUNCH_TIMEOUT=str(case.get("timeout", 600)))
        started = time.time()
        # A LAN case (lan=True): a host and a guest that joins it (lan_test.sh; steps see $H, $G).
        # A dedicated server case (server=True): a server and a player joining it (server_test.sh;
        # steps see $G, sv and $S, the server's folder; the log checked is the server's).
        script = "server_test.sh" if case.get("server") else "lan_test.sh" if case.get("lan") else "screen_test.sh"
        r = subprocess.run([str(TOOLS / script), *case["steps"]], env=env, capture_output=True,
                           text=True, timeout=case.get("timeout", 600) + 300)
        (out / "screen_test.txt").write_text(r.stdout + r.stderr)
        if case.get("server"):
            scratch = Path(Path("/tmp/hz-bridge-server").read_text().strip())
            log_path = scratch / "logs" / "latest.log"
        else:
            scratch = Path(Path("/tmp/hz-bridge-scratch").read_text().strip())
            log_path = scratch / "home" / "logs" / "latest.log"
        log = log_path.read_text(errors="replace") if log_path.exists() else ""
        shutil.copy(log_path, out / "latest.log") if log_path.exists() else None
        results = []
        for kind, file, pattern, what in case["checks"]:
            if kind == "log":
                ok = re.search(pattern, log) is not None
            elif kind == "file":
                text = (scratch / file).read_text(errors="replace") if (scratch / file).exists() else ""
                (out / file).write_text(text)
                ok = re.search(pattern, text) is not None
            else:
                path, expect = pattern
                value = None
                try:
                    text = (scratch / file).read_text()
                    (out / file).write_text(text)
                    value = json.loads(text)
                    for key in path:
                        value = value[key]
                    ok = value == expect
                except (OSError, ValueError, KeyError, IndexError, TypeError):
                    ok = False
                if not ok:
                    what = f"{what} (got {json.dumps(value)[:80]})"
            results.append((ok, what))
        if join_within is not None:
            joined = join_seconds(log)
            results.append((joined is not None and joined <= join_within,
                            f"a player joins within {join_within} s of the start"
                            + (f" ({joined} s)" if joined is not None else " (never)")))
        for pattern in case.get("forbid", []):
            hits = re.findall(rf".*{pattern}.*", log)
            results.append((not hits, f"no '{pattern}' in the log" + (f" ({hits[0].strip()[:120]})" if hits else "")))
        # The log's failures, for a reader (not a check: known noise stays).
        errors = [l[:220] for l in log.splitlines() if re.search(r"failed|Exception", l) and "content:" not in l]
        results.append((log != "", f"the game ran ({time.time() - started:.0f} s)"))
        return results, errors
    finally:
        shutil.rmtree(top, ignore_errors=True)


def join_seconds(log):
    """Seconds from the log's first line to the first player's join (None if none joined)."""
    stamps = re.findall(r"^\[(\d\d):(\d\d):(\d\d)\]", log, re.M)
    joined = re.search(r"^\[(\d\d):(\d\d):(\d\d)\].*System chat: \S+ joined the game", log, re.M)
    if not stamps or not joined:
        return None
    first = int(stamps[0][0]) * 3600 + int(stamps[0][1]) * 60 + int(stamps[0][2])
    at = int(joined.group(1)) * 3600 + int(joined.group(2)) * 60 + int(joined.group(3))
    return (at - first) % 86400


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("cases", nargs="*", help="cases to run (default: all)")
    ap.add_argument("--jars", default=os.environ.get("HZ_JARS"), help="a folder of Modrinth mod jars")
    ap.add_argument("--pack", default=os.environ.get("HZ_PACK"), help="a whole pack's mods folder (bigpack)")
    ap.add_argument("--join-within", type=int, default=int(os.environ.get("HZ_JOIN_WITHIN", "900")),
                    help="bigpack: seconds from the start to the player's join (default 900; the first run "
                         "of a set of jars also makes its content)")
    ap.add_argument("--out", default="/tmp/hz-regress")
    ap.add_argument("--java-only", action="store_true", help="rebuild only the Java side (java.sh)")
    ap.add_argument("--list", action="store_true")
    ap.add_argument("--code-check", action="store_true",
                    help="only check that the bridge's code names no mod's packages (no game run)")
    a = ap.parse_args()
    if a.list:
        for name, case in CASES.items():
            print(f"{name:10} {case['about']}")
        return 0
    names = a.cases or list(CASES)
    unknown = [n for n in names if n not in CASES]
    if unknown:
        sys.exit(f"unknown cases: {', '.join(unknown)} (--list)")
    out_root = Path(a.out)
    out_root.mkdir(parents=True, exist_ok=True)
    fixtures = out_root / "fixtures"
    fixtures.mkdir(exist_ok=True)
    ok, hits, roots = mod_code_check([a.jars, a.pack])
    print(f"---- nomodcode: the bridge's code names no mod's packages ({roots} package roots)")
    for h in hits:
        print(f"  FAIL {h}")
    if a.code_check:
        print("nomodcode:" + ("pass" if ok else "FAIL"))
        return 0 if ok else 1
    extra_env = build(a.java_only)
    summary = [("nomodcode", ok)]
    for name in names:
        case = CASES[name]
        out = out_root / name
        shutil.rmtree(out, ignore_errors=True)
        out.mkdir()
        if case.get("pack") and not (a.pack and list(Path(a.pack).glob("*.jar"))):
            print(f"SKIP {name}: no --pack folder of jars")
            summary.append((name, None))
            continue
        jars, missing = find_jars(a.jars, case.get("mods", []))
        if jars is None:
            print(f"SKIP {name}: no {missing} jar in {a.jars or '(no --jars)'}")
            summary.append((name, None))
            continue
        for opt in case.get("optional", []):
            more, _ = find_jars(a.jars, [opt])
            jars += more or []
        try:
            jars += [fixture(f, fixtures, list(jars)) for f in case.get("fixtures", [])]
            print(f"---- {name}: {case['about']}", flush=True)
            results, errors = run_game(case, name, jars, out, extra_env,
                                       pack=Path(a.pack).resolve() if case.get("pack") else None,
                                       join_within=a.join_within if case.get("pack") else None)
        except Exception as e:  # a case that cannot run fails, the others still run
            results, errors = [(False, f"the case could not run: {e}")], []
        for ok, what in results:
            print(f"  {'ok  ' if ok else 'FAIL'} {what}")
        if case.get("look"):
            shot = "server-1.png" if case.get("server") else f"{case['scene']}-0.png"
            print(f"  look {case['look']}: {out}/{shot}")
        if errors:
            print(f"  ({len(errors)} failure lines in the log: {out}/latest.log)")
        summary.append((name, all(ok for ok, _ in results)))
    print()
    print("  ".join(f"{n}:{'pass' if r else 'FAIL' if r is False else 'skip'}" for n, r in summary))
    return 1 if any(r is False for _, r in summary) else 0


if __name__ == "__main__":
    sys.exit(main())
