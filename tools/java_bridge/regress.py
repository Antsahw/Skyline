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
    "architectury": ["architectury*"],
    "cloth": ["cloth-config*", "cloth_config*"],
    "bop": ["biomes*o*plenty*", "biomesoplenty*"],
    "terrablender": ["terrablender*"],
    "glitchcore": ["glitchcore*"],
    "fabric-api": ["fabric-api*"],
    "veinminer": ["veinminer-fabric*"],
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
    "skyblock": ["skyblock-infinite*"],
    "simplyoneblock": ["simply_oneblock*", "simply-oneblock*"],
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
        steps=['$C click-at 141 187', '$C click-at 163 62', '$C state > $S/state.json; $C screen > $S/screen.json'],
        checks=[check_json("state.json", ["screen"], "container", "the pot's screen is open"),
                check_json("state.json", ["looking_at", "block", "block"], "farmersdelight:cooking_pot", "looking at the pot"),
                check_file("screen.json", screen_has(300, "minecraft:beef"), "beef is in the pot's first slot")],
    ),
    "terminal": dict(
        about="Tom's Storage: the terminal lists the chest beside its connector",
        mods=["toms"], scene="terminal",
        steps=['sleep 2; $C state > $S/state.json'],
        checks=[check_json("state.json", ["screen"], "container", "the terminal's screen is open"),
                check_log(r"Java mods: commands", "the mods started")],
        look="the terminal shows 12 diamonds and 40 oak logs",
    ),
    "ber": dict(
        about="Farmer's Delight's block entity renderers: beef on the stove, a carrot on the board",
        mods=["fd"], scene="ber",
        steps=[],
        checks=[check_log(r"Java mods: commands", "the mods started")],
        look="beef on the stove's grill, a carrot on the cutting board",
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
        about="REI's recipe view: vanilla and Farmer's Delight recipes",
        mods=["rei", "architectury", "cloth", "fd"], scene="none",
        steps=['$C command "recipes minecraft:crafting_table" >/dev/null; sleep 8',
               '$C key escape tap >/dev/null; $C command "recipes farmersdelight:beef_stew" >/dev/null; sleep 5'],
        checks=[check_log(r"recipes of 1 minecraft:crafting_table: opened", "crafting table's recipe shows"),
                check_log(r"recipes of 1 farmersdelight:beef_stew: opened", "beef stew's recipe shows")],
        forbid=[r"recipe viewer failed"],
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
                check_log(r"experience_orbs go to the mods", "orb pickup goes to the mods")],
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
        about="Better Combat: a click is its attack request; the cow takes the sword's hit",
        mods=["bettercombat", "cloth", "playeranim"], scene="fight", env={"HORIZON_JAVA_DEBUG": "combat"},
        steps=['$C mouse left click >/dev/null; sleep 1.5; $C entities 6 minecraft:cow > $S/cow.json'],
        checks=[check_file("cow.json", r'"health":[0-4]\.', "the cow lost 6 health"),
                check_log(r"combat: Better Combat combo 0", "Better Combat's attack ran")],
    ),
    "ride": dict(
        about="Entity.startRiding's head (mixinmod: no minecarts): the cart refuses, the boat takes the player",
        fixtures=["mixinmod"], scene="ride",
        steps=['$C look-at $(cat $S/cart.pos) >/dev/null; sleep 0.5; $C mouse right click >/dev/null; sleep 1; $C status > $S/cart.json',
               '$C look-at $(cat $S/boat.pos) >/dev/null; sleep 0.5; $C mouse right click >/dev/null; sleep 1; $C status > $S/boat.json'],
        checks=[check_json("cart.json", ["riding"], None, "not in the minecart"),
                check_file("boat.json", r'"riding":\{|"riding":[0-9]', "in the boat")],
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
    "bigpack": dict(
        about="A whole pack (--pack): Java starts, the registries load, a player joins in time, the mods' worldgen runs",
        pack=True, scene="none", timeout=2700,
        steps=['sleep 20; $C state > $S/state.json'],
        checks=[check_log(r"Fabric started in", "Java and Fabric start"),
                check_log(r"System chat: \S+ joined the game", "a player joins"),
                check_file("state.json", r'"block_pos":\[', "the player is in the world")],
        forbid=[r"Java mods: not started", r"Registry loading errors", r"their content was not made",
                r"Java mods' worldgen: chunk -?\d+ -?\d+ failed", r"control socket did not open",
                r"fell out of the world"],
        look="the pack's world, its HUD (minimap) drawn",
    ),
    "prism": dict(
        about="Prism Launcher's wrapper: an instance's mods linked, Fabric API left out (no game)",
        mods=["fd"], optional=["fabric-api"], prism=True,
    ),
}


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


