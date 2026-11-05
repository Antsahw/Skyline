#!/bin/bash
# What the bridge costs a player who doesn't use it (maintainers' condition c): startup and frame
# time with `--benchmark standard`, release builds, three ways:
#   upstream   upstream's main (horizon-rust/horizon), the plain game
#   plain      this branch built without the java-mods feature
#   java-off   this branch built with java-mods, the switch off (no --java-mods), one jar in mods-java/
# Each runs REPEATS times (default 3), alternating, in a fresh game folder; the reports
# (docs/benchmark-report.md) go to OUT (default /tmp/hz-perf-off) and the table to OUT/table.md,
# ready for docs/java-bridge-perf.md.
#   tools/java_bridge/perf_off.sh [REPEATS]
#   RESOURCES=…/.local/horizon-client/26.3 (default: this checkout's), UPSTREAM_REF=main,
#   UPSTREAM_URL=https://github.com/horizon-rust/horizon, SKIP_BUILD=1 (reuse OUT's binaries),
#   ARGS="--render-distance 12" (more game arguments; the window size and distance are the options').
# Keep the machine idle and the window on screen; compare only runs of one machine.
set -euo pipefail
R=$(cd "$(dirname "$0")/../.." && pwd)
REPEATS=${1:-3}
OUT=${OUT:-/tmp/hz-perf-off}
V=$(python3 -c "import json;print(json.load(open('$R/research-lock.json'))['minecraft']['version'])")
RES=${RESOURCES:-$R/.local/horizon-client/$V}
[ -d "$RES" ] || { echo "no prepared resources at $RES (RESOURCES=…)" >&2; exit 1; }
mkdir -p "$OUT/bin" "$OUT/reports"

if [ -z "${SKIP_BUILD:-}" ]; then
  # Upstream in a worktree of its own (its own target folder: nothing shared with this branch's).
  up=$OUT/upstream
  git -C "$R" fetch --depth 1 "${UPSTREAM_URL:-https://github.com/horizon-rust/horizon}" "${UPSTREAM_REF:-main}"
  git -C "$R" worktree remove --force "$up" 2>/dev/null || rm -rf "$up"
  git -C "$R" worktree add --detach "$up" FETCH_HEAD
  (cd "$up" && CARGO_TARGET_DIR=$OUT/target-upstream cargo build --release --locked -p horizon-client --bin horizon-client)
  cp "$OUT/target-upstream/release/horizon-client" "$OUT/bin/upstream"
  git -C "$up" rev-parse --short HEAD > "$OUT/bin/upstream.commit"
  (cd "$R" && CARGO_TARGET_DIR=$OUT/target-plain cargo build --release --locked -p horizon-client --bin horizon-client)
  cp "$OUT/target-plain/release/horizon-client" "$OUT/bin/plain"
  # The release's way: the bridge's sources carried, nothing of Fabric's on this machine for it.
  (cd "$R" && HORIZON_BRIDGE_PLATFORM=source CARGO_TARGET_DIR=$OUT/target-java \
    cargo build --release --locked -p horizon-client --features java-mods --bin horizon-client)
  cp "$OUT/target-java/release/horizon-client" "$OUT/bin/java-off"
  git -C "$R" rev-parse --short HEAD > "$OUT/bin/branch.commit"
fi

# A jar in mods-java/ for java-off: the switch, not an empty folder, keeps Java out.
jar=$OUT/some-mod.jar
python3 - "$jar" <<'EOF'
import sys, zipfile
with zipfile.ZipFile(sys.argv[1], "w") as z:
    z.writestr("fabric.mod.json", '{"schemaVersion":1,"id":"perf_probe","version":"1"}')
EOF

