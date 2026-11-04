#!/bin/bash
# The Java mods from a normal install, as the launcher starts the game: a scratch install laid
# out like a launcher version (`<version>/horizon-client`, research-lock.json, the preparation's
# `.local/minecraft/<v>` and `.local/horizon-client/<v>`, linked from this checkout), the game run
# in a scratch game folder holding mods-java/, with only `--resources` pointing at the install
# and the Java the launcher passes (HORIZON_JAVA_HOME: this machine's Java 25): no HORIZON_ROOT,
# JAVA_HOME, HORIZON_JAVA_JARS, and no java on the PATH. The bridge then finds the install, its
# game jar and libraries, and fetches Fabric's libraries. Steps see G (horizon-ctl); a
# screenshot after each in OUT (default /tmp): install-N.png.
#   MODS=~/mods/fd tools/java_bridge/install_test.sh 'sleep 3'
#   KEEP_JAVA=1 ...: leave the PATH's Java (only the install layout is tested)
#   NO_JAVA=1 ...: no Java passed and none to find: the game downloads none (that's the
#   launcher's, after its sign-in) and says why the mods didn't start
#   OFF=1 ...: without --java-mods, as the launcher starts an instance with its Fabric switch off:
#   no Java may start (or be downloaded), jars in mods-java/ or not
R=$(cd "$(dirname "$0")/../.." && pwd)
OPT_IN=--java-mods; [ -n "$OFF" ] && OPT_IN=
: "${MODS:?MODS: a folder of mod jars}" "${OUT:=/tmp}"
mkdir -p "$OUT"
CTL="$R/target/fast/horizon-ctl"
V=$(python3 -c "import json;print(json.load(open('$R/research-lock.json'))['minecraft']['version'])")
old=$(cat /tmp/hz-bridge-install 2>/dev/null || true)
if [ -n "$old" ] && [ -d "$old" ]; then
  "$CTL" --socket "$old/ctl.sock" quit >/dev/null 2>&1
  for _ in $(seq 60); do pgrep -f "$old/ctl.sock" >/dev/null || break; sleep 0.5; done
  rm -rf "$old" 2>/dev/null || true
fi
I=$(mktemp -d /tmp/hz-install.XXXX); echo "$I" > /tmp/hz-bridge-install
# The install: the game binary (its bridge jars built in) and what a preparation leaves.
mkdir -p "$I/version/.local/minecraft/$V" "$I/version/.local/horizon-client" "$I/home/mods-java" \
  "$I/lh" "$I/data" "$I/conf"
cp "$R/target/fast/horizon-client" "$I/version/horizon-client"
cp "$R/research-lock.json" "$I/version/"
for f in jars libraries manifest.json; do ln -s "$R/.local/minecraft/$V/$f" "$I/version/.local/minecraft/$V/$f"; done
ln -s "$R/.local/horizon-client/$V" "$I/version/.local/horizon-client/$V"
cp "$MODS"/*.jar "$I/home/mods-java/"
"$R/tools/java_bridge/fps_cap.sh" "$I/home"
if [ -z "$DISPLAY" ]; then
  pgrep -x Xvfb >/dev/null || (nohup Xvfb :77 -screen 0 1280x1024x24 >/tmp/xvfb.log 2>&1 &)
  export DISPLAY=:77; sleep 1
fi
# No Java the bridge could find: no Java on the PATH, and installed ones not looked at
# (HORIZON_JAVA_NO_SYSTEM: only Minecraft's and the launcher's runtimes).
path=/usr/local/bin:/usr/bin:/bin; nosys=1
[ -n "$KEEP_JAVA" ] && path=$PATH && nosys=0
# The launcher's Java for the mods (java_variant::switch_command): this machine's Java 25.
jh=${HORIZON_JAVA_HOME:-}
if [ -z "$jh" ]; then
  for c in "$JAVA_HOME" /usr/lib/jvm/java-25* "$(dirname "$(dirname "$(readlink -f "$(command -v java)")")")"; do
    [ -x "$c/bin/javac" ] && grep -q 'JAVA_VERSION="25' "$c/release" 2>/dev/null && jh=$c && break
  done
fi
[ -n "$NO_JAVA" ] && jh=
# The game folder is HORIZON_HOME (the launcher starts the game there; horizon-ctl too).
cd "$I/home"
env -u HORIZON_ROOT -u HORIZON_JAVA_HOME -u JAVA_HOME -u HORIZON_JAVA_JARS -u JAVA_TOOL_OPTIONS \
  ${jh:+HORIZON_JAVA_HOME=$jh} PATH=$path HORIZON_JAVA_NO_SYSTEM=$nosys HORIZON_HOME=$I/home HORIZON_LAUNCHER_HOME=$I/lh XDG_DATA_HOME=$I/data XDG_CONFIG_HOME=$I/conf \
  HORIZON_LAUNCHER_NO_SELF_UPDATE=1 HORIZON_NO_AUDIO=1 HORIZON_CTL_LAUNCH_TIMEOUT=900 \
  "$CTL" --socket "$I/ctl.sock" launch --world W --seed "${SEED:-1234}" --creative --wait --offscreen $OPT_IN \
  --game "$I/version/horizon-client" --resources "$I/version/.local/horizon-client/$V" > "$I/ctl.out" 2>&1
G="$CTL --socket $I/ctl.sock"
n=0
for step in "$@"; do
  eval "$step"; sleep 1; n=$((n+1))
  $G screenshot "$I/s$n.png" >/dev/null; cp "$I/s$n.png" "$OUT/install-$n.png"
done
$G screenshot "$I/s0.png" >/dev/null; cp "$I/s0.png" "$OUT/install-0.png"
log=$I/home/logs/latest.log
grep -E "Java mods: (JVM|not started|Fabric can't|downloading)|Java mods: Fabric loaded" "$log" | cut -c1-240
grep -E "failed|could not|    at " "$log" | grep -v "content:" | head -15 | cut -c1-240
echo "install: $I  screenshots: $OUT/install-*.png  log: $log"
