#!/bin/bash
# The NeoForge bridge prototype, standalone: NeoBoot on the system class path (as Rust would put
# it), FML loads the mods in NEOFORGE_INSTALL_DIR/mods plus the platform mod, then NeoCheck
# prints what was registered. No dedicated server starts.
#   tools/java_bridge/neoforge/run_proto.sh NEOFORGE_INSTALL_DIR
# The platform mod: crates/horizon-jvm/java/neoforge/platform (built into INSTALL/horizon/).
set -euo pipefail
JAVA_SRC=$(cd "$(dirname "$0")/../../../crates/horizon-jvm/java" && pwd)
HERE=$JAVA_SRC/neoforge
INSTALL=$(cd "$1" && pwd)
JAVA_HOME=${JAVA_HOME:-/usr/lib/jvm/java-25-openjdk-amd64}
ARGS=$(ls "$INSTALL"/libraries/net/neoforged/neoforge/*/unix_args.txt | head -1)
CP=$(tr ' ' '\n' < "$ARGS" | grep -A1 '^-classpath$' | tail -1 | tr ':' '\n' | sed "s|^libraries/|$INSTALL/libraries/|" | paste -sd:)
LAUNCHER=$(tr ' ' '\n' < "$ARGS" | grep '^net.neoforged.fml.startup')
FML_ARGS=$(tr ' ' '\n' < "$ARGS" | sed -n '/^net.neoforged.fml.startup/,$p' | tail -n +2 | tr '\n' ' ')
OUT=$(mktemp -d); trap 'rm -rf "$OUT"' EXIT
"$JAVA_HOME/bin/javac" -proc:none --release 25 -d "$OUT/boot" "$JAVA_SRC/dev/horizonlab/bridge/neo/NeoBoot.java" "$JAVA_SRC/dev/horizonlab/bridge/Natives.java"
LIBS=$(find "$INSTALL/libraries" -name '*.jar' ! -name 'installertools*' | paste -sd:)
"$JAVA_HOME/bin/javac" -proc:none --release 25 -d "$OUT/platform" -cp "$LIBS" $(find "$HERE/platform" -name '*.java')
cp -r "$HERE/platform/META-INF" "$HERE/platform/horizon_neo.mixins.json" "$OUT/platform/"
# The platform mod (Horizon's game finds it here too: HORIZON_NEOFORGE=INSTALL).
mkdir -p "$INSTALL/horizon" "$INSTALL/mods"
(cd "$OUT/platform" && "$JAVA_HOME/bin/jar" cf "$INSTALL/horizon/horizon-neo-platform.jar" .)
ln -sf "$INSTALL/horizon/horizon-neo-platform.jar" "$INSTALL/mods/horizon-neo-platform.jar"
cd /
"$JAVA_HOME/bin/java" -Xmx3G --add-opens java.base/java.lang.invoke=ALL-UNNAMED \
  -DlibraryDirectory="$INSTALL/libraries" -cp "$OUT/boot:$CP" dev.horizonlab.bridge.neo.NeoBoot "$LAUNCHER" $FML_ARGS --gameDir "$INSTALL"
