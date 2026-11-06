#!/bin/bash
# Starts the game headless with the Java mods of MODS (a folder of jars), in a fresh scratch
# folder (CLAUDE.md: never real player data), driven by horizon-ctl. Prints the scratch folder;
# its ctl.sock is the socket. The world has seed SEED (default 1234: the same land each run).
# JAVA_MODS=0: the Java side off (the plain game, for comparisons).
# SIZE=WxH: the window's size (default 1280x720); GUI_SCALE=n: options.txt's guiScale.
# Usage: MODS=~/mods [WORLD=name|none] [SEED=n] [SAVE=world-folder] tools/java_bridge/launch.sh [game args...]
set -e
R=$(cd "$(dirname "$0")/../.." && pwd)
: "${MODS:?MODS: a folder of mod jars}"
# Java 25 (26.3's classes are version 69): JAVA_HOME if it is one, else the first installed.
if ! "${JAVA_HOME:-/nonexistent}/bin/java" -version 2>&1 | grep -q '"25'; then
  JAVA_HOME=$(ls -d /usr/lib/jvm/*25* 2>/dev/null | head -1)
fi
[ -n "$JAVA_HOME" ] || { echo "no Java 25 in /usr/lib/jvm: set JAVA_HOME" >&2; exit 1; }
export JAVA_HOME HORIZON_JAVA_HOME=$JAVA_HOME PATH=$JAVA_HOME/bin:$PATH
# The previous run's scratch folder goes (each holds a world and the mods' content: ~1 GB).
old=$(cat /tmp/hz-bridge-scratch 2>/dev/null || true)
if [ -n "$old" ] && [ -d "$old" ]; then
  "$R/target/fast/horizon-ctl" --socket "$old/ctl.sock" quit >/dev/null 2>&1 || true
  # The game still writes its world while it quits: wait for it, so the folder can go.
  for _ in $(seq 60); do pgrep -f "$old/ctl.sock" >/dev/null || break; sleep 0.5; done
  rm -rf "$old" 2>/dev/null || true
fi
S=$(mktemp -d /tmp/hz-bridge.XXXX)
mkdir -p "$S/mods-java" "$S/home" "$S/lh" "$S/data" "$S/conf"
cp -p "$MODS"/*.jar "$S/mods-java/"
# The pack's mod configs: a config/ folder beside MODS (as in a launcher instance).
[ -d "$MODS/../config" ] && cp -r "$MODS/../config" "$S/config"
# CACHE=dir keeps the mods' content (made once per set of jars: minutes for a big pack) across runs.
[ -n "$CACHE" ] && mkdir -p "$CACHE" && ln -s "$CACHE" "$S/.horizon-cache"
[ -n "$OPTS" ] && cp "$OPTS" "$S/options.txt"
[ -n "$GRAPHICS" ] && cp "$GRAPHICS" "$S/horizon-graphics.txt"
"$R/tools/java_bridge/fps_cap.sh" "$S"
[ -n "$GUI_SCALE" ] && echo "guiScale:$GUI_SCALE" >> "$S/options.txt"
size=()
[ -n "$SIZE" ] && size=(--width "${SIZE%x*}" --height "${SIZE#*x}")
# The pack's resource packs (resourcepacks/ beside MODS, as in a launcher instance), turned on as
# its options.txt selects them.
[ -d "$MODS/../resourcepacks" ] && ln -s "$(cd "$MODS/../resourcepacks" && pwd)" "$S/resourcepacks"
if ! grep -q '^resourcePacks:' "$S/options.txt"; then
  grep -sE '^(resourcePacks|incompatibleResourcePacks):' "$MODS/../options.txt" >> "$S/options.txt" || true
fi
# CONFIG: a folder copied over the game folder's config/ (mods' settings before they start).
[ -n "$CONFIG" ] && mkdir -p "$S/config" && cp -r "$CONFIG"/. "$S/config/"
# SAVE: a world folder (a kept save) opened as WORLD instead of a new one.
[ -n "$SAVE" ] && mkdir -p "$S/saves" && cp -r "$SAVE" "$S/saves/${WORLD:-W}" && rm -f "$S/saves/${WORLD:-W}/session.lock"
echo "$S" > /tmp/hz-bridge-scratch
# A display for the offscreen renderer when there is none.
if [ -z "$DISPLAY" ]; then
  pgrep -x Xvfb >/dev/null || (nohup Xvfb :77 -screen 0 1280x1024x24 >/tmp/xvfb.log 2>&1 &)
  export DISPLAY=:77; sleep 1
fi
cd "$S"
export HORIZON_HOME=$S/home HORIZON_LAUNCHER_HOME=$S/lh XDG_DATA_HOME=$S/data XDG_CONFIG_HOME=$S/conf \
  HORIZON_LAUNCHER_NO_SELF_UPDATE=1 HORIZON_NO_AUDIO=1 HORIZON_JAVA_MODS=${JAVA_MODS:-1} HORIZON_ROOT=$R
# WORLD=none starts at the title screen (to make a world through Create New World).
world=(--world "${WORLD:-W}" --seed "${SEED:-1234}" --creative --wait)
[ "${WORLD:-W}" = none ] && world=()
"$R/target/fast/horizon-ctl" --socket "$S/ctl.sock" launch "${world[@]}" "${size[@]}" --offscreen \
  --game "${GAME:-$R/target/fast/horizon-client}" --resources "$R/.local/horizon-client/26.3" "$@" > "$S/ctl.out" 2>&1
# --wait waits for a world; at the title screen, for the screen (the mods' content first).
[ "${WORLD:-W}" = none ] && "$R/target/fast/horizon-ctl" --socket "$S/ctl.sock" wait-for screen title --timeout 900 >> "$S/ctl.out" 2>&1
echo "$S"
