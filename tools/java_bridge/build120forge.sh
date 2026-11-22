#!/bin/bash
# Builds the bridge's 1.20.1 platform as a Forge mod (docs/java-bridge-120-forge.md, phase 3), from
# the same sources as build120.sh's Fabric one (crates/horizon-jvm/java/platform120/src) plus its
# Forge side (platform120/forge):
#   1. mappings: build120.sh's official/intermediary/named file with Forge's runtime names added
#      as `srg` (Mojang's class names, SRG member names), from the installer's mcp_config (once)
#   2. the compile jars: Forge's patched client, the rest of the client and Forge itself, remapped
#      from srg to Mojang's names (once)
#   3. the sources compiled against them
#   4. remapped to srg, with the mixin config minus the mixins on Fabric API's own classes
#      -> $OUT/platform120-forge.jar (a Forge mod)
# Everything made from Mojang's or Forge's files stays in the cache, never in the repo.
#   python3 -I tools/java_bridge/fetch120.py && tools/java_bridge/build120.sh \
#     && python3 -I tools/java_bridge/fetch120forge.py && tools/java_bridge/build120forge.sh
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
FORGE=$CACHE/forge
[ -f "$FORGE/launch.json" ] || { echo "run tools/java_bridge/fetch120forge.py first"; exit 1; }
DEV=$CACHE/dev
MAPPINGS=$DEV/mappings-1.20.1.tiny
[ -f "$MAPPINGS" ] || { echo "run tools/java_bridge/build120.sh first (its mappings)"; exit 1; }
OUT=${OUT:-$CACHE/horizon}
FDEV=$FORGE/dev
mkdir -p "$OUT" "$FDEV"
field() { python3 -I -c "import json,sys;d=json.load(open(sys.argv[1]));v=d[sys.argv[2]];print(':'.join(v) if isinstance(v,list) else v)" "$1" "$2"; }
REMAPPER=$(field "$LAUNCH" build_tools)
VERSION=$(field "$FORGE/launch.json" forge)
LIB=$FORGE/libraries
TMP=$(mktemp -d); trap 'rm -rf "$TMP"' EXIT
JAVAC=("$JAVA_HOME/bin/javac" -proc:none -nowarn -encoding UTF-8 --release 17)
"${JAVAC[@]}" -cp "$REMAPPER" -d "$TMP/tool" $(find "$SRC/build" -name '*.java')
REMAP=("$JAVA_HOME/bin/java" -Xmx3G -cp "$TMP/tool:$REMAPPER" dev.horizonlab.build120.Remap120)
NAMED=$DEV/minecraft-1.20.1-named.jar
# 1.
SRG=$FDEV/mappings-1.20.1-srg.tiny
MERGED=$(find "$LIB/de/oceanlabs/mcp/mcp_config" -name '*-mappings-merged.txt' | head -1)
[ -f "$SRG" ] || "${REMAP[@]}" srg "$MAPPINGS" "$MERGED" "$SRG"
# 2. Forge's patched classes first: where Forge patched a class, its version is the one to see.
PATCHED=$LIB/net/minecraftforge/forge/1.20.1-$VERSION/forge-1.20.1-$VERSION-client.jar
UNIVERSAL=$LIB/net/minecraftforge/forge/1.20.1-$VERSION/forge-1.20.1-$VERSION-universal.jar
CLIENT_SRG=$(find "$LIB/net/minecraft/client" -name 'client-*-srg.jar' | head -1)
for jar in "$PATCHED" "$CLIENT_SRG" "$UNIVERSAL"; do
  out=$FDEV/$(basename "$jar" .jar)-named.jar
  [ -s "$out" ] || "${REMAP[@]}" remap "$SRG" srg named "$jar" "$out" "$NAMED" 2>&1 | grep -v '^\[WARN\]' || true
  [ -s "$out" ] || { echo "remapping $(basename "$jar") failed"; exit 1; }
