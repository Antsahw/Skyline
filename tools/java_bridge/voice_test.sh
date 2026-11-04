#!/bin/bash
# Two games on LAN hearing each other with Simple Voice Chat (or another voice mod in MODS):
# a host and a guest (lan_test.sh), each with its own sound devices on a PulseAudio server of
# null sinks: a tone played into one game's microphone must come out of the other game's
# speakers. Needs pulseaudio, pactl, paplay, parecord and sox (apt install pulseaudio
# pulseaudio-utils sox). Prints what each side heard; screenshots in OUT (lan_test.sh's).
#   MODS=~/vc tools/java_bridge/voice_test.sh
# The voice mod's client settings: CONFIG (default: Simple Voice Chat on voice activation,
# unmuted, no noise suppression, which takes a pure tone for noise).
R=$(cd "$(dirname "$0")/../.." && pwd)
: "${MODS:?MODS: a folder of mod jars (Simple Voice Chat, Fabric API)}" "${OUT:=/tmp}"
mkdir -p "$OUT"
for tool in pulseaudio pactl paplay parecord sox; do
  command -v $tool >/dev/null || { echo "voice_test: needs $tool" >&2; exit 1; }
done
# A sound server of our own (never the user's): null sinks for each game's speakers and mics.
export PULSE_RUNTIME_PATH=$(mktemp -d /tmp/hz-pulse.XXXX)
export PULSE_SERVER=unix:$PULSE_RUNTIME_PATH/native
pulseaudio -n --daemonize=yes --exit-idle-time=-1 --use-pid-file=no \
  -L "module-native-protocol-unix socket=$PULSE_RUNTIME_PATH/native auth-anonymous=1" >/dev/null 2>&1
sleep 1
for s in hostout guestout hostmic guestmic; do pactl load-module module-null-sink sink_name=$s >/dev/null; done
if [ -z "$CONFIG" ]; then
  CONFIG=$(mktemp -d /tmp/hz-voice-config.XXXX)
  mkdir -p "$CONFIG/voicechat"
  # (config_version: without it the mod migrates the file and resets half of it.)
  printf '%s\n' config_version=1 onboarding_finished=true microphone_activation_type=VOICE muted=false \
    denoiser=false voice_activity_detection=false automatic_gain_control=false voice_activation_threshold=-60 \
    > "$CONFIG/voicechat/voicechat-client.properties"
fi
export CONFIG
# The host plays and records on its own devices, the guest on its own.
export PULSE_SINK=hostout PULSE_SOURCE=hostmic.monitor
export GUEST_ENV="PULSE_SINK=guestout PULSE_SOURCE=guestmic.monitor"
tone() { sox -n -r 48000 -c 1 -b 16 -t wav - synth "$3" sine "$2" vol 0.5 2>/dev/null | PULSE_SINK=$1 paplay; }
heard() {  # heard SINK FREQ SECONDS: the tone's share of what the sink played
  parecord --device="$1.monitor" --rate=48000 --channels=1 --format=s16le --file-format=wav "$OUT/voice-$1.wav" &
  local rec=$!; sleep "$3"; kill $rec; wait $rec 2>/dev/null
  python3 - "$OUT/voice-$1.wav" "$2" <<'EOF'
import math, struct, sys, wave
w = wave.open(sys.argv[1]); f = float(sys.argv[2]); rate = w.getframerate()
data = w.readframes(w.getnframes()); n = len(data) // 2
s = struct.unpack("<%dh" % n, data[: n * 2])
# Half-second windows: Goertzel's power at f over the window's power (near 1: the tone is
# what plays). A whole-file Goertzel would be thrown off by any gap or jitter in the voice.
win = rate // 2; tone = total = 0
for i in range(0, n - win, win):
    seg = s[i:i + win]; power = sum(x * x for x in seg) / win
    k = 2 * math.cos(2 * math.pi * f / rate); a = b = 0.0
    for x in seg: a, b = x + k * a - b, a
    total += 1
    if power > 1e4 and (a * a + b * b - k * a * b) * 2 / win / win / power > 0.5: tone += 1
print(f"{sys.argv[1]}: the {f:.0f} Hz tone in {tone} of {total} half-seconds")
EOF
}
voice() {
  sleep 20  # both voice clients connect
  tone guestmic 440 25 & (sleep 6; $G screenshot "$OUT/voice-guest-talking.png" >/dev/null) & heard hostout 440 20; wait
  tone hostmic 660 25 & (sleep 6; $H screenshot "$OUT/voice-host-talking.png" >/dev/null) & heard guestout 660 20; wait
}
export -f tone heard voice; export OUT
# Then the mod's own screen on each side (V: its key for its menu).
"$R/tools/java_bridge/lan_test.sh" voice '$G key v tap; sleep 2' '$G key escape tap; $H key v tap; sleep 2' "$@"
for log in "$(cat /tmp/hz-bridge-scratch)/home/logs/latest.log" "$(cat /tmp/hz-bridge-guest)/home/logs/latest.log"; do
  echo "== voice in $log"
  grep -E "voicechat|device" "$log" | grep -E "connected|authenticat|Connecting|run here|own sound|failed|ERROR|unavailable" | head -12 | cut -c1-220
done
pactl exit 2>/dev/null
