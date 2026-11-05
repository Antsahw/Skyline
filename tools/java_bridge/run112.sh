#!/bin/bash
# The bridge's 1.12 side standalone (docs/java-bridge-112.md, phase 1): Boot112 on the system
# class path (as horizon-jvm's boot112.rs puts it), Cleanroom boots hidden (SDL's offscreen
# driver, OpenAL's null device), the platform mod reports every block, item, fluid, entity,
# biome and dimension, then the client thread answers a ping.
#   tools/java_bridge/run112.sh [MODS_DIR] [REPORT.json] [SCREENSHOT.png]
# MODS_DIR: extra 1.12 mods (the fixture hello112 is always in). GAME=DIR runs in a prepared game
# folder (a pack's config/, scripts/ and mods/) instead of a new one; SHOT_MOD=id: whose items
# the screenshot shows (default hello112). Needs fetch112.py + build112.sh.
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/../.." && pwd)
JAVA_HOME=${HORIZON_JAVA_HOME:-${JAVA_HOME:-/usr/lib/jvm/java-25-openjdk-amd64}}
VER=$(python3 -I -c "import json;print(json.load(open('$ROOT/research-lock.json'))['cleanroom']['version'])")
CACHE=${HORIZON_112_CACHE:-$HOME/.cache/horizon-112}/$VER
LAUNCH=$CACHE/launch.json
REPORT=${2:-$PWD/report112.json}
CP=$(python3 -I -c "import json;print(':'.join(json.load(open('$LAUNCH'))['class_path']))")
TMP=$(mktemp -d); GAME=${GAME:-$(mktemp -d)}; trap 'rm -rf "$TMP"' EXIT
"$JAVA_HOME/bin/javac" -proc:none --release 25 -d "$TMP/boot" \
  "$ROOT/crates/horizon-jvm/java/dev/horizonlab/bridge/c112/Boot112.java" "$ROOT/crates/horizon-jvm/java/dev/horizonlab/bridge/Natives.java"
mkdir -p "$GAME/mods" "$GAME/config"
# The platform mod stays out of mods/: FML takes it from --mods, relative to the game folder.
mkdir -p "$GAME/horizon-java-112"
cp "$CACHE/horizon/horizon112-platform.jar" "$GAME/horizon-java-112/"
cp "$CACHE/horizon/hello112.jar" "$GAME/mods/"
[ -n "${1:-}" ] && cp "$1"/*.jar "$GAME/mods/"
echo "enabled=false" > "$GAME/config/splash.properties"
echo "game folder: $GAME"
cd "$GAME"
SDL_VIDEO_DRIVER=${SDL_VIDEO_DRIVER:-offscreen} ALSOFT_DRIVERS=${ALSOFT_DRIVERS:-null} \
"$JAVA_HOME/bin/java" -Xmx${XMX:-3G} -Dhorizon.bridge112.report="$REPORT" \
  ${3:+-Dhorizon.bridge112.screenshot="$3"} -Dhorizon.bridge112.screenshot.mod=${SHOT_MOD:-hello112} \
  -Djava.library.path="$CACHE/natives" -Dorg.lwjgl.librarypath="$CACHE/natives" \
  -cp "$TMP/boot:$CP" dev.horizonlab.bridge.c112.Boot112 top.outlands.foundation.boot.Foundation \
  --username Horizon --version 1.12.2 --gameDir "$GAME" --assetsDir "$CACHE/assets" --assetIndex 1.12 \
  --accessToken 0 --userType legacy --tweakClass net.minecraftforge.fml.common.launcher.FMLTweaker \
  --mods horizon-java-112/horizon112-platform.jar