done
COMPILE="$FDEV/$(basename "$PATCHED" .jar)-named.jar:$FDEV/$(basename "$CLIENT_SRG" .jar)-named.jar:$FDEV/$(basename "$UNIVERSAL" .jar)-named.jar"
# Forge's and Mojang's libraries (not the game jars, not the installer's tools).
LIBS=$(find "$LIB" -name '*.jar' | grep -v -e '/net/minecraft/client/' -e '/forge-1.20.1-' -e installertools -e ForgeAutoRenamingTool -e jarsplitter -e binarypatcher | tr '\n' ':')
MOJANG=$(python3 -I -c "import json,sys;print(':'.join(p for p in json.load(open(sys.argv[1]))['class_path'] if '/libraries/' in p))" "$LAUNCH")
# 3. With the bridge's conversation classes (bridge/remote: JDK types only), which the Fabric
# runtime has on its class path and a Forge game has only from its mods. The shared sources need Fabric API's to compile (FabricLoader120 and its modules ride along
# unused in the Forge jar, never loaded: Loader120 picks the Forge side).
API=$(for m in "$DEV"/fabric-api-named/*.jar; do printf '%s:' "$m"; done)
FABRIC=$(python3 -I -c "import json,sys;print(':'.join(p for p in json.load(open(sys.argv[1]))['class_path'] if '/fabric/' in p))" "$LAUNCH")
"${JAVAC[@]}" -cp "$COMPILE:$LIBS$MOJANG:$API$FABRIC" -d "$TMP/classes" $(find "$SRC/src" "$SRC/forge" "$ROOT/crates/horizon-jvm/java/dev/horizonlab/bridge/remote" -name '*.java')
# 4. The mixin config without the mixins on Fabric API's classes, named in the manifest as Forge
# reads it; mods.toml.
python3 -I - "$SRC/src/horizon-platform120.mixins.json" "$TMP/classes/horizon-platform120.forge.mixins.json" <<'PY'
import json, sys
# On Fabric API's own classes:
fabric_only = {"ClientPlayNetworkingMixin", "ServerPlayNetworkingMixin", "PlayerLookupMixin"}
config = json.load(open(sys.argv[1]))
config.setdefault("minVersion", "0.8")
for side in ("mixins", "client", "server"):
    if side in config:
        config[side] = [m for m in config[side] if m not in fabric_only]
# On Forge's own classes (forge/.../mixin):
config["mixins"] = config.get("mixins", []) + ["NetworkInstanceMixin"]
json.dump(config, open(sys.argv[2], "w"), indent=2)
PY
# Mojang's names of srg method names (m_..._), for the mods' mixins on vanilla's methods
# (Mixins120, Names): from Mojang's mappings, so into the jar in the cache, never the repo.
mkdir -p "$TMP/classes/horizon"
python3 -I - "$SRG" "$TMP/classes/horizon/methods-1.20.1.tsv" <<'PY'
import sys
with open(sys.argv[1]) as f, open(sys.argv[2], "w") as methods:
    names = f.readline().rstrip("\n").split("\t")[3:]
    s, n = names.index("srg"), names.index("named")
    seen = set()
    for line in f:
        if line.startswith("\tm\t"):
            parts = line.rstrip("\n").split("\t")[3:]
            if parts[s].startswith("m_") and parts[s] not in seen:
                seen.add(parts[s])
                methods.write(parts[s] + "\t" + parts[n] + "\n")
PY
mkdir -p "$TMP/classes/META-INF"
cp "$SRC/forge/META-INF/mods.toml" "$TMP/classes/META-INF/"
printf 'Manifest-Version: 1.0\nMixinConfigs: horizon-platform120.forge.mixins.json\n' > "$TMP/manifest"
(cd "$TMP/classes" && "$JAVA_HOME/bin/jar" cfm "$TMP/platform120-forge-named.jar" "$TMP/manifest" .)
# shellcheck disable=SC2046
"${REMAP[@]}" remap "$SRG" named srg "$TMP/platform120-forge-named.jar" "$OUT/platform120-forge.jar" \
  $(echo "$COMPILE:$LIBS$MOJANG" | tr ':' ' ') 2>&1 | grep -v '^\[WARN\]' || true
[ -s "$OUT/platform120-forge.jar" ] || { echo "remapping the platform to srg failed"; exit 1; }
echo "$OUT/platform120-forge.jar"
