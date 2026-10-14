#!/bin/sh
# Builds the extension, assembles a Godot test project from the Spine example exports and runs the
# GDScript tests against it.
#
#   tests/godot/run.sh <spine-runtimes checkout> [path to the Godot binary]
#
# api_test, loader_test and skin_test run headless; cull_test, stale_test, slot_test, attachment_test and rebuilt_mesh_test need a real window (they check
# what the render server reports about visibility and draw calls). Benchmarks and screenshots are separate scenes in this folder
# (bench.tscn, shot.tscn, debug_shot.tscn); see the comments at the top of each script.
set -e
CHECKOUT="$1"
GODOT="${2:-${GODOT:-$(command -v godot || echo /Applications/Godot.app/Contents/MacOS/Godot)}}"
HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../../../.." && pwd)"
PROJECT="$ROOT/target/godot-test-project"

cargo build -p spine2d-godot --release --manifest-path "$ROOT/Cargo.toml"
case "$(uname)" in
    Darwin) LIB=libspine2d_godot.dylib; PACKED=librust_spine2d.dylib ;;
    Linux) LIB=libspine2d_godot.so; PACKED=librust_spine2d.so ;;
    *) LIB=spine2d_godot.dll; PACKED=rust_spine2d.dll ;;
esac

rm -rf "$PROJECT"
mkdir -p "$PROJECT/bin" "$PROJECT/assets"
cp "$HERE"/*.gd "$HERE"/*.tscn "$HERE/project.godot" "$ROOT/crates/spine2d-godot/rust_spine2d.gdextension" "$PROJECT/"
cp "$ROOT/target/release/$LIB" "$PROJECT/bin/$PACKED"
for name in spineboy coin alien tank raptor mix-and-match; do
    mkdir -p "$PROJECT/assets/$name"
    cp "$CHECKOUT/examples/$name/export/"*.json "$CHECKOUT/examples/$name/export/"*.skel \
       "$CHECKOUT/examples/$name/export/"*.atlas "$CHECKOUT/examples/$name/export/"*.png "$PROJECT/assets/$name/"
done

# loader_test loads the same skeleton through the .spine-json extension.
cp "$PROJECT/assets/spineboy/spineboy-pro.json" "$PROJECT/assets/spineboy/spineboy-pro.spine-json"

"$GODOT" --headless --path "$PROJECT" --import --quit > /dev/null 2>&1
failed=0
for scene in api_test loader_test skin_test; do
    "$GODOT" --headless --path "$PROJECT" --scene "res://$scene.tscn" | grep -E "FAIL|RESULT" || failed=1
done
for scene in cull_test stale_test slot_test attachment_test rebuilt_mesh_test; do
    "$GODOT" --path "$PROJECT" --resolution 900x700 --scene "res://$scene.tscn" | grep -E "FAIL|RESULT" || failed=1
done
exit $failed
