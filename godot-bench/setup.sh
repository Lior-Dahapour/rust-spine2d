#!/bin/sh
# Fills the two benchmark projects with everything that is not stored in git.
#
#   godot-bench/setup.sh <spine-runtimes checkout> <official extension dir> [<spine2d addon dir>]
#
# <official extension dir> is the official spine-godot `example-v4-extension/bin` after building
# it with scons (it holds spine_godot_extension.gdextension and the libraries for the system it was
# built on: macos/*.framework, linux/*.so or windows/*.dll). The spine2d
# addon dir defaults to the output of crates/spine2d-godot/package.sh (target/package/addons/rust_spine2d),
# which is built first if it does not exist. The example art is copied from the spine-runtimes
# checkout into the git-ignored assets folders; it must never be committed.
set -e
if [ $# -lt 2 ]; then
    sed -n '2,10p' "$0" >&2
    exit 1
fi
HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/.." && pwd)"
CHECKOUT="$1"
OFFICIAL_BIN="$2"
OURS_ADDON="${3:-$ROOT/target/package/addons/rust_spine2d}"

if [ ! -d "$OURS_ADDON" ]; then
    "$ROOT/crates/spine2d-godot/package.sh"
fi

for project in official ours; do
    cp "$HERE/shared/bench.gd" "$HERE/shared/bench.tscn" "$HERE/shared/export_presets.cfg" \
        "$HERE/$project/"
    rm -rf "$HERE/$project/assets"
    for name in spineboy coin alien raptor tank; do
        dest="$HERE/$project/assets/$name"
        mkdir -p "$dest"
        src="$CHECKOUT/examples/$name/export"
        cp "$src/$name-pro.skel" "$src/$name.atlas" "$src/$name.png" "$dest/"
    done
done

rm -rf "$HERE/official/bin" "$HERE/ours/addons"
mkdir -p "$HERE/official/bin" "$HERE/ours/addons"
cp -R "$OFFICIAL_BIN/." "$HERE/official/bin/"
cp -R "$OURS_ADDON" "$HERE/ours/addons/rust_spine2d"
echo "projects are ready; run godot-bench/run.py"
