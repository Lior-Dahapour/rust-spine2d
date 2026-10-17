# Benchmarks

Results of the benchmarks in this folder on an Apple M3 Pro (macOS, Metal), release builds, one
scene per process, nothing in parallel. All numbers are frame times in milliseconds. The Godot
table was run on 2026-10-17 (one pass).

| Folder | What it measures |
|---|---|
| `godot-bench/` | official spine-godot against spine2d-godot, Godot 4.7.1, Forward+ release exports, scripted scenes (constraints, materials, skins, nodes, events) |

The benchmark reads the Spine example art from the git-ignored `bench/assets/`, which
`bench/setup-assets.sh <spine-runtimes checkout>` fills. How to run it is in
`godot-bench/README.md`.

## How to read the numbers

- The display paces light scenes: the empty scene (about 5 ms here) is the cost of presenting a
  frame, and anything near 10.00 ms is a throttled frame loop, not the plugin. `run.py` rejects
  runs within 0.15 ms of 10.00 ms and retries them. Trust the heavy scenes (500 skeletons and up)
  most; for the light ones the ratio is a lower bound.
- The machine was not fully quiet (the runs were done while the session was open), so small
  differences in the light scenes are noise.

## 1. Godot: official spine-godot against spine2d-godot

Run on 2026-10-17, Apple M3 Pro, Godot 4.7.1, Forward+ release exports, **one pass** of 600 frames
per scene (not a median), with the current ECS runtime. Frame time in ms; a scene that averaged
more than 100 ms per frame after 20 measured frames was stopped and is shown as "> 100". The
scripted scenes (churn, constraints, materials, skins, nodes, events) run the same script on both
plugins and only call what exists in the official plugin's API.

| Scene | Official ms | Ours ms | Ours vs official | Draw calls (official / ours) | Peak MB (official / ours) |
|---|---|---|---|---|---|
| empty scene | 5.02 | 5.37 | 0.94x (floor) | 0 / 0 | 301 / 298 |
| spineboy x100 | 6.71 | 2.72 | 2.47x | 1800 / 100 | 360 / 314 |
| spineboy x500 | 38.55 | 7.19 | 5.36x | 9000 / 500 | 580 / 359 |
| spineboy x1000 | 76.50 | 9.63 | 7.94x | 18000 / 1000 | 852 / 418 |
| spineboy x5000 | > 100 | 44.61 | > 2.2x | - / 5000 | - / 858 |
| spineboy churn x3000 | > 100 | 72.55 | > 1.4x | - / 5436 | - / 1216 |
| mixed churn x3000 | crashed | > 100 | - | - | - |
| mixed constraints x300 | 56.42 | 9.12 | 6.18x | 13118 / 2100 | 637 / 410 |
| spineboy materials x500 | 39.10 | 6.57 | 5.95x | 9000 / 500 | 579 / 360 |
| mix-and-match skins x300 | 34.87 | 5.94 | 5.87x | 5014 / 712 | 543 / 388 |
| spineboy nodes x300 | 23.12 | 7.46 | 3.10x | 5400 / 600 | 473 / 344 |
| spineboy events x1000 | 80.30 | 9.28 | 8.66x | 18000 / 1000 | 844 / 419 |
| coin x200 | 9.04 | 3.87 | 2.34x | 800 / 800 | 341 / 341 |
| alien x200 | 10.94 | 3.58 | 3.05x | 3400 / 1000 | 397 / 349 |
| raptor x200 | 21.93 | 5.90 | 3.72x | 7200 / 800 | 473 / 356 |
| tank x100 | 41.42 | 6.50 | 6.38x | 14336 / 2200 | 636 / 380 |

What the scenes do:

- **churn**: every frame a script moves sprites, sets, queues and clears animations, changes time
  scales and replaces a few sprites (`mixed` is five skeleton kinds).
- **constraints**: every frame a script finds constraints by name and writes their mix, position
  and so on.
- **materials**: half the sprites get a custom material per blend mode, and every frame a few get
  one, lose it or swap it.
- **skins**: every frame a few sprites get a new skin built from the base skin and random
  accessories.
- **nodes**: every sprite has a bone node following the head, a bone node driving the front fist
  (moved by the script every frame) and a slot node on the gun.
- **events**: every sprite listens to its animation events (`footstep`) and the script counts them.

Read the table with these in mind:

- It is one pass. A second pass of `spineboy x1000` measured 72.2 (official) and 9.4 (ours) ms,
  so differences of a few percent are noise.
- The empty scene shows the cost of presenting a frame on this machine, about 5 ms. Ours rows below
  about 8 ms are close to it, so the ratios shown for them are lower bounds, not exact.
- The draw call gap is mostly batching: ours merges the consecutive slots of a sprite that share a
  texture, blend mode and tint, the official plugin draws roughly one batch per slot. Custom
  materials did not add draw calls for ours.
- The official plugin **crashed** on `mixed churn x3000`: Godot logs "Element limit reached" for
  its RID allocator and then dereferences null. Ours finishes the scene, but slowly (> 100 ms).
  `spineboy x10000` was not run (the official plugin crashed on it in an earlier run).
- Each new scene ends with a cheap check that it did its work (events were received, skins and
  materials were set, the bone node sits near its bone), and a scene whose check fails is
  rejected. The runs here only compare time, memory and draw calls: they do not compare the
  resulting poses or pictures of the two plugins in the new scenes. Picture and pose equality of the plain
  scenes (spineboy, alien, raptor, tank, coin) was checked against the first extension in an
  earlier session; it has not been repeated for the current runtime.

## Not measured

GPU time, loading time, and any platform other than macOS arm64. Memory is the peak footprint of the whole process.
