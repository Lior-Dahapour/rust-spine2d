#!/bin/sh
# Fills the two benchmark projects with everything that is not stored in git.
#
#   bench/godot-bench/setup.sh <official extension dir> [<spine2d addon dir>]
#
# <official extension dir> is the official spine-godot `example-v4-extension/bin` after building
# it with scons (it holds spine_godot_extension.gdextension and the libraries for the system it was
# built on: macos/*.framework, linux/*.so or windows/*.dll). The spine2d
# addon dir defaults to the output of crates/spine2d-godot/package.sh (target/package/addons/rust_spine2d),
# which is built first if it does not exist. The example art is copied from bench/assets (fill it
# with bench/setup-assets.sh) into the git-ignored assets folders; it must never be committed.
set -e
if [ $# -lt 1 ]; then
    sed -n '2,10p' "$0" >&2
    exit 1
fi
HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
ART="$HERE/../assets"
OFFICIAL_BIN="$1"
OURS_ADDON="${2:-$ROOT/target/package/addons/rust_spine2d}"
PROJECTS="official ours"

if [ ! -d "$ART" ]; then
    echo "bench/assets is missing; run bench/setup-assets.sh first" >&2
    exit 1
fi
if [ ! -d "$OURS_ADDON" ]; then
    "$ROOT/crates/spine2d-godot/package.sh"
fi

for project in $PROJECTS; do
    cp "$HERE/shared/bench.gd" "$HERE/shared/bench.tscn" "$HERE/shared/export_presets.cfg" \
        "$HERE/$project/"
    rm -rf "$HERE/$project/assets"
    for name in spineboy coin alien raptor tank mix-and-match; do
        dest="$HERE/$project/assets/$name"
        mkdir -p "$dest"
        cp "$ART/$name/$name-pro.skel" "$ART/$name/$name.atlas" "$ART/$name/$name.png" "$dest/"
        if [ "$project" = official ]; then
            # The benchmark script reads the official plugin's files as raw bytes (load_from_file),
            # and an export only keeps the raw file when the importer is "keep".
            for raw in "$dest/$name-pro.skel" "$dest/$name.atlas"; do
                printf '[remap]\n\nimporter="keep"\n' > "$raw.import"
            done
        fi
    done
done

rm -rf "$HERE/official/bin" "$HERE/ours/addons"
mkdir -p "$HERE/official/bin" "$HERE/ours/addons"
cp -R "$OFFICIAL_BIN/." "$HERE/official/bin/"
cp -R "$OURS_ADDON" "$HERE/ours/addons/rust_spine2d"
echo "projects are ready; run bench/godot-bench/run.py"
