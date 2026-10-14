# godot-bench

A frame-time benchmark of the official spine-godot extension against spine2d-godot, in two
identical Godot projects that differ only in the extension they contain. Both projects use the
Forward+ renderer and are exported with the release export template, so the engine, the
extension and the game scripts all run as release builds. Nothing here touches the editor build.

It runs on macOS, Linux and Windows. Only macOS (arm64) has been tried; the Linux and Windows export
presets and paths were written but never run, so expect to adjust small things there.

```
godot-bench/
  official/     project.godot for the official spine-godot extension
  ours/         project.godot for spine2d-godot
  shared/       bench.gd, bench.tscn and export_presets.cfg, copied into both projects
  setup.sh      fills the projects with the art, the extensions and the shared files
  run.py        exports both projects, runs every scene, prints the comparison table
```

The Spine example art, the built extensions and the export output are git-ignored. The art in
particular must never be committed.

## What a run does

For each scene, `bench.gd` spawns N sprites on a 900x700 window with vsync off and no frame cap,
lets the engine update and draw them (there are no per-frame script calls), skips 120 frames, then
records the wall time of the next 600. It reports the mean, median and 99th percentile frame time,
the frame rate, and the draw calls and primitives of the last frame. The script prints the active
renderer and whether the build is a release build, and `run.py` rejects any run that is not
Forward+ and release. Builds alternate between the two plugins and nothing runs in parallel. A
pass runs every scene once per plugin; the pass is repeated `--runs` times (3 by default, the order
of the plugins flipped each time), which gives one table per pass and a conclusion table with the
median and the range.

Some runs on macOS report exactly 10.00 ms per frame, which is a throttled frame loop and not a
measurement (it happens to both plugins, at random). `run.py` rejects such runs and repeats the
scene up to four times. Machine load still shifts the small scenes by a lot, so close other
applications before a run.

The first row is an empty scene. It shows the fixed cost of presenting a frame on this machine, so
the other rows can be read relative to it.

## Requirements

On every system:

- Godot 4.5 or newer, plus its **export templates** for the same version and the system you run on
  (in the editor: Editor, Manage Export Templates; or download the `.tpz` from the Godot site). The
  release template is enough.
- Python 3.10 or newer, to run `run.py`.
- Rust and the build tools for it (see the [root README](../README.md#requirements-and-build)), to
  build the spine2d addon.
- `git`, and `scons` (`pip install scons`) with a C++ toolchain, to build the official extension.
- A checkout of [spine-runtimes](https://github.com/EsotericSoftware/spine-runtimes) on the `4.3`
  branch. Its `examples/<name>/export` folders supply the art (spineboy, coin, alien, raptor, tank).
- A GPU and driver that Godot's Forward+ renderer supports (Metal on macOS, Vulkan on Linux, Vulkan
  or Direct3D 12 on Windows), and a desktop session: the games open a real window.

| System | Toolchain for the official extension | `godot` executable |
|---|---|---|
| macOS | Xcode Command Line Tools | `/Applications/Godot.app/Contents/MacOS/Godot` (found automatically) |
| Linux | `g++` or `clang++` | Put it on `PATH` as `godot`, or set `GODOT=/path/to/Godot_v4.5-stable_linux.x86_64` |
| Windows | Visual Studio Build Tools (MSVC) | Set `GODOT` to the full path, for example `C:\Godot\Godot_v4.5-stable_win64.exe` |

`setup.sh` is a shell script: on Windows run it in Git Bash. `run.py` is plain Python and runs in
PowerShell, `cmd` or Git Bash.

## Steps

1. Build the official extension from the spine-runtimes checkout. This follows the official
   `spine-godot/build/setup-extension.sh` and `build-extension.sh`; only the editor and release
   targets are needed. The script is a shell script, so on Windows run it in Git Bash.

   ```sh
   cd <spine-runtimes>/spine-godot/build
   ./setup-extension.sh 4.5-stable false
   cd ..
   ```

   then, in `spine-godot`, for your system:

   ```sh
   # macOS (Apple silicon)
   scons -j 8 platform=macos arch=arm64 target=editor
   scons -j 8 platform=macos arch=arm64 target=template_release lto=full optimize=speed
   # Linux (x86_64)
   scons -j 8 platform=linux arch=x86_64 target=editor
   scons -j 8 platform=linux arch=x86_64 target=template_release lto=full optimize=speed
   # Windows
   scons -j 8 platform=windows target=editor
   scons -j 8 platform=windows target=template_release lto=full optimize=speed
   ```

   The result is `spine-godot/example-v4-extension/bin`. Use the same `-j` as your core count.

2. Fill the projects (this also builds the addon with `crates/spine2d-godot/package.sh` if it is
   missing):

   ```sh
   godot-bench/setup.sh <spine-runtimes> <spine-runtimes>/spine-godot/example-v4-extension/bin
   ```

3. Export and run:

   ```sh
   python3 godot-bench/run.py              # --runs 3 --reps 1 --frames 600 by default
   python3 godot-bench/run.py --no-export  # reuse the games from the last export
   python3 godot-bench/run.py --godot /path/to/godot
   ```

   `run.py` picks the export preset of your system (macOS, Linux or Windows Desktop). The exported
   games are `godot-bench/build/<official|ours>/bench.app` on macOS (signed ad hoc), `.../bench` on
   Linux and `.../bench.exe` on Windows.

   On a Linux machine with an ARM CPU, change `binary_format/architecture` to `arm64` in
   `shared/export_presets.cfg` and run `setup.sh` again.

Close other applications first. Frame times depend on machine load, and macOS may still pace
presentation even with vsync disabled, which is why the empty scene is part of the table.

## Checking that both plugins draw the same picture

A speedup only means something if both builds draw the same thing, so check the pictures first.
`bench.gd` takes two optional arguments after the seven scene arguments: a path prefix for two
screenshots taken 20 frames apart, and the number of warm-up frames. Run both exported apps with
`--fixed-fps 60` so they are at the same animation time, for example:

```sh
godot-bench/build/ours/bench.app/Contents/MacOS/bench-ours --fixed-fps 60 -- \
    tank tank-pro.skel tank.atlas drive 6 5 0.12 /tmp/ours-tank 40
```

(on Linux the program is `godot-bench/build/ours/bench`, on Windows `godot-bench\\build\\ours\\bench.exe`).

and compare the PNGs. With the screenshot argument the script also prints the world position of a
few bones, which shows whether the poses agree. Do not time anything with `--fixed-fps`: it paces
every frame to about 10 ms.

## Limits

- One machine so far: macOS arm64, Forward+ on Metal. The Linux and Windows paths are untested.
- Culling is switched off for spine2d so both do the same work: the official plugin cannot cull
  sprites that are off screen.
- GPU time is not measured. Metal timestamps are not reported by Godot there, so the table shows
  wall-clock frame time, which includes GPU and presentation.
- The official plugin does more (3D, constraint API, slot nodes, importers); this compares the
  common 2D rendering path only.
