#!/bin/bash
# CI (game-build.yml, input java_mods): the game built with Fabric Java mods (feature java-mods),
# packaged beside the plain one as <asset>-java. The build has no Minecraft jar: the bridge
# carries its platform mod's sources and builds them on the player's machine against their own
# game (crates/horizon-jvm/src/platform.rs), so HORIZON_BRIDGE_PLATFORM=source.
#   package_java_variant.sh TARGET EXE DIR ASSET     (TARGET may be empty; EXE is "" or ".exe")
# $BIN: the plain build's programs (horizon-ctl, horizon-prepare, horizon-server come from there).
set -euo pipefail
target=$1 exe=$2 dir=$3 asset=$4
export HORIZON_BRIDGE_PLATFORM=source CARGO_TARGET_DIR=target-java
cargo build --release --locked ${target:+--target "$target"} -p horizon-client --features java-mods --bin horizon-client
bin="target-java/${target:+$target/}release"
"$bin/horizon-client$exe" --help | grep -q -- '--resources'
for tool in horizon-ctl horizon-prepare horizon-server; do cp "$BIN/$tool$exe" "$bin/"; done
cargo run --locked -p xtask -- package "$bin/horizon-client$exe" "dist/$dir"
cd dist
rm -f "$asset"
case "$asset" in
  *.tar.gz) tar czf "$asset" "$dir" ;;
  *) 7z a -tzip "$asset" "$dir" > /dev/null ;;
esac
ls -l "$asset"
