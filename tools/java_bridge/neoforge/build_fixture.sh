#!/bin/bash
# Builds the NeoForge fixture mod (crates/horizon-jvm/java/neoforge/neohello) into a jar.
#   tools/java_bridge/neoforge/build_fixture.sh NEOFORGE_INSTALL_DIR OUT.jar
# NEOFORGE_INSTALL_DIR: a folder where NeoForge's installer ran (--installServer); its
# libraries/ are the class path (the patched game, NeoForge, FML). Java 25.
set -euo pipefail
SRC=$(cd "$(dirname "$0")/../../../crates/horizon-jvm/java/neoforge/neohello" && pwd)
INSTALL=$(cd "$1" && pwd); OUT=$(realpath -m "$2")
JAVA_HOME=${JAVA_HOME:-/usr/lib/jvm/java-25-openjdk-amd64}
CP=$(find "$INSTALL/libraries" -name '*.jar' ! -name 'installertools*' | tr '\n' ':')
TMP=$(mktemp -d); trap 'rm -rf "$TMP"' EXIT
"$JAVA_HOME/bin/javac" -proc:none --release 25 -d "$TMP" -cp "$CP" $(find "$SRC" -name '*.java')
cp -r "$SRC/META-INF" "$TMP/"
mkdir -p "$(dirname "$OUT")"
(cd "$TMP" && "$JAVA_HOME/bin/jar" cf "$OUT" .)
echo "built $OUT"
