#!/bin/bash
# The bridge's 1.20.1 runtime standalone (docs/java-bridge-120.md, phase 1): Fabric 1.20.1 with
# unmodified mods boots hidden (no window, no game loop) in a JVM of its own, and Boot120 writes
# its report: mods and how their entrypoints went, blocks, items, entities, biomes, dimensions.
#   tools/java_bridge/run120.sh [MODS_DIR] [REPORT.json]
# MODS_DIR: 1.20.1 Fabric mods (copied into a fresh game folder; GAME=DIR uses that folder and
# its mods/ instead). Fabric API comes along unless the mods hold their own. Needs fetch120.py.
# Java: HORIZON_JAVA_HOME, JAVA_HOME, /usr/lib/jvm/default (Arch), then java on the PATH.
# The same boot through Rust and the socket: cargo test -p horizon-jvm-remote --test boot120
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/../.." && pwd)
JAVA_HOME=${HORIZON_JAVA_HOME:-${JAVA_HOME:-}}
if [ -z "$JAVA_HOME" ]; then
  if [ -x /usr/lib/jvm/default/bin/javac ]; then JAVA_HOME=/usr/lib/jvm/default
  else JAVA_HOME=$(dirname "$(dirname "$(readlink -f "$(command -v javac)")")"); fi
fi
CACHE=${HORIZON_120_CACHE:-$HOME/.cache/horizon-120}/1.20.1
LAUNCH=$CACHE/launch.json
[ -f "$LAUNCH" ] || { echo "run tools/java_bridge/fetch120.py first"; exit 1; }
REPORT=$(realpath -m "${2:-$PWD/report120.json}")
field() { python3 -I -c "import json,sys;d=json.load(open(sys.argv[1]));v=d[sys.argv[2]];print(':'.join(v) if isinstance(v,list) else v)" "$LAUNCH" "$1"; }
CP=$(field class_path)
TMP=$(mktemp -d); trap 'rm -rf "$TMP"' EXIT
"$JAVA_HOME/bin/javac" -proc:none -nowarn --release 17 -d "$TMP/boot" \
  "$ROOT"/crates/horizon-jvm/java/dev/horizonlab/bridge/remote/*.java \
  "$ROOT"/crates/horizon-jvm/java/dev/horizonlab/bridge/r120/*.java
GAME=${GAME:-$(mktemp -d)}
mkdir -p "$GAME/mods"
[ -n "${1:-}" ] && cp "$1"/*.jar "$GAME/mods/"
ADD=()
ls "$GAME/mods" | grep -q '^fabric-api-' || ADD=(--add-mods "$(field fabric_api)")
# What 1.20.1's libraries need from a newer Java (horizon-jvm-remote's boot120.rs does the same).
MAJOR=$("$JAVA_HOME/bin/java" -XshowSettings:properties -version 2>&1 | sed -n 's/^ *java.specification.version = //p')
OPTS=()
[ "${MAJOR%%.*}" -ge 22 ] 2>/dev/null && OPTS+=(--enable-native-access=ALL-UNNAMED)
[ "${MAJOR%%.*}" -ge 24 ] 2>/dev/null && OPTS+=(--sun-misc-unsafe-memory-access=allow)
echo "game folder: $GAME, Java $MAJOR"
cd "$GAME"
"$JAVA_HOME/bin/java" -Xmx${XMX:-3G} "${OPTS[@]}" -cp "$TMP/boot:$CP" dev.horizonlab.bridge.r120.Boot120 \
  --report "$REPORT" --game-jar "$(field game_jar)" --mappings "$(field mojang_mappings)" \
  --game-dir "$GAME" "${ADD[@]}"
python3 -I - "$REPORT" <<'PY'
import json, sys
r = json.load(open(sys.argv[1]))
failed = [f"{m} {k}: {v}" for m, e in r["entrypoints"].items() for k, v in e.items() if v != "ok"]
print(f"{len(r['mods'])} mods, {len(r['blocks'])} blocks, {len(r['items'])} items in {r['start_ms']} ms on Java {r['java']}")
print(f"entrypoints failed: {len(failed)}" + "".join("\n  " + f for f in failed))
print(f"problems: {r['problems'] or 'none'}")
PY
