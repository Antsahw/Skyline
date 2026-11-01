#!/bin/bash
# The lost-block repro (regress.py's `lost`): right after spawn, while chunks around are still
# generating, sets 36 gold blocks in a 6×6 grid out to 40 blocks away, waits, and reads them
# back. A block a late worldgen pass overwrote is "lost". Writes one summary line to OUT:
#   lost <n> of <set> set (unloaded <n>)
# then a line per lost block. Usage (C is horizon-ctl with its socket):
#   tools/java_bridge/lost_blocks.sh "$C" out.txt [wait seconds, default 25]
C=$1 OUT=$2 WAIT=${3:-25}
read -r X Y Z < <($C state | python3 -c "import json,sys,math;p=json.load(sys.stdin)['player']['position'];print(math.floor(p[0]),math.floor(p[1]),math.floor(p[2]))")
pts=""
for dx in -40 -24 -8 8 24 40; do for dz in -40 -24 -8 8 24 40; do
  x=$((X+dx)); z=$((Z+dz)); y=$((Y+3))
  $C command "setblock $x $y $z minecraft:gold_block" | grep -q Changed && pts="$pts $x,$y,$z"
done; done
sleep "$WAIT"
lost=0; unl=0; detail=""
for p in $pts; do
  IFS=, read -r x y z <<< "$p"
  b=$($C block "$x" "$y" "$z" | python3 -c "import json,sys; d=json.load(sys.stdin); print(d.get('block') if d.get('loaded') else 'unloaded')")
  if [ "$b" = unloaded ]; then unl=$((unl+1))
  elif [ "$b" != minecraft:gold_block ]; then
    lost=$((lost+1)); detail="$detail  lost at $x $y $z: $b (chunk $((x>>4)) $((z>>4)))"$'\n'
  fi
done
{ echo "lost $lost of $(echo $pts | wc -w) set (unloaded $unl)"; printf %s "$detail"; } > "$OUT"
