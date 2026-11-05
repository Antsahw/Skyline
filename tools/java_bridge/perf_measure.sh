#!/bin/bash
# Measures a headless game with the Java mods of MODS (or none: MODS unset): start time, the
# process's memory (RSS, and the JVM's own: heap, native memory by area, the live heap's
# biggest classes), server tick times standing and walking (/tick query, the Java profile).
# Writes everything to OUT (default /tmp/hz-perf/<name>); prints a summary.
#   MODS=~/hz-test/mods-java CACHE=/tmp/pack-cache tools/java_bridge/perf_measure.sh pack
#   tools/java_bridge/perf_measure.sh vanilla
# STAND (s, default 60) and WALK (s, default 30) set the phases; the run starts once the world is
# joined. Uses only scratch folders (CLAUDE.md), and attaches jcmd to the game's own JVM
# (-XX:+StartAttachListener: the bridge's JVM runs with -Xrs, which leaves SIGQUIT alone).
set -u
R=$(cd "$(dirname "$0")/../.." && pwd)
NAME=${1:-run}
OUT=${OUT:-/tmp/hz-perf/$NAME}; rm -rf "$OUT"; mkdir -p "$OUT"
: "${STAND:=60}" "${WALK:=30}"
J=$(ls -d /usr/lib/jvm/*25* 2>/dev/null | head -1)
export JAVA_HOME=$J HORIZON_JAVA_HOME=$J PATH=$J/bin:$PATH
export HORIZON_JAVA_PROFILE=1
export HORIZON_JVM_OPTIONS="-XX:+StartAttachListener -XX:NativeMemoryTracking=summary -Xlog:gc:file=$OUT/gc.log ${HORIZON_JVM_OPTIONS:-}"
if [ -z "${MODS:-}" ]; then
  # No mods: launch.sh with an empty folder and the Java side off.
  MODS=$(mktemp -d); touch "$MODS/none.jar"; NOJAVA=1
fi
t0=$(date +%s.%N)
S=$(MODS="$MODS" JAVA_MODS=${NOJAVA:+0} "$R/tools/java_bridge/launch.sh") || { echo "launch failed"; exit 1; }
t1=$(date +%s.%N)
C="$R/target/fast/horizon-ctl --socket $S/ctl.sock"
PID=$(pgrep -f "horizon-client.*$S" | head -1)
echo "scratch $S pid $PID" | tee "$OUT/run.txt"
echo "join_s $(awk "BEGIN{printf \"%.1f\", $t1 - $t0}")" | tee -a "$OUT/run.txt"
sample() { # name
  local n=$1
  grep -E 'VmRSS|VmHWM|RssAnon|RssFile' /proc/$PID/status > "$OUT/$n.status"
  if [ -z "${NOJAVA:-}" ]; then
    jcmd $PID GC.heap_info > "$OUT/$n.heap" 2>&1
    jcmd $PID VM.native_memory summary > "$OUT/$n.nmt" 2>&1
  fi
  $C command "tick query" > "$OUT/$n.tick" 2>&1
  $C screenshot "$OUT/$n.png" >/dev/null 2>&1
}
# JFR over the measured phases: CPU samples (where tick time goes) and allocations (direct buffers).
[ -z "${NOJAVA:-}" ] && jcmd $PID JFR.start name=perf settings=profile filename=$OUT/perf.jfr >/dev/null 2>&1
sleep "$STAND"; sample stand
$C key w down >/dev/null; $C key ctrl down >/dev/null
sleep "$WALK"; $C key w up >/dev/null; $C key ctrl up >/dev/null
sample walk
sleep 20; sample after
if [ -z "${NOJAVA:-}" ]; then
  jcmd $PID JFR.stop name=perf >/dev/null 2>&1
  jcmd $PID GC.class_histogram > "$OUT/histo.txt" 2>&1   # after a full GC: live objects only
  grep -E 'VmRSS' /proc/$PID/status > "$OUT/aftergc.status"
  jcmd $PID GC.heap_info > "$OUT/aftergc.heap" 2>&1
  jcmd $PID VM.native_memory summary > "$OUT/aftergc.nmt" 2>&1
fi
cp "$S/home/logs/latest.log" "$OUT/latest.log"
$C quit >/dev/null 2>&1
for n in stand walk after aftergc; do
  [ -f "$OUT/$n.status" ] && echo "$n $(grep VmRSS "$OUT/$n.status" | tr -s ' ')"
  [ -f "$OUT/$n.tick" ] && grep -o 'Average time per tick: [0-9.]*ms\|P50[^"]*' "$OUT/$n.tick" | tr '\n' ' ' && echo
done
grep -h "Pause Full" "$OUT/gc.log" 2>/dev/null
echo "results in $OUT"