def fixture(name, cache):
    jar = cache / f"{name}.jar"
    if not jar.exists():
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
                        "--features", "horizon-client/java-mods"], cwd=ROOT, env=env,
                       capture_output=True, text=True)
    if r.returncode != 0:
        sys.exit(f"build failed:\n{r.stderr[-3000:]}")
    return {}


def run_prism(case, jars, out):
    inst = Path(tempfile.mkdtemp(prefix="hz-prism."))
    try:
        mods = inst / ".minecraft" / "mods"
        mods.mkdir(parents=True)
        for jar in jars:
            shutil.copy(jar, mods)
        env = dict(os.environ, INST_MC_DIR=str(inst / ".minecraft"), HORIZON_PRISM_DRY="1")
        r = subprocess.run([str(TOOLS / "prism.sh")], input="param --username\nparam Steve\nlaunch\n",
                           env=env, capture_output=True, text=True, timeout=60)
        (out / "prism.txt").write_text(r.stdout + r.stderr)
        linked = sorted(p.name for p in (inst / ".minecraft" / "horizon" / "mods-java").glob("*.jar"))
        results = [(r.returncode == 0 and "horizon-client" in r.stdout, "the game's command is made"),
                   (len(linked) == len([j for j in jars if not fnmatch.fnmatch(j.name.lower(), "fabric-api*")]),
                    f"the mods are linked ({len(linked)})"),
                   (not any(n.lower().startswith("fabric-api") for n in linked), "Fabric API is left out"),
                   ((inst / ".minecraft" / "horizon" / "config").is_symlink(), "config/ is the instance's")]
        return results, []
    finally:
        shutil.rmtree(inst, ignore_errors=True)


def run_game(case, name, jars, out, extra_env, pack=None, join_within=None):
    mods = Path(tempfile.mkdtemp(prefix="hz-regress-mods."))
    try:
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
        r = subprocess.run([str(TOOLS / "screen_test.sh"), *case["steps"]], env=env, capture_output=True,
                           text=True, timeout=case.get("timeout", 600) + 300)
        (out / "screen_test.txt").write_text(r.stdout + r.stderr)
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
                try:
                    value = json.loads((scratch / file).read_text())
                    for key in path:
                        value = value[key]
                    ok = value == expect
                except (OSError, ValueError, KeyError, IndexError, TypeError):
                    ok = False
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
        shutil.rmtree(mods, ignore_errors=True)


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
    extra_env = build(a.java_only)
    summary = []
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
            jars += [fixture(f, fixtures) for f in case.get("fixtures", [])]
            print(f"---- {name}: {case['about']}", flush=True)
            if case.get("prism"):
                results, errors = run_prism(case, jars, out)
            else:
                results, errors = run_game(case, name, jars, out, extra_env,
                                           pack=Path(a.pack).resolve() if case.get("pack") else None,
                                           join_within=a.join_within if case.get("pack") else None)
        except Exception as e:  # a case that cannot run fails, the others still run
            results, errors = [(False, f"the case could not run: {e}")], []
        for ok, what in results:
            print(f"  {'ok  ' if ok else 'FAIL'} {what}")
        if case.get("look"):
            print(f"  look {case['look']}: {out}/{case['scene']}-0.png")
        if errors:
            print(f"  ({len(errors)} failure lines in the log: {out}/latest.log)")
        summary.append((name, all(ok for ok, _ in results)))
    print()
    print("  ".join(f"{n}:{'pass' if r else 'FAIL' if r is False else 'skip'}" for n, r in summary))
    return 1 if any(r is False for _, r in summary) else 0


if __name__ == "__main__":
    sys.exit(main())
