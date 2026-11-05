#!/bin/bash
# Builds the bridge's 1.12 side (docs/java-bridge-112.md) from crates/horizon-jvm/java/platform112:
#   boot jar      Boot112 (system class path: starts Cleanroom on a thread of its own)
#   platform jar  the Forge mod "horizon112" (reports blocks, items... then parks the client)
#   fixture jar   a small test mod (one block with metadata, two items)
# into $OUT (default ~/.cache/horizon-112/<version>/horizon). The first run also makes the
# compile jar: Mojang's 1.12.2 classes as FML deobfuscates them (SRG names) and Cleanroom
# patches them, dumped by a Java agent from one boot (devdump/). It holds Mojang's code: it
# stays in the cache, never in the repo.
#   python3 -I tools/java_bridge/fetch112.py && tools/java_bridge/build112.sh
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/../.." && pwd)
SRC=$ROOT/crates/horizon-jvm/java/platform112
JAVA_HOME=${HORIZON_JAVA_HOME:-${JAVA_HOME:-/usr/lib/jvm/java-25-openjdk-amd64}}
VER=$(python3 -I -c "import json;print(json.load(open('$ROOT/research-lock.json'))['cleanroom']['version'])")
CACHE=${HORIZON_112_CACHE:-$HOME/.cache/horizon-112}/$VER
LAUNCH=$CACHE/launch.json
[ -f "$LAUNCH" ] || { echo "run tools/java_bridge/fetch112.py first"; exit 1; }
OUT=${OUT:-$CACHE/horizon}
DEV=$CACHE/dev
mkdir -p "$OUT" "$DEV"
CP=$(python3 -I -c "import json;print(':'.join(json.load(open('$LAUNCH'))['class_path']))")
# The libraries without Mojang's (notch-named) client jar: platform code compiles against SRG.
LIBS=$(python3 -I -c "import json;print(':'.join(p for p in json.load(open('$LAUNCH'))['class_path'] if not p.endswith('/versions/1.12.2.jar')))")
TMP=$(mktemp -d); trap 'rm -rf "$TMP"' EXIT
JAVAC=("$JAVA_HOME/bin/javac" -proc:none -nowarn -encoding UTF-8)

# 1. The compile jar (once per Cleanroom version).
SRG=$DEV/minecraft-1.12.2-srg.jar
if [ ! -f "$SRG" ]; then
  echo "making the compile jar (one boot, ~1 min)"
  "${JAVAC[@]}" --release 8 -d "$TMP/dump" -cp "$LIBS" $(find "$SRC/devdump" -name '*.java')
  printf 'Premain-Class: dev.horizonlab.bridge112.dump.ClassDumpAgent\n' > "$TMP/agent.mf"
  # Two jars: the agent's goes on the system class path, so the mod must not be in it (its
  # annotations would resolve against the app loader's Forge classes, which FML never sees).
  (cd "$TMP/dump" && "$JAVA_HOME/bin/jar" cfm "$TMP/agent.jar" "$TMP/agent.mf" dev/horizonlab/bridge112/dump/ClassDumpAgent*.class)
  GAME=$(mktemp -d); mkdir -p "$GAME/mods" "$GAME/config"
  (cd "$TMP/dump" && "$JAVA_HOME/bin/jar" cf "$GAME/mods/horizon112-dump.jar" dev/horizonlab/bridge112/dump/DumpMod*.class)
  echo "enabled=false" > "$GAME/config/splash.properties"
  (cd "$GAME" && SDL_VIDEO_DRIVER=${SDL_VIDEO_DRIVER:-offscreen} ALSOFT_DRIVERS=null timeout 600 "$JAVA_HOME/bin/java" -Xmx3G \
    -javaagent:"$TMP/agent.jar=$SRG.tmp" \
    -Djava.library.path="$CACHE/natives" -Dorg.lwjgl.librarypath="$CACHE/natives" -cp "$CP" \
    top.outlands.foundation.boot.Foundation --username Dev --version 1.12.2 --gameDir "$GAME" \
    --assetsDir "$CACHE/assets" --assetIndex 1.12 --accessToken 0 --userType legacy \
    --tweakClass net.minecraftforge.fml.common.launcher.FMLTweaker > "$DEV/dump.log" 2>&1) || true
  grep '\[horizon112\] loaded' "$DEV/dump.log" || { tail -30 "$DEV/dump.log"; exit 1; }
  mv "$SRG.tmp" "$SRG"
  rm -rf "$GAME"
fi

# 2. MCP's names (stable_39: what 1.12 mods are written in) and the compile jar in those names.
MCP=$DEV/mcp_stable-39-1.12
if [ ! -f "$MCP/methods.csv" ]; then
  read -r URL SHA < <(python3 -I -c "import json;c=json.load(open('$ROOT/research-lock.json'))['cleanroom'];print(c['mcp_names_url'],c['mcp_names_sha256'])")
  curl -sSfL -o "$TMP/mcp.zip" "$URL"
  echo "$SHA  $TMP/mcp.zip" | sha256sum -c --quiet
  mkdir -p "$MCP" && (cd "$MCP" && unzip -q -o "$TMP/mcp.zip")
fi
"${JAVAC[@]}" --release 25 -d "$TMP/tools" -cp "$LIBS" $(find "$SRC/devdump" -name 'Remap112.java')
REMAP=("$JAVA_HOME/bin/java" -cp "$TMP/tools:$LIBS" dev.horizonlab.bridge112.dump.Remap112)
MCPJAR=$DEV/minecraft-1.12.2-mcp.jar
[ -f "$MCPJAR" ] || "${REMAP[@]}" deobf "$MCPJAR" "$MCP/methods.csv" "$MCP/fields.csv" "$SRG"

# 3. A mod: compiled with MCP names against the compile jar, renamed to SRG, jarred with its files.
build_mod() { # NAME SRC_DIR
  local name=$1 dir=$2
  rm -rf "$TMP/$name" && mkdir -p "$TMP/$name/mcp" "$TMP/$name/srg"
  "${JAVAC[@]}" --release 8 -d "$TMP/$name/mcp" -cp "$MCPJAR:$LIBS" $(find "$dir" -name '*.java')
  "${REMAP[@]}" reobf "$TMP/$name/mcp" "$TMP/$name/srg" "$MCP/methods.csv" "$MCP/fields.csv" "$SRG" | sed "s/^/$name: /"
  (cd "$dir" && find . -type f ! -name '*.java' -exec cp --parents {} "$TMP/$name/srg/" \;)
  (cd "$TMP/$name/srg" && "$JAVA_HOME/bin/jar" cf "$OUT/$name.jar" .)
}
build_mod horizon112-platform "$SRC/mod"
build_mod hello112 "$SRC/fixture"
echo "built: $OUT/horizon112-platform.jar $OUT/hello112.jar"
