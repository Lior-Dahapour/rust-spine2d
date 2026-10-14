#!/bin/sh
# Builds the C++ reference binaries the golden tests compare against.
#
#   tests/cpp/build.sh <spine-runtimes checkout> <output dir>
#
# Produces <output dir>/headless (spine-cpp's HeadlessTest) and <output dir>/driver
# (tests/cpp/driver.cpp). Then run, from the crate directory:
#
#   SPINE_RUNTIMES_DIR=<checkout> \
#   SPINE_CPP_HEADLESS=<output dir>/headless \
#   SPINE_CPP_DRIVER=<output dir>/driver \
#   cargo test --release --test golden --test golden_driver
#
# The compiler is clang++ unless the CXX environment variable names another one.
#
# The reference is built with `-O3 -flto`, like an optimized plugin build. With link-time
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
FLAGS="-std=c++11 -O3 -flto -w -ffp-contract=off -I$SRC/include"
"$CXX" $FLAGS -o "$OUT/headless" "$SRC"/src/spine/*.cpp "$SRC/tests/HeadlessTest.cpp"
"$CXX" $FLAGS -o "$OUT/driver" "$HERE/driver.cpp" "$SRC"/src/spine/*.cpp
echo "built $OUT/headless and $OUT/driver"
