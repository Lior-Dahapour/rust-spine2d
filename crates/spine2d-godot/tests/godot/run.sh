#!/bin/sh
# Builds the extension, assembles a Godot test project from the Spine example exports and runs the
# GDScript tests against it.
#
#   tests/godot/run.sh <spine-runtimes checkout> [path to the Godot binary]
#
# api_test, loader_test and skin_test run headless; the others need a real window.
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
cp "$PROJECT/assets/spineboy/spineboy-pro.json" "$PROJECT/assets/spineboy/spineboy-pro.spine-json"

# constraint_kinds_test loads the core's synthetic skeletons: together they hold every kind of
# constraint, which none of the example skeletons does.
mkdir -p "$PROJECT/assets/synthetic"
cp "$ROOT/crates/spine2d-core/tests/fixtures/synthetic/"*-options.json "$PROJECT/assets/synthetic/"

"$GODOT" --headless --path "$PROJECT" --import --quit > /dev/null 2>&1
failed=0
# Runs one scene, prints its FAIL and RESULT lines, and marks the run failed when the scene exits
# with a non-zero status (the tests quit with their number of failures), prints no result, or does
# not finish within two minutes. A scene whose script fails to load never quits, hence the limit.
run_scene() {
    log=$(mktemp)
    "$GODOT" "$@" > "$log" 2>&1 &
    pid=$!
    waited=0
    while kill -0 "$pid" 2>/dev/null; do
        if [ "$waited" -ge 120 ]; then
            kill "$pid"
            echo "FAIL timed out: $*"
            failed=1
            break
        fi
        sleep 1
        waited=$((waited + 1))
    done
    code=0
    wait "$pid" || code=$?
    grep -E "FAIL|RESULT" "$log" || code=1
    [ "$code" -eq 0 ] || failed=1
    rm -f "$log"
}
for scene in api_test loader_test skin_test follow_test events_test constraint_test constraint_kinds_test bone_test constants_test data_setters_test slot_data_test animation_api_test skeleton_api_test resource_api_test bone_node_test sprite_api_test; do
    run_scene --headless --path "$PROJECT" --scene "res://$scene.tscn"
done
for scene in cull_test stale_test slot_test attachment_test visibility_test; do
    run_scene --path "$PROJECT" --resolution 900x700 --scene "res://$scene.tscn"
done
exit $failed
