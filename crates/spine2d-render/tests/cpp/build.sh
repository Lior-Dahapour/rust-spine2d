#!/bin/sh
# Builds the C++ binaries that tests/golden_render.rs and examples/render_bench.rs compare against:
# render_driver (golden output) and render_bench (timing, built with -O3 -flto).
#
#   tests/cpp/build.sh <spine-runtimes checkout> <output dir>
#
# Then, from the crate directory:
#
#   SPINE_RUNTIMES_DIR=<checkout> SPINE_CPP_RENDER_DRIVER=<output dir>/render_driver \
#   cargo test --release --test golden_render
#
# Benchmarking: spine-cpp defines tiny accessors (BonePose::getA, getWorldX, ...) in .cpp files, so
# without LTO they cannot inline and C++ looks far slower than it is. Always compare against
# render_bench built here, and build the Rust side with
#   CARGO_PROFILE_RELEASE_LTO=fat CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1 cargo build --release \
#     -p spine2d-render --example render_bench
# Run the binaries one at a time, interleaved, and compare the best of several runs.
#
# Both binaries are built with `-O3 -flto`, like an optimized plugin build. With link-time
# optimization clang merges `sin(x)` and `cos(x)` of the same angle into one fused call, and the
# Rust port does the same in release builds, so run the golden tests with `--release`.
#
# `-ffp-contract=off` matters: on ARM64 clang otherwise fuses a*b+c into fused multiply-adds,
# which Rust never does, and results then differ in the last bit.
set -e
CXX="${CXX:-clang++}"
SRC="$1/spine-cpp"
OUT="$2"
HERE="$(cd "$(dirname "$0")" && pwd)"
mkdir -p "$OUT"
"$CXX" -std=c++11 -O3 -flto -w -ffp-contract=off -I"$SRC/include" -o "$OUT/render_driver" "$HERE/render_driver.cpp" "$SRC"/src/spine/*.cpp
"$CXX" -std=c++11 -O3 -flto -w -ffp-contract=off -I"$SRC/include" -o "$OUT/render_bench" "$HERE/../../benches/cpp/bench.cpp" "$SRC"/src/spine/*.cpp
echo "built $OUT/render_driver and $OUT/render_bench"
