#!/bin/bash
# Makes a new world of world type TYPE (as Create New World's World Type button names it:
# "Default", "Superflat", a mod's "One Block") through the title screen of the game launch.sh
# started with WORLD=none, in Creative, and waits until the player is in it.
# Usage: tools/java_bridge/new_world.sh "One Block"
R=$(cd "$(dirname "$0")/../.." && pwd)
TYPE="${1:?a world type, as the button names it}"
S=$(cat /tmp/hz-bridge-scratch); C="$R/target/fast/horizon-ctl --socket $S/ctl.sock"
label() {
  $C screen | python3 -c "import json,sys; print(next((b['label'] for b in json.load(sys.stdin)['buttons'] if b['label'].startswith('World Type')), ''))"
}
# A fresh game folder opens on the first-launch accessibility screen
# (AccessibilityOnboardingScreen) before the title screen.
$C click-button Continue >/dev/null 2>&1
for _ in $(seq 20); do
  $C click-button Singleplayer 2>/dev/null | grep -q '"ok":true' && break
  sleep 0.5
done
sleep 2
$C click-button World >/dev/null; sleep 1
for _ in 1 2 3 4 5 6 7 8; do
  cur=$(label)
  [ "$cur" = "World Type: $TYPE" ] && break
  [ -z "$cur" ] && { echo "no World Type button" >&2; exit 1; }
  $C click-button "$cur" >/dev/null; sleep 0.4
done
[ "$(label)" = "World Type: $TYPE" ] || { echo "no world type $TYPE" >&2; exit 1; }
$C click-button Game >/dev/null; sleep 0.4
# Survival, Hardcore, Creative.
$C click-button "Game Mode: Survival" >/dev/null; $C click-button "Game Mode: Hardcore" >/dev/null; sleep 0.3
$C click-button "Create New World" >/dev/null
$C wait-ready 600 >/dev/null 2>&1
