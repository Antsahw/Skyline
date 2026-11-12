#!/bin/bash
# Builds the bridge's 1.20.1 platform (docs/java-bridge-120.md, phase 2) from
# crates/horizon-jvm/java/platform120, as Fabric's Loom builds a mod, without Gradle:
#   1. mappings: official, intermediary and Mojang's names in one Tiny file (once)
#   2. the compile jar: Mojang's 1.20.1 client remapped to Mojang's names (once)
#   3. platform120's sources (written with Mojang's names) compiled against it
#   4. the classes remapped to intermediary, the names 1.20.1 runs under in Fabric, with the
#      class table platform120's Names reads -> $OUT/platform120.jar (a Fabric mod)
# The compile jar and the table come from Mojang's files: they stay in the cache, never in the
# repo. OUT defaults to ~/.cache/horizon-120/1.20.1/horizon.
#   python3 -I tools/java_bridge/fetch120.py && tools/java_bridge/build120.sh
# Java: HORIZON_JAVA_HOME, JAVA_HOME, /usr/lib/jvm/default (Arch), then javac on the PATH.
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/../.." && pwd)
SRC=$ROOT/crates/horizon-jvm/java/platform120
JAVA_HOME=${HORIZON_JAVA_HOME:-${JAVA_HOME:-}}
if [ -z "$JAVA_HOME" ]; then
  if [ -x /usr/lib/jvm/default/bin/javac ]; then JAVA_HOME=/usr/lib/jvm/default
  else JAVA_HOME=$(dirname "$(dirname "$(readlink -f "$(command -v javac)")")"); fi
fi
CACHE=${HORIZON_120_CACHE:-$HOME/.cache/horizon-120}/1.20.1
LAUNCH=$CACHE/launch.json
[ -f "$LAUNCH" ] || { echo "run tools/java_bridge/fetch120.py first"; exit 1; }
OUT=${OUT:-$CACHE/horizon}
DEV=$CACHE/dev
mkdir -p "$OUT" "$DEV"
field() { python3 -I -c "import json,sys;d=json.load(open(sys.argv[1]));v=d[sys.argv[2]];print(':'.join(v) if isinstance(v,list) else v)" "$LAUNCH" "$1"; }
REMAPPER=$(field build_tools)
[ -f "$REMAPPER" ] || { echo "no remapper in launch.json: run tools/java_bridge/fetch120.py again"; exit 1; }
# Mojang's libraries (not the game jar, not the loader): what Minecraft's code compiles against.
LIBS=$(python3 -I -c "import json,sys;print(':'.join(p for p in json.load(open(sys.argv[1]))['class_path'] if '/libraries/' in p))" "$LAUNCH")
# Mixin and ASM (the loader's): platform120's Mixins compile against their annotations.
MIXIN=$(python3 -I -c "import json,sys;print(':'.join(p for p in json.load(open(sys.argv[1]))['class_path'] if 'sponge-mixin' in p.rsplit('/',1)[-1] or p.rsplit('/',1)[-1].startswith('asm')))" "$LAUNCH")
TMP=$(mktemp -d); trap 'rm -rf "$TMP"' EXIT
JAVAC=("$JAVA_HOME/bin/javac" -proc:none -nowarn -encoding UTF-8 --release 17)
"${JAVAC[@]}" -cp "$REMAPPER" -d "$TMP/tool" $(find "$SRC/build" -name '*.java')
REMAP=("$JAVA_HOME/bin/java" -Xmx3G -cp "$TMP/tool:$REMAPPER" dev.horizonlab.build120.Remap120)
MAPPINGS=$DEV/mappings-1.20.1.tiny
NAMED=$DEV/minecraft-1.20.1-named.jar
# 1. and 2., once (a fetch120.py with other pins starts them over: delete $DEV).
[ -f "$MAPPINGS" ] || "${REMAP[@]}" mappings "$(dirname "$(field game_jar)")/../fabric/$(basename "$(python3 -I -c "import json,sys;print([p for p in json.load(open(sys.argv[1]))['class_path'] if 'intermediary-' in p][0])" "$LAUNCH")")" "$(field mojang_mappings)" "$MAPPINGS"
if [ ! -f "$NAMED" ]; then
  # shellcheck disable=SC2046
  "${REMAP[@]}" remap "$MAPPINGS" official named "$(field game_jar)" "$NAMED" $(echo "$LIBS" | tr ':' ' ') 2>&1 | grep -v '^\[WARN\]' || true
  [ -f "$NAMED" ] || { echo "the compile jar failed"; exit 1; }
fi
# The Fabric API modules platform120 calls (the Transfer API for Horizon's hoppers, the events
# it fires), remapped
# to Mojang's names to compile against, as Loom remaps a mod's dependencies; once, like the
# compile jar. Compile-only: at run time the loader's own Fabric API is there.
API_NAMED=$DEV/fabric-api-named
API_MODULES="fabric-api-base fabric-api-lookup-api-v1 fabric-transfer-api-v1 fabric-events-interaction-v0 fabric-lifecycle-events-v1 fabric-networking-api-v1 fabric-key-binding-api-v1 fabric-biome-api-v1 fabric-renderer-api-v1 fabric-renderer-indigo fabric-model-loading-api-v1"
mkdir -p "$API_NAMED"
for m in $API_MODULES; do
  [ -s "$API_NAMED/$m.jar" ] && continue
  unzip -p "$(field fabric_api)" "META-INF/jars/$m-*.jar" > "$TMP/$m.jar"
  "${REMAP[@]}" remap "$MAPPINGS" intermediary named "$TMP/$m.jar" "$API_NAMED/$m.jar" "$NAMED" 2>&1 | grep -v '^\[WARN\]' || true
  [ -s "$API_NAMED/$m.jar" ] || { echo "remapping Fabric API's $m failed"; exit 1; }
done
API=$(for m in $API_MODULES; do printf '%s:' "$API_NAMED/$m.jar"; done)
# 3. Compile.
"${JAVAC[@]}" -cp "$NAMED:$LIBS:$MIXIN:$API" -d "$TMP/classes" $(find "$SRC/src" -name '*.java')
cp "$SRC/src/fabric.mod.json" "$SRC/src/horizon-platform120.mixins.json" "$TMP/classes/"
mkdir -p "$TMP/classes/horizon"
python3 -I - "$MAPPINGS" "$TMP/classes/horizon/classes-1.20.1.tsv" <<'PY'
import sys
# Tiny v2: "c<TAB>official<TAB>intermediary<TAB>named" for every class.
with open(sys.argv[1]) as f, open(sys.argv[2], "w") as out:
    header = f.readline().rstrip("\n").split("\t")
    names = header[3:]
    i, n = names.index("intermediary"), names.index("named")
    for line in f:
        if line.startswith("c\t"):
            parts = line.rstrip("\n").split("\t")[1:]
            out.write(parts[i].replace("/", ".") + "\t" + parts[n].replace("/", ".") + "\n")
PY
(cd "$TMP/classes" && "$JAVA_HOME/bin/jar" cf "$TMP/platform120-named.jar" .)
# 4. Remap to intermediary.
# shellcheck disable=SC2046
"${REMAP[@]}" remap "$MAPPINGS" named intermediary "$TMP/platform120-named.jar" "$OUT/platform120.jar" "$NAMED" $(echo "$MIXIN:$LIBS:$API" | tr ':' ' ') 2>&1 | grep -v '^\[WARN\]'
echo "$OUT/platform120.jar"
