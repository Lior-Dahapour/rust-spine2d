#!/bin/sh
# Builds the extension with full optimization for this machine and assembles a Godot addon folder.
#
#   crates/spine2d-godot/package.sh [--out DIR] [--verify <spine-runtimes checkout> [godot binary]]
#
# Result: DIR/addons/rust_spine2d/{rust_spine2d.gdextension, bin/<library>, LICENSE-SPINE, README.md}
# (DIR defaults to target/package). Copy the `addons` folder into a Godot 4.5+ project.
#
# On macOS the library is universal (arm64 + x86_64) when both Rust targets are installed
# (`rustup target add x86_64-apple-darwin`); otherwise it is the host architecture only. Linux and
# Windows libraries come from building on those systems; .github/workflows/godot-addon.yml does
# that and merges the results into one addon.
#
# --verify assembles a clean Godot project that contains nothing but the addon plus the example
# assets, and runs the headless GDScript tests against it.
set -e
HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
OUT="$ROOT/target/package"
CHECKOUT=""
GODOT="${GODOT:-$(command -v godot || echo /Applications/Godot.app/Contents/MacOS/Godot)}"
while [ $# -gt 0 ]; do
    case "$1" in
        --out) OUT="$2"; shift 2 ;;
        --verify) CHECKOUT="$2"; shift 2; if [ $# -gt 0 ] && [ "${1#--}" = "$1" ]; then GODOT="$1"; shift; fi ;;
        *) echo "unknown argument: $1" >&2; exit 1 ;;
    esac
done

# Full optimization, no machine-specific flags: the library must run on other computers.
export CARGO_PROFILE_RELEASE_LTO=fat
export CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1
export CARGO_TARGET_DIR="$ROOT/target/package-build"

ADDON="$OUT/addons/rust_spine2d"
rm -rf "$ADDON"
mkdir -p "$ADDON/bin"

build() {
    cargo build -p spine2d-godot --release --manifest-path "$ROOT/Cargo.toml" "$@"
}

case "$(uname)" in
    Darwin)
        LIB=libspine2d_godot.dylib
        PACKED=librust_spine2d.dylib
        installed="$(rustup target list --installed 2>/dev/null || true)"
        if echo "$installed" | grep -q aarch64-apple-darwin && echo "$installed" | grep -q x86_64-apple-darwin; then
            build --target aarch64-apple-darwin
            build --target x86_64-apple-darwin
            lipo -create \
                "$CARGO_TARGET_DIR/aarch64-apple-darwin/release/$LIB" \
                "$CARGO_TARGET_DIR/x86_64-apple-darwin/release/$LIB" \
                -output "$ADDON/bin/$PACKED"
        else
            build
            cp "$CARGO_TARGET_DIR/release/$LIB" "$ADDON/bin/$PACKED"
        fi
        ;;
    Linux)
        LIB=libspine2d_godot.so
        PACKED=librust_spine2d.so
        build
        cp "$CARGO_TARGET_DIR/release/$LIB" "$ADDON/bin/$PACKED"
        ;;
    *)
        LIB=spine2d_godot.dll
        PACKED=rust_spine2d.dll
        build
        cp "$CARGO_TARGET_DIR/release/$LIB" "$ADDON/bin/$PACKED"
        ;;
esac

cp "$HERE/rust_spine2d.gdextension" "$ADDON/"
cp "$ROOT/LICENSE-SPINE" "$ADDON/"
cp "$HERE/README.md" "$ADDON/"
echo "packaged $ADDON"
ls -la "$ADDON/bin"

if [ -n "$CHECKOUT" ]; then
    PROJECT="$ROOT/target/clean-project"
    rm -rf "$PROJECT"
    mkdir -p "$PROJECT/assets" "$PROJECT/addons"
    cp -R "$ADDON" "$PROJECT/addons/"
    cat > "$PROJECT/project.godot" <<'PROJECT_FILE'
config_version=5

[application]
config/name="rust-spine2d-clean-project"

[rendering]
renderer/rendering_method="gl_compatibility"
PROJECT_FILE
    cp "$HERE/tests/godot/api_test."* "$HERE/tests/godot/loader_test."* "$PROJECT/"
    for name in spineboy; do
        mkdir -p "$PROJECT/assets/$name"
        cp "$CHECKOUT/examples/$name/export/"*.json "$CHECKOUT/examples/$name/export/"*.skel \
           "$CHECKOUT/examples/$name/export/"*.atlas "$CHECKOUT/examples/$name/export/"*.png "$PROJECT/assets/$name/"
    done
    cp "$PROJECT/assets/spineboy/spineboy-pro.json" "$PROJECT/assets/spineboy/spineboy-pro.spine-json"
    # Import by running the editor for a while, not with `--import --quit`: quitting within the
    # first few frames crashes Godot 4.7.1 with any gdext 0.5.5 extension that registers a class
    # (see the README, "Limitations"). Running 120 frames avoids it.
    "$GODOT" --headless --editor --path "$PROJECT" --quit-after 120 > /dev/null 2>&1
    failed=0
    for scene in api_test loader_test; do
        "$GODOT" --headless --path "$PROJECT" --scene "res://$scene.tscn" | grep -E "FAIL|RESULT" || failed=1
    done
    exit $failed
fi
