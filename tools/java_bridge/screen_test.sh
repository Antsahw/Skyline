#!/bin/bash
# Rebuilds, starts the game with MODS, sets up a scene (SCENE: a name below), then runs each
# argument as a step, a screenshot after each in OUT (default /tmp): <scene>-N.png. Prints the
# log's failures. Steps see C (horizon-ctl). Click at GUI pixels: '$C click-at X Y'.
#   MODS=~/fd SCENE=pot   tools/java_bridge/screen_test.sh '$C click-at 141 187'
#   MODS=~/toms SCENE=terminal tools/java_bridge/screen_test.sh '$C click-at 149 45'
#   MODS=~/critters SCENE=entity tools/java_bridge/screen_test.sh '$C mouse left click'
#   MODS=~/nat SCENE=entity ENTITY=naturalist:bear tools/java_bridge/screen_test.sh
#   MODS=~/cl SCENE=loader tools/java_bridge/screen_test.sh '$C click-at 213 120'
#   JAVA_ONLY=1 ... : after a Java-only change (no game rebuild; see java.sh)
# Steps see S (the scratch folder) too: '$C state > $S/state.json' keeps what a check reads.
R=$(cd "$(dirname "$0")/../.." && pwd)
: "${SCENE:=pot}" "${OUT:=/tmp}"
# Java 25 (26.3's classes are version 69): JAVA_HOME if it is one, else the first installed.
if ! "${JAVA_HOME:-/nonexistent}/bin/java" -version 2>&1 | grep -q '"25'; then
  JAVA_HOME=$(ls -d /usr/lib/jvm/*25* 2>/dev/null | head -1)
fi
[ -n "$JAVA_HOME" ] || { echo "no Java 25 in /usr/lib/jvm: set JAVA_HOME" >&2; exit 1; }
export JAVA_HOME HORIZON_JAVA_HOME=$JAVA_HOME PATH=$JAVA_HOME/bin:$PATH
old=$(cat /tmp/hz-bridge-scratch 2>/dev/null)
[ -n "$old" ] && "$R/target/fast/horizon-ctl" --socket "$old/ctl.sock" quit >/dev/null 2>&1 \
  && { rm -rf "$old" 2>/dev/null || { sleep 3; rm -rf "$old" 2>/dev/null; }; }
# JAVA_ONLY=1: a Java-only change; just the jars are rebuilt (seconds) and run from disk.
# NO_BUILD=1: the caller built already (regress.py: once for every scene).
if [ -n "$NO_BUILD" ]; then
  :
elif [ -n "$JAVA_ONLY" ]; then
  jars=$("$R/tools/java_bridge/java.sh") || exit 1  # (export X=$(...) would hide its failure)
  export HORIZON_JAVA_JARS=$jars
else
  cd "$R" && cargo build -q --profile fast -p horizon-client -p horizon-client-lib --features horizon-client/java-mods 2>&1 \
    | grep -vE "Picked up JAVA_TOOL" | grep -E "^error|error:|\.java:[0-9]+" -A6 | head -30
fi
S=$("$R/tools/java_bridge/launch.sh") || exit 1
C="$R/target/fast/horizon-ctl --socket $S/ctl.sock"
case "$SCENE" in
  # Farmer's Delight: a lit stove, a cooking pot on it, beef and carrots (an emptied
  # inventory first: seeds the scene's setblocks drop may be picked up); the pot opened.
  pot) "$R/tools/java_bridge/around.sh" \
    'q "setblock $((X+2)) $Y $Z farmersdelight:stove[facing=west,lit=true]"' \
    'q "setblock $((X+2)) $((Y+1)) $Z farmersdelight:cooking_pot[facing=west,support=tray]"' \
    'q "clear @s"' 'q "give @s minecraft:beef 3"' 'q "give @s minecraft:carrot 2"' \
    '$C look-at-block $((X+2)) $((Y+1)) $Z >/dev/null' '$C mouse right click >/dev/null' 'sleep 3' ;;
  # Tom's Storage: a chest (12 diamonds, 40 logs), a connector beside it, a terminal on the
  # connector facing it (pos=down: it reads the block below); the terminal opened.
  terminal) "$R/tools/java_bridge/around.sh" \
    'q "setblock $((X+3)) $Y $Z minecraft:chest"' \
    'q "item replace block $((X+3)) $Y $Z container.0 with minecraft:diamond 12"' \
    'q "item replace block $((X+3)) $Y $Z container.1 with minecraft:oak_log 40"' \
    'q "setblock $((X+2)) $Y $Z toms_storage:inventory_connector"' \
    'q "setblock $((X+2)) $((Y+1)) $Z toms_storage:storage_terminal[facing=west,pos=down]"' \
    '$C look-at-block $((X+2)) $((Y+1)) $Z >/dev/null' '$C mouse right click >/dev/null' 'sleep 3' ;;
  # A pack (docs/java-bridge.md, "A 20-40 mod pack"): a row of blocks from its content mods,
  # looked at from three blocks away; then a waystone used (its screen).
  pack) "$R/tools/java_bridge/around.sh" \
    'q "setblock $((X+3)) $Y $((Z-2)) biomesoplenty:redwood_log"' \
    'q "setblock $((X+3)) $Y $((Z-1)) biomesoplenty:jacaranda_leaves"' \
    'q "setblock $((X+3)) $Y $Z waystones:sandstone_waystone"' \
    'q "setblock $((X+3)) $Y $((Z+1)) farmersdelight:stove"' \
    'q "setblock $((X+3)) $Y $((Z+2)) toms_storage:inventory_connector"' \
    'q "setblock $((X+3)) $((Y+1)) $((Z-2)) biomesoplenty:blue_hydrangea"' \
    'q "setblock $((X+3)) $((Y+1)) $((Z+2)) farmersdelight:cooking_pot"' \
    '$C look-at-block $((X+3)) $Y $Z >/dev/null' 'sleep 2' ;;
  # Mods' entities: two of ENTITY (default the critter fixture: tools/java_bridge/fixture_mod.sh
  # crittermod) summoned ahead, the player facing them; one standing still (NoAI) under the crosshair.
  entity) "$R/tools/java_bridge/around.sh" \
    'q "summon ${ENTITY:-horizon_critters:critter} ~3 ~ ~2"' \
    'q "summon ${ENTITY:-horizon_critters:critter} ~2 ~ ~ {NoAI:1b}"' \
    'q "tp @s ~ ~ ~ -90 25"' 'sleep 3' ;;
  # Hostile mods' mobs: a biter (the critter fixture's hostile one, 3 a bite) ahead of a
  # player in survival, on normal difficulty; it comes and bites.
  hostile) "$R/tools/java_bridge/around.sh" \
    'q "difficulty normal"' 'q "gamemode survival"' \
    'q "summon horizon_critters:biter ~4 ~ ~1"' \
    'q "tp @s ~ ~ ~ -90 15"' 'sleep 2' ;;
  # Riding a mod's mob: a mount (the critter fixture's rideable one: it stays put until
  # ridden) two blocks east of the player, its middle in $S/mount.pos, on a flat stone lane
  # 24 blocks east (the floor's height, $S/floor.y, is the lane's; the case drives 0.8 s,
  # ~11 blocks, short of its end).
  mount) "$R/tools/java_bridge/around.sh" \
    'q "fill $((X-1)) $((Y-1)) $((Z-2)) $((X+24)) $((Y-1)) $((Z+2)) stone"' \
    'q "fill $((X-1)) $Y $((Z-2)) $((X+24)) $((Y+4)) $((Z+2)) air"' \
    'q "summon horizon_critters:mount $(p $X+2.5) $Y $(p $Z+0.5)"' \
    'echo "$(p $X+2.5) $(p $Y+0.5) $(p $Z+0.5)" > "$S/mount.pos"' 'echo "$Y" > "$S/floor.y"' \
    'q "tp @s $(p $X+0.5) $Y $(p $Z+0.5) -90 20"' 'sleep 2' ;;
  # Horizon's iron golem ahead of the player in a 9x9 pen (2-high walls: it stays), with the
  # critter a case summons ($S/critter.pos, four blocks from it).
  golem) "$R/tools/java_bridge/around.sh" \
    'q "fill $((X-4)) $Y $((Z-4)) $((X+4)) $((Y+1)) $((Z-4)) stone_bricks"' \
    'q "fill $((X-4)) $Y $((Z+4)) $((X+4)) $((Y+1)) $((Z+4)) stone_bricks"' \
    'q "fill $((X-4)) $Y $((Z-4)) $((X-4)) $((Y+1)) $((Z+4)) stone_bricks"' \
    'q "fill $((X+4)) $Y $((Z-4)) $((X+4)) $((Y+1)) $((Z+4)) stone_bricks"' \
    'q "summon minecraft:iron_golem $(p $X+2.5) $Y $(p $Z-1.5)"' \
    'echo "$(p $X+2.5) $Y $(p $Z+2.5)" > "$S/critter.pos"' \
    'q "tp @s $(p $X-2.5) $Y $(p $Z+0.5) -90 20"' 'sleep 1' ;;
  # A pen with no golem (Horizon's projectiles on mods' mobs): where a mod's mob goes, four
  # blocks ahead of the player, in $S/critter.pos (the case summons it).
  range) "$R/tools/java_bridge/around.sh" \
    'q "fill $((X-4)) $Y $((Z-4)) $((X+4)) $((Y+1)) $((Z-4)) stone_bricks"' \
    'q "fill $((X-4)) $Y $((Z+4)) $((X+4)) $((Y+1)) $((Z+4)) stone_bricks"' \
    'q "fill $((X-4)) $Y $((Z-4)) $((X-4)) $((Y+1)) $((Z+4)) stone_bricks"' \
    'q "fill $((X+4)) $Y $((Z-4)) $((X+4)) $((Y+1)) $((Z+4)) stone_bricks"' \
    'echo "$(p $X+1.5) $Y $(p $Z+0.5)" > "$S/critter.pos"' \
    'q "tp @s $(p $X-2.5) $Y $(p $Z+0.5) -90 20"' 'sleep 1' ;;
  # Mods' mobs and Horizon's: one of Horizon's sheep (no AI: it stays) ahead of the player;
  # where a hunter (the critter fixture's, it hunts sheep) goes, four blocks from it, in
  # $S/hunter.pos (the case summons it).
  neighbours) "$R/tools/java_bridge/around.sh" \
    'q "summon minecraft:sheep $(p $X+3.5) $Y $(p $Z+2.5) {NoAI:1b}"' \
    'echo "$(p $X+3.5) $Y $(p $Z-1.5)" > "$S/hunter.pos"' \
    'q "tp @s ~ ~ ~ -90 25"' 'sleep 1' ;;
  # Farmer's Delight's block entity renderers: beef on a lit stove's grill, a carrot on a
  # cutting board beside it; the player looking at both.
  ber) "$R/tools/java_bridge/around.sh" \
    'q "setblock $((X+2)) $Y $Z farmersdelight:stove[facing=west,lit=true]"' \
    'q "setblock $((X+2)) $Y $((Z+1)) farmersdelight:cutting_board[facing=west]"' \
    'q "item replace entity @s hotbar.0 with minecraft:beef 3"' \
    'q "item replace entity @s hotbar.1 with minecraft:carrot 2"' \
    'q "tp @s $(p $X+0.5) $Y $(p $Z+1.5)"' \
    '$C key 1 tap >/dev/null; $C look-at $(p $X+2.5) $((Y+1)) $(p $Z+0.5) >/dev/null' '$C mouse right click >/dev/null' \
    '$C key 2 tap >/dev/null; $C look-at $(p $X+2.5) $(p $Y+0.06) $(p $Z+1.5) >/dev/null' '$C mouse right click >/dev/null' \
    '$C look-at $(p $X+2.5) $Y $(p $Z+0.9) >/dev/null' 'sleep 1' ;;
  # Chunk Loaders: a single chunk loader placed by the player (its owner) on the floor ahead,
  # then opened (its chunk grid).
  loader) "$R/tools/java_bridge/around.sh" \
    'q "gamemode survival"' 'q "give @s chunkloaders:single_chunk_loader"' \
    '$C look-at-block $((X+2)) $((Y-1)) $Z >/dev/null' '$C mouse right click >/dev/null' \
    '$C look-at-block $((X+2)) $Y $Z >/dev/null' '$C mouse right click >/dev/null' 'sleep 3' ;;
  # Breaking by hand: dirt two blocks east and an oak log beside it, survival, looking at the
  # dirt; their places are written to $S/dig.pos (dirt, then the log).
  dig) "$R/tools/java_bridge/around.sh" \
    'q "gamemode survival"' 'q "setblock $((X+2)) $Y $Z minecraft:dirt"' \
    'q "setblock $((X+2)) $Y $((Z+1)) minecraft:oak_log"' \
    'echo "$((X+2)) $Y $Z $((X+2)) $Y $((Z+1))" > "$S/dig.pos"' \
    '$C look-at-block $((X+2)) $Y $Z >/dev/null' ;;
  # Veinminer: a 7-ore iron vein three blocks east (a 3x2 wall and one on top), survival, a
  # diamond pickaxe, looking at the vein's middle; the region is written to $S/vein.pos.
  vein) "$R/tools/java_bridge/around.sh" \
    'q "fill $((X+3)) $Y $((Z-1)) $((X+3)) $((Y+1)) $((Z+1)) minecraft:iron_ore"' \
    'q "setblock $((X+3)) $((Y+2)) $Z minecraft:iron_ore"' \
    'q "gamemode survival"' 'q "give @s minecraft:diamond_pickaxe"' 'q "tp @s ~ ~ ~ -90 0"' \
    'echo "$((X+3)) $Y $((Z-1)) $((X+3)) $((Y+2)) $((Z+1))" > "$S/vein.pos"' \
    '$C look-at-block $((X+3)) $((Y+1)) $Z west >/dev/null' ;;
  # Right Click Harvest: a row of ripe wheat on wet farmland two blocks east, survival,
  # looking at the middle one.
  harvest) "$R/tools/java_bridge/around.sh" \
    'q "fill $((X+2)) $((Y-1)) $((Z-1)) $((X+2)) $((Y-1)) $((Z+1)) minecraft:farmland[moisture=7]"' \
    'q "fill $((X+2)) $Y $((Z-1)) $((X+2)) $Y $((Z+1)) minecraft:wheat[age=7]"' \
    'q "gamemode survival"' 'q "tp @s ~ ~ ~ -90 30"' \
    'echo "$((X+2)) $Y $Z" > "$S/crop.pos"' '$C look-at-block $((X+2)) $Y $Z west >/dev/null' ;;
  # Carry On: a cow standing still two blocks east, survival, empty hands, looking at it.
  carry) "$R/tools/java_bridge/around.sh" \
    'q "summon minecraft:cow ~2 ~ ~ {NoAI:1b,CustomName:\"Bessie\"}"' 'q "gamemode survival"' \
    'q "tp @s ~ ~ ~ -90 20"' 'sleep 1' \
    '$C look-at-entity $($C entities 6 minecraft:cow | python3 -c "import json,sys;print(json.load(sys.stdin)[\"entities\"][0][\"id\"])") >/dev/null' ;;
  # Serene Seasons: a wheat seedling on wet farmland under the sky (glass over it would be
  # a greenhouse: fertile in every season); its position in $S/crop.pos.
  seasons) "$R/tools/java_bridge/around.sh" \
    'q "setblock $((X+3)) $((Y-1)) $Z minecraft:farmland[moisture=7]"' \
    'q "setblock $((X+3)) $((Y-1)) $((Z+1)) minecraft:water"' \
    'q "setblock $((X+3)) $Y $Z minecraft:wheat[age=0]"' \
    'echo "$((X+3)) $Y $Z" > "$S/crop.pos"' ;;
  # A fight: a cow and a pig standing still ahead (two blocks east, a block apart), survival,
  # an iron sword in hand, looking at the cow; their ids in $S/cow.id and $S/pig.id.
  fight) "$R/tools/java_bridge/around.sh" \
    'q "gamemode survival"' 'q "item replace entity @s weapon.mainhand with minecraft:iron_sword"' \
    'q "summon minecraft:cow ~2.5 ~ ~-0.6 {NoAI:1b}"' 'q "summon minecraft:pig ~2.5 ~ ~1.4 {NoAI:1b}"' \
    'q "tp @s ~ ~ ~ -90 15"' 'sleep 1' \
    '$C entities 6 minecraft:cow | python3 -c "import json,sys;print(json.load(sys.stdin)[\"entities\"][0][\"id\"])" > "$S/cow.id"' \
    '$C entities 6 minecraft:pig | python3 -c "import json,sys;print(json.load(sys.stdin)[\"entities\"][0][\"id\"])" > "$S/pig.id"' \
    '$C look-at-entity $(cat "$S/cow.id") >/dev/null' ;;
  # A sweep: three zombies standing still in a row two and a half blocks east (z -1, 0, +1),
  # fire resistant (daylight), survival, an iron sword in hand, looking at the middle one.
  sweep) "$R/tools/java_bridge/around.sh" \
    'q "gamemode survival"' 'q "item replace entity @s weapon.mainhand with minecraft:iron_sword"' \
    'q "summon minecraft:zombie ~2.5 ~ ~-1 {NoAI:1b}"' 'q "summon minecraft:zombie ~2.5 ~ ~ {NoAI:1b}"' \
    'q "summon minecraft:zombie ~2.5 ~ ~1 {NoAI:1b}"' 'q "effect give @e[type=minecraft:zombie,distance=..6] minecraft:fire_resistance infinite 0 true"' \
    'q "tp @s ~ ~ ~ -90 10"' 'sleep 1' \
    '$C look-at-entity $($C entities 6 minecraft:zombie | python3 -c "import json,sys;print(json.load(sys.stdin)[\"entities\"][0][\"id\"])") >/dev/null' ;;
  # Vehicles: a minecart two blocks north, a boat two blocks south; where they are in
  # $S/cart.pos and $S/boat.pos (points to look at).
  ride) "$R/tools/java_bridge/around.sh" \
    'q "summon minecraft:minecart $(p $X+0.5) $Y $(p $Z-1.5)"' 'q "summon minecraft:oak_boat $(p $X+0.5) $Y $(p $Z+2.5)"' \
    'echo "$(p $X+0.5) $(p $Y+0.3) $(p $Z-1.5)" > "$S/cart.pos"' 'echo "$(p $X+0.5) $(p $Y+0.25) $(p $Z+2.5)" > "$S/boat.pos"' ;;
  # Precipitation: rain, a 9x9 stone floor under the sky (around.sh's), its middle column in
  # $S/floor.pos; seasons and tick speed are the case's.
  snow) "$R/tools/java_bridge/around.sh" \
    'q "weather rain"' 'echo "$((X-4)) $Y $((Z-4)) $((X+4)) $Y $((Z+4))" > "$S/floor.pos"' ;;
  # The mixinmod fixture's first round: dirt two blocks east to break (looking at it), and a
  # wheat seedling shut in stone to the west, lit 8 by a light block beside it (enough to
  # stay, one short of growing); its position in $S/crop.pos.
  mixins1) "$R/tools/java_bridge/around.sh" \
    'q "gamemode survival"' 'q "setblock $((X+2)) $Y $Z minecraft:dirt"' \
    'q "fill $((X-4)) $Y $((Z-2)) $((X-2)) $((Y+2)) $((Z+2)) minecraft:stone"' \
    'q "setblock $((X-3)) $((Y+1)) $((Z+1)) minecraft:light[level=9]"' \
    'q "setblock $((X-3)) $Y $Z minecraft:farmland[moisture=7]"' 'q "setblock $((X-3)) $((Y+1)) $Z minecraft:wheat[age=0]"' \
    'echo "$((X-3)) $((Y+1)) $Z" > "$S/crop.pos"' '$C look-at-block $((X+2)) $Y $Z west >/dev/null' ;;
  # Flowers to punch, survival: a poppy three blocks east, a dandelion beside it (z+2), on grass;
  # looking at the poppy. Their positions in $S/poppy.pos and $S/dandelion.pos.
  punch) "$R/tools/java_bridge/around.sh" \
    'q "tp @s $(p $X+0.5) $Y $(p $Z+0.5) -90 30"' 'q "gamemode survival"' 'q "fill $((X+3)) $((Y-1)) $Z $((X+3)) $((Y-1)) $((Z+2)) minecraft:grass_block"' \
    'q "setblock $((X+3)) $Y $Z minecraft:poppy"' 'q "setblock $((X+3)) $Y $((Z+2)) minecraft:dandelion"' \
    'echo "$((X+3)) $Y $Z" > "$S/poppy.pos"' 'echo "$((X+3)) $Y $((Z+2))" > "$S/dandelion.pos"' \
    '$C look-at-block $((X+3)) $Y $Z west >/dev/null' ;;
  # A one-level beacon three blocks east (iron 3x3 under it, open sky above), survival; the
  # beacon's position in $S/beacon.pos (its power is the steps').
  beacon) "$R/tools/java_bridge/around.sh" \
    'q "gamemode survival"' 'q "fill $((X+2)) $Y $((Z-1)) $((X+4)) $Y $((Z+1)) minecraft:iron_block"' \
    'q "fill $((X+3)) $((Y+1)) $Z $((X+3)) 319 $Z minecraft:air"' \
    'echo "$((X+3)) $((Y+1)) $Z" > "$S/beacon.pos"' '$C look-at $(p $X+3.5) $(p $Y+1.5) $(p $Z+0.5) >/dev/null' ;;
  # A flat floor (around.sh's), survival, the player at its middle looking east; a trial
  # spawner (a zombie's) goes three blocks east in the steps: its position in $S/spawner.pos.
  omen) "$R/tools/java_bridge/around.sh" \
    'q "tp @s $(p $X+0.5) $Y $(p $Z+0.5) -90 0"' 'q "gamemode survival"' \
    'echo "$((X+3)) $Y $Z" > "$S/spawner.pos"' ;;

  # CC: Tweaked: an advanced computer two blocks east, opened (its terminal screen).
  computer) "$R/tools/java_bridge/around.sh" \
    'q "setblock $((X+2)) $Y $Z computercraft:computer_advanced[facing=west]"' \
    '$C look-at-block $((X+2)) $Y $Z >/dev/null' '$C mouse right click >/dev/null' 'sleep 6' ;;
  # Create: a creative motor (16 rpm) driving a shaft, a cogwheel and a shaft westward,
  # looked at from the south-west (the cog's face toward the camera).
  create) "$R/tools/java_bridge/around.sh" \
    'q "setblock $((X+4)) $Y $((Z-3)) create:creative_motor[facing=west]"' \
    'q "setblock $((X+3)) $Y $((Z-3)) create:shaft[axis=x]"' \
    'q "setblock $((X+2)) $Y $((Z-3)) create:cogwheel[axis=x]"' \
    'q "setblock $((X+1)) $Y $((Z-3)) create:shaft[axis=x]"' \
    'q "tp @s ~ ~ ~ -145 30"' 'sleep 3' ;;
  # Mods' fluids (FLUIDS: block ids, default Create's honey and chocolate): a source of each
  # sunk in a hole of the floor ahead (they can't spread), a stone wall behind, looked down at.
  fluids) "$R/tools/java_bridge/around.sh" \
    'q "fill $((X+4)) $Y $((Z-3)) $((X+4)) $((Y+1)) $((Z+3)) stone_bricks"' \
    'n=0; for f in ${FLUIDS:-create:honey create:chocolate}; do q "setblock $((X+2)) $((Y-1)) $((Z-1+n*2)) $f"; n=$((n+1)); done' \
    'q "tp @s ~ ~ ~ -90 50"' 'sleep 3' ;;
  # Create's machines, each on its own creative motor (16 rpm), in a row east of the player:
  # a press two blocks over a depot (north: its item is put on with a click, a step), a
  # millstone under a hopper of wheat, a deployer under a hopper of cobblestone, over air it
  # places them in (south); west, a belt's two shafts
  # (the belt itself is made with its item, a step).
  machines) "$R/tools/java_bridge/around.sh" \
    'q "setblock $((X+3)) $((Y+2)) $((Z-3)) create:creative_motor[facing=south]"' \
    'q "setblock $((X+3)) $((Y+2)) $((Z-2)) create:mechanical_press[facing=south]"' \
    'q "setblock $((X+3)) $Y $((Z-2)) create:depot"' \
    'q "setblock $((X+3)) $Y $Z create:creative_motor[facing=up]"' \
    'q "setblock $((X+3)) $((Y+1)) $Z create:millstone"' \
    'q "setblock $((X+3)) $((Y+2)) $Z minecraft:hopper[facing=down]"' \
    'q "item replace block $((X+3)) $((Y+2)) $Z container.0 with minecraft:wheat 16"' \
    'q "setblock $((X+3)) $((Y+1)) $((Z+2)) create:creative_motor[facing=south]"' \
    'q "setblock $((X+3)) $((Y+1)) $((Z+3)) create:deployer[facing=down,axis_along_first=false]"' \
    'q "setblock $((X+3)) $((Y+2)) $((Z+3)) minecraft:hopper[facing=down]"' \
    'q "item replace block $((X+3)) $((Y+2)) $((Z+3)) container.0 with minecraft:cobblestone 8"' \
    'q "setblock $((X-3)) $Y $((Z-3)) create:creative_motor[facing=east]"' \
    'q "setblock $((X-2)) $Y $((Z-3)) create:shaft[axis=x]"' \
    'q "setblock $((X-2)) $Y $((Z+1)) create:shaft[axis=x]"' \
    'echo $X $Y $Z > $S/pos' 'q "tp @s ~ ~ ~ -90 25"' 'sleep 3' ;;
  # A Create contraption: a mechanical bearing (on a creative motor) with planks and a log on
  # it, east of the player; a step assembles it (a right-click on the bearing, empty hand).
  contraption) "$R/tools/java_bridge/around.sh" \
    'q "setblock $((X+3)) $Y $Z create:creative_motor[facing=up]"' \
    'q "setblock $((X+3)) $((Y+1)) $Z create:mechanical_bearing[facing=up]"' \
    'q "setblock $((X+3)) $((Y+2)) $Z minecraft:oak_planks"' \
    'echo $X $Y $Z > $S/pos' 'q "tp @s ~ ~ ~ -90 10"' 'sleep 3' ;;
  # Create, clicks a block takes first and timed use (bridge-mixins-11): a depot two blocks
  # ahead-right, two chain conveyors six apart behind it; the player in survival, its block in
  # $S/pos. The case places tracks with their item (their railway graph is made in onPlace).
  createclick) "$R/tools/java_bridge/around.sh" \
    'q "gamemode survival"' 'q "clear @s"' \
    'q "setblock $((X+2)) $Y $((Z-2)) create:depot"' \
    'q "setblock $((X-3)) $Y $((Z-3)) create:chain_conveyor"' \
    'q "setblock $((X+3)) $Y $((Z-3)) create:chain_conveyor"' \
    'echo $X $Y $Z > $S/pos' 'sleep 1' ;;
  # Create, Horizon's items into its blocks (bridge-mixins-11): a depot, a millstone on a
  # motor, crushing wheels on two motors (their controller between, facing down), a fan on a
  # motor blowing north through lava; $S/pos the player's block.
  createitemsin) "$R/tools/java_bridge/around.sh" \
    'q "gamemode creative"' 'q "tp @s $X $Y $Z 0 20"' \
    'q "setblock $((X+2)) $Y $((Z-2)) create:depot"' \
    'q "setblock $((X+3)) $Y $((Z+1)) create:creative_motor[facing=up]"' \
    'q "setblock $((X+3)) $((Y+1)) $((Z+1)) create:millstone"' \
    'q "setblock $((X-3)) $((Y+1)) $((Z-2)) create:crushing_wheel[axis=z]"' \
    'q "setblock $((X-1)) $((Y+1)) $((Z-2)) create:crushing_wheel[axis=z]"' \
    'q "setblock $((X-3)) $((Y+1)) $((Z-1)) create:creative_motor[facing=north]"' \
    'q "setblock $((X-1)) $((Y+1)) $((Z-3)) create:creative_motor[facing=south]"' \
    'q "setblock $((X-2)) $Y $((Z+4)) create:creative_motor[facing=north]"' \
    'q "setblock $((X-2)) $Y $((Z+3)) create:encased_fan[facing=north]"' \
    'q "setblock $((X-2)) $Y $((Z+2)) minecraft:lava"' \
    'echo $X $Y $Z > $S/pos' 'sleep 3' ;;
  # Lava's interactions with Create (bridge-mixins-11): a 2x2 pool of still water in the floor
  # at the north-west (the case pours lava on it, then honey and chocolate at the north-east
  # meet lava); $S/pos the player's block.
  createfluids) "$R/tools/java_bridge/around.sh" \
    'q "gamemode creative"' 'q "tp @s $X $((Y+1)) $Z 0 50"' \
    'q "fill $((X-4)) $((Y-2)) $((Z-4)) $((X-1)) $((Y-2)) $((Z-1)) stone"' \
    'q "fill $((X-3)) $((Y-1)) $((Z-3)) $((X-2)) $((Y-1)) $((Z-2)) water"' \
    'echo $X $Y $Z > $S/pos' 'sleep 2' ;;
  none) ;;
esac
# For steps: aim FILE looks at the block whose "x y z" FILE holds (a small one too: a flower),
# from points up its middle until the crosshair is on it; false if it never is.
aim(){
  local a b c; read a b c < "$1"
  local try
  for try in 0.3 0.15 0.45 0.6 up west north south east; do
    case $try in
      0.*) $C look-at "$(awk "BEGIN{print $a+0.5}")" "$(awk "BEGIN{print $b+$try}")" "$(awk "BEGIN{print $c+0.5}")" >/dev/null ;;
      *) $C look-at block "$a" "$b" "$c" "$try" >/dev/null ;;
    esac
    sleep 0.4
    $C state | python3 -c "import json,sys;b=(json.load(sys.stdin).get('looking_at') or {}).get('block') or {};sys.exit(b.get('pos')!=[$a,$b,$c])" && return 0
  done
  return 1
}
n=0; for step in "$@"; do eval "$step"; sleep 1; n=$((n+1)); $C screenshot "$S/s$n.png" >/dev/null; cp "$S/s$n.png" "$OUT/$SCENE-$n.png"; done
$C screenshot "$S/s0.png" >/dev/null; cp "$S/s0.png" "$OUT/$SCENE-0.png"
grep -E "failed|could not|    at " "$S/home/logs/latest.log" | grep -v "content:" | head -30 | cut -c1-260
echo "screenshots: $OUT/$SCENE-*.png  log: $S/home/logs/latest.log"
