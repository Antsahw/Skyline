#!/bin/bash
# Builds one of the bridge's fixture mods (crates/horizon-jvm/java/fixtures/<name>/, its Java
# sources and assets) into a Fabric mod jar, compiled against the 26.3 client jar, Fabric
# Loader and Fabric API (as the bridge's build has them).
#   tools/java_bridge/fixture_mod.sh crittermod /tmp/critters/critters.jar
set -e
R=$(cd "$(dirname "$0")/../.." && pwd)
NAME=${1:?a fixture: crittermod}
OUT=${2:?the jar to write}
SRC=$R/crates/horizon-jvm/java/fixtures/$NAME
if ! "${JAVA_HOME:-/nonexistent}/bin/java" -version 2>&1 | grep -q '"25'; then
  JAVA_HOME=$(ls -d /usr/lib/jvm/*25* 2>/dev/null | head -1)
fi
MODULES=$(ls -dt "$R"/target/*/build/horizon-jvm-*/out/fabric-api-modules 2>/dev/null | head -1)
[ -n "$MODULES" ] || { echo "no Fabric API modules: build horizon-client with java-mods first" >&2; exit 1; }
shopt -s globstar nullglob
CP="$R/.local/minecraft/26.3/jars/client.jar"
for j in "$R"/.local/minecraft/26.3/libraries/**/*.jar "$R"/.local/bridge/lib/*.jar "$MODULES"/*.jar; do
  CP="$CP:$j"
done
# MixinExtras (@Local, @WrapOperation), which Fabric Loader carries inside its jar.
X=$(mktemp -d)
for l in "$R"/.local/bridge/lib/fabric-loader-*.jar; do
  (cd "$X" && unzip -q -o "$l" 'META-INF/jars/mixinextras-*.jar' 2>/dev/null) || true
done
for j in "$X"/META-INF/jars/*.jar; do CP="$CP:$j"; done
# FIXTURE_CP: mod jars a fixture builds against (GeckoLib for geckomod), ':'-separated.
[ -n "$FIXTURE_CP" ] && CP="$CP:$FIXTURE_CP"
W=$(mktemp -d)
# compile-only/: APIs the fixture compiles against that the game provides (Mod Menu's,
# which Horizon makes): built beside the jar, not into it.
if [ -d "$SRC/compile-only" ]; then
  C=$(mktemp -d)
  "$JAVA_HOME/bin/javac" -nowarn -proc:none -d "$C" -cp "$CP" $(find "$SRC/compile-only" -name '*.java')
  CP="$CP:$C"
fi
"$JAVA_HOME/bin/javac" -nowarn -proc:none -d "$W" -cp "$CP" $(find "$SRC" -name '*.java' -not -path "$SRC/compile-only/*")
cp "$SRC/fabric.mod.json" "$W/"
cp "$SRC"/*.mixins.json "$W/" 2>/dev/null || true
[ -d "$SRC/assets" ] && cp -r "$SRC/assets" "$W/"
mkdir -p "$(dirname "$OUT")"
rm -f "$OUT"
(cd "$W" && "$JAVA_HOME/bin/jar" --create --file "$OUT" .)
rm -rf "$W"
echo "$OUT"
