#!/bin/sh
# Fills bench/assets with the Spine example art that every benchmark here reads. The art is not
# stored in git and must never be committed.
#
#   bench/setup-assets.sh <spine-runtimes checkout>
#
# The files are copied from the checkout's examples/<name>/export folders (branch 4.3): the
# binary skeleton, and the plain and premultiplied-alpha (-pma) atlas with its page image.
set -e
if [ $# -lt 1 ]; then
    sed -n '2,9p' "$0" >&2
    exit 1
fi
HERE="$(cd "$(dirname "$0")" && pwd)"
CHECKOUT="$1"

rm -rf "$HERE/assets"
for name in spineboy coin alien raptor tank mix-and-match; do
    dest="$HERE/assets/$name"
    mkdir -p "$dest"
    src="$CHECKOUT/examples/$name/export"
    cp "$src/$name-pro.skel" "$src/$name.atlas" "$src/$name.png" \
        "$src/$name-pma.atlas" "$src/$name-pma.png" "$dest/"
done
echo "assets are ready in $HERE/assets"
