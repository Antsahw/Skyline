#!/bin/bash
# Survey one mod: MODDIR (jars), OUTDIR, MODID, MENU=1 (has a Mod Menu entrypoint), EXTRA (steps).
# Starts headless with the Java mods switch on (launch.sh sets HORIZON_JAVA_MODS=1), waits for
# the world, screenshots the HUD, looks at a chest (tooltips/HUD mods), opens the inventory,
# opens the mod's config screen (/modconfig), and keeps the log.
R=$(cd "$(dirname "$0")/../../.." && pwd)
: "${MODDIR:?}" "${OUTDIR:?}" "${MODID:?}"
mkdir -p "$OUTDIR"
start=$(date +%s)
S=$(MODS=$MODDIR timeout 900 "$R/tools/java_bridge/launch.sh" 2>"$OUTDIR/launch.err")
rc=$?
S=${S:-$(cat /tmp/hz-bridge-scratch)}
echo "launch rc=$rc secs=$(( $(date +%s) - start ))" > "$OUTDIR/run.txt"
cp "$S/ctl.out" "$OUTDIR/ctl.out" 2>/dev/null
C="$R/target/fast/horizon-ctl --socket $S/ctl.sock"
shot(){ timeout 30 $C screenshot "$OUTDIR/$1.png" >/dev/null 2>&1; }
st(){ timeout 20 $C state > "$OUTDIR/$1.json" 2>/dev/null; }
if [ $rc -eq 0 ] && timeout 10 $C state >/dev/null 2>&1; then
  echo "joined=yes" >> "$OUTDIR/run.txt"
  timeout 60 "$R/tools/java_bridge/around.sh" \
    'q "setblock $((X+2)) $Y $Z minecraft:chest"' \
    'q "item replace block $((X+2)) $Y $Z container.0 with minecraft:diamond 5"' \
    'q "give @s minecraft:bread 4"' 'q "give @s minecraft:shulker_box"' 'q "gamemode survival"' \
    '$C look-at-block $((X+2)) $Y $Z >/dev/null' 'sleep 4' > "$OUTDIR/around.txt" 2>&1
  shot 1-hud; st 1-hud
  timeout 10 $C key e tap >/dev/null 2>&1; sleep 3; shot 2-inventory; st 2-inventory
  timeout 10 $C key esc tap >/dev/null 2>&1; sleep 1
  if [ -n "$MENU" ]; then
    timeout 20 $C command "modconfig $MODID" > "$OUTDIR/modconfig.json" 2>&1; sleep 4
    shot 3-config; st 3-config
    timeout 10 $C key esc tap >/dev/null 2>&1; sleep 1
  fi
  n=4
  for step in $EXTRA; do  # steps: key:<name> (tap, screenshot, esc), cmd:<command,with_underscores_for_spaces>
    case $step in
      key:*) timeout 10 $C key "${step#key:}" tap >/dev/null 2>&1; sleep 3; shot "$n-${step#key:}"; st "$n-${step#key:}"
             timeout 10 $C key esc tap >/dev/null 2>&1 ;;
      cmd:*) c=${step#cmd:}; timeout 20 $C command "${c//_/ }" > "$OUTDIR/$n-cmd.json" 2>&1; sleep 3; shot "$n-cmd" ;;
    esac
    n=$((n+1)); sleep 1
  done
  sleep 5; shot 9-end; st 9-end
  timeout 10 $C chat-log 40 > "$OUTDIR/chat.txt" 2>&1
else
  echo "joined=no" >> "$OUTDIR/run.txt"
fi
alive=no; pgrep -f "$S/ctl.sock" >/dev/null && alive=yes; echo "alive=$alive" >> "$OUTDIR/run.txt"
cp "$S/home/logs/latest.log" "$OUTDIR/latest.log" 2>/dev/null
timeout 30 $C quit >/dev/null 2>&1
for _ in $(seq 40); do pgrep -f "$S/ctl.sock" >/dev/null || break; sleep 0.5; done
pkill -9 -f "$S/ctl.sock" 2>/dev/null
cp "$S/home/logs/latest.log" "$OUTDIR/latest.log" 2>/dev/null
ls "$S"/home/crash-reports/* >/dev/null 2>&1 && cp -r "$S/home/crash-reports" "$OUTDIR/"
rm -rf "$S"
echo "total secs=$(( $(date +%s) - start ))" >> "$OUTDIR/run.txt"