run() {
  local label=$1 n=$2 g
  g=$(mktemp -d /tmp/hz-perf-off.XXXX)
  mkdir -p "$g/home" "$g/lh" "$g/data" "$g/conf"
  [ "$label" = java-off ] && mkdir -p "$g/home/mods-java" && cp "$jar" "$g/home/mods-java/"
  (cd "$g/home" && env -u HORIZON_JAVA_MODS -u HORIZON_JAVA_MODS_OWNER -u HORIZON_ROOT \
    HORIZON_HOME="$g/home" HORIZON_LAUNCHER_HOME="$g/lh" XDG_DATA_HOME="$g/data" XDG_CONFIG_HOME="$g/conf" \
    HORIZON_LAUNCHER_NO_SELF_UPDATE=1 HORIZON_NO_AUDIO=1 \
    "$OUT/bin/$label" --resources "$RES" --benchmark standard ${ARGS:-} > "$g/out.txt" 2>&1) || true
  local report
  report=$(ls -t "$g/home/benchmarks/"*.json 2>/dev/null | head -1 || true)
  if [ -z "$report" ]; then echo "$label #$n: no report (log: $g/home/logs/latest.log)" >&2; return; fi
  cp "$report" "$OUT/reports/$label-$n.json"
  if [ "$label" = java-off ]; then
    # The switch off: Java never started, nothing fetched.
    grep -q "Java mods: off (1 jars in mods-java)" "$g/home/logs/latest.log" \
      || echo "java-off #$n: the log doesn't say the mods were off" >&2
    grep -q "Java mods: JVM" "$g/home/logs/latest.log" && echo "java-off #$n: Java started!" >&2
  fi
  echo "$label #$n: $OUT/reports/$label-$n.json"
  rm -rf "$g"
}

for n in $(seq "$REPEATS"); do
  for label in upstream plain java-off; do run "$label" "$n"; done
done

python3 - "$OUT" <<'EOF'
import json, statistics, sys
from pathlib import Path
out = Path(sys.argv[1])
labels = {"upstream": "Upstream main", "plain": "java-bridge, no feature", "java-off": "java-mods build, switch off"}
rows = {}
hw = None
for label in labels:
    reps = [json.loads(p.read_text()) for p in sorted((out / "reports").glob(f"{label}-*.json"))]
    if not reps:
        continue
    hw = hw or reps[0].get("hardware", {})
    def metric(name):
        vals = [r["metrics"][name]["mean"] for r in reps if name in r.get("metrics", {})]
        return vals
    frames = [f for r in reps for f in r.get("frames", [])]
    def frame(key):
        return [f[key] for f in frames if key in f]
    rows[label] = dict(n=len(reps), title=metric("startup.title_ms"), world=metric("startup.world_ms"),
                       fps=frame("avg_fps"), p50=frame("p50_ms"), p99=frame("p99_ms"),
                       low1=frame("low_1pct_fps"), rss=metric("memory.rss_peak_mb"),
                       size=metric("game.binary_bytes"))
def cell(vals, fmt="{:.0f}"):
    if not vals:
        return "—"
    m = statistics.mean(vals)
    return (fmt.format(m) + (" ± " + fmt.format(statistics.stdev(vals)) if len(vals) > 1 else ""))
lines = []
commit = lambda f: (out / "bin" / f).read_text().strip() if (out / "bin" / f).exists() else "?"
gpu = (hw or {}).get("gpu", {})
lines.append(f"Machine: {(hw or {}).get('cpu', '?')}, {gpu.get('name', '?')} ({gpu.get('backend', '?')}), "
             f"{(hw or {}).get('os', '?')} {(hw or {}).get('os_version', '')}. Upstream {commit('upstream.commit')}, "
             f"branch {commit('branch.commit')}. `--benchmark standard`, mean ± stdev over runs (frames: over every "
             f"scene's flight).")
lines.append("")
lines.append("| | " + " | ".join(labels[l] for l in rows) + " |")
lines.append("|---|" + "---|" * len(rows))
for name, key, fmt in [("Runs", None, None), ("Start to title (ms)", "title", "{:.0f}"),
                       ("Start to in-world (ms)", "world", "{:.0f}"), ("Average FPS", "fps", "{:.0f}"),
                       ("Frame p50 (ms)", "p50", "{:.2f}"), ("Frame p99 (ms)", "p99", "{:.2f}"),
                       ("1% low FPS", "low1", "{:.0f}"), ("Peak RSS (MB)", "rss", "{:.0f}"),
                       ("Binary (MB)", "size", "{:.1f}")]:
    if key is None:
        lines.append(f"| {name} | " + " | ".join(str(r["n"]) for r in rows.values()) + " |")
    elif key == "size":
        lines.append(f"| {name} | " + " | ".join(cell([v / 1e6 for v in r[key]], fmt) for r in rows.values()) + " |")
    else:
        lines.append(f"| {name} | " + " | ".join(cell(r[key], fmt) for r in rows.values()) + " |")
text = "\n".join(lines) + "\n"
(out / "table.md").write_text(text)
print(text)
EOF
