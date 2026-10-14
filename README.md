# rust-spine2d

An unofficial Rust port of the [Spine](https://esotericsoftware.com) 4.3 runtime, with a render
library and a Godot 4 extension.

This is not an official Spine runtime and is not affiliated with Esoteric Software. "Spine" and
Esoteric Software are registered trademarks of Esoteric Software LLC.

## License

The code is derived from the Spine Runtimes and is distributed under the
[Spine Runtimes License Agreement](http://esotericsoftware.com/spine-runtimes-license), included
unmodified as [`LICENSE-SPINE`](LICENSE-SPINE). In short: integrating this runtime into your
product requires a Spine license, or the users of your product must have a Spine license, and
copies of this code must include the license text and its copyright notice. The notice is also at
the top of every ported file. This summary does not replace the license; read it.

The port is a translation of the official `spine-cpp`, made with the help of an AI assistant
(Claude), and is tested against that runtime, see [Testing](#testing). It is not a clean-room
rewrite.

No Spine editor files, example skeletons or artwork are included. The tests read the official
examples from your own checkout of
[spine-runtimes](https://github.com/EsotericSoftware/spine-runtimes).

## Crates

| Crate | Contents |
|---|---|
| [`spine2d-core`](crates/spine2d-core) | The runtime: skeletons, skins, attachments, all timelines, `AnimationState`, atlas parser, JSON and binary loaders, and the IK, transform, path, physics and slider constraints. No engine dependency. |
| [`spine2d-render`](crates/spine2d-render) | Turns a posed skeleton into batched draw data, with clipping, blend modes, two-color tint and bounding boxes. |
| [`spine2d-godot`](crates/spine2d-godot) | Godot 4.5+ extension with a `SpineSprite` node. See its [README](crates/spine2d-godot/README.md). |

A Bevy integration is planned but not started.

## Platforms

| Platform | `spine2d-core` and `spine2d-render` | Godot addon |
|---|---|---|
| macOS (arm64 and x86_64) | Built and tested on arm64 | Universal library, built by the GitHub workflow and locally. Tested on arm64 with Godot 4.7.1; the x86_64 half is built but not loaded on an Intel Mac |
| Linux (x86_64 and arm64) | Plain Rust, no platform-specific code; not tested yet | x86_64 is built by the GitHub workflow, arm64 by `package.sh` on an ARM machine; neither has been tested yet |
| Windows (x86_64) | Plain Rust, no platform-specific code; not tested yet | Built by the GitHub workflow; not tested yet |

Everything so far was developed and tested on macOS arm64. The Linux and Windows instructions below
are what should work on those systems; if something does not, please open an issue.

## Requirements and build

You need a recent stable Rust toolchain ([rustup](https://rustup.rs)) and Git on every system.

| System | Also needed |
|---|---|
| macOS | Xcode Command Line Tools: `xcode-select --install` |
| Linux | A C compiler and linker, for example `build-essential` on Debian and Ubuntu or `gcc` on Fedora |
| Windows | The MSVC toolchain: the Visual Studio Build Tools with "Desktop development with C++", and rustup's default `x86_64-pc-windows-msvc` target. Git for Windows (it includes Git Bash) if you want to run the `.sh` scripts |

The same commands work in a macOS or Linux terminal, in PowerShell and in `cmd`:

```sh
git clone https://github.com/Lior-Dahapour/rust-spine2d.git
cd rust-spine2d
cargo build --release --workspace
cargo test --release --workspace
```

`cargo test` needs nothing else: the tests that compare against the official C++ runtime skip
themselves when their environment variables are not set (see [Testing](#testing)). To build the
Godot addon, see the [Godot README](crates/spine2d-godot/README.md#install).

The crates are not published on crates.io. To use them in your own project, depend on them from git
or by path, for example
`spine2d-core = { git = "https://github.com/Lior-Dahapour/rust-spine2d" }`. The Godot extension is
built as an addon and is not used as a Rust dependency.

## Usage

```rust
use spine2d_core::{
    AnimationState, AnimationStateData, Atlas, AtlasAttachmentLoader, Physics, Skeleton, SkeletonJson,
};
use spine2d_render::SkeletonRenderer;
use std::sync::Arc;

// `PageHandles` is your `TextureLoader`: it loads each atlas page image and keeps a handle.
let atlas = Atlas::from_file("hero.atlas", Some(&mut PageHandles))?;
let attachments = AtlasAttachmentLoader::new(&atlas);
let data = Arc::new(SkeletonJson::new(&attachments).read_skeleton_data_file("hero.json")?);

let mut skeleton = Skeleton::new(data.clone());
let mut state = AnimationState::new(Arc::new(AnimationStateData::new(data)));
state.set_animation_by_name(0, "run", true)?;

let mut renderer = SkeletonRenderer::new();
loop {
    skeleton.update_time(delta);
    state.update(delta);
    state.apply(&mut skeleton);
    skeleton.update_world_transform(Physics::Update);

    for command in renderer.render(&skeleton) {
        // One draw call: positions, uvs, colors, indices, blend_mode, texture.
    }
}
```

A complete example is
[`quickstart.rs`](crates/spine2d-render/examples/quickstart.rs):

```sh
cargo run --release -p spine2d-render --example quickstart -- hero.json hero.atlas run
```

Bones, slots and track entries are identified by handles (`BoneId`, `SlotId`, `EntryId`) instead of
references. The world is y-down, as in Spine 4.3; call `spine2d_core::set_y_down(false)` for y-up.
All values are `f32`, as in the reference. Names are stored as `SmolStr`.

## Testing

```sh
cargo test --release --workspace
```

This works on every system and runs the unit and integration tests. The golden tests, which compare
this port with the official C++ runtime frame by frame, are skipped unless you provide the C++
reference. They need a clone of
[spine-runtimes](https://github.com/EsotericSoftware/spine-runtimes) (branch `4.3`) and a C++
compiler, and the helper scripts need a POSIX shell: a macOS or Linux terminal, or Git Bash or WSL
on Windows.

**macOS and Linux**

```sh
crates/spine2d-core/tests/cpp/build.sh   <spine-runtimes checkout> <out dir>
crates/spine2d-render/tests/cpp/build.sh <spine-runtimes checkout> <out dir>
SPINE_RUNTIMES_DIR=<checkout> \
SPINE_CPP_HEADLESS=<out dir>/headless SPINE_CPP_DRIVER=<out dir>/driver \
SPINE_CPP_RENDER_DRIVER=<out dir>/render_driver \
GOLDEN_ALL=1 cargo test --release --workspace
```

**Windows (Git Bash)** is the same as above. In PowerShell, run the two `build.sh` scripts from Git
Bash first, then set the variables before `cargo test`:

```powershell
$env:SPINE_RUNTIMES_DIR = "C:\path\to\spine-runtimes"
$env:SPINE_CPP_HEADLESS = "C:\path\to\out\headless"
$env:SPINE_CPP_DRIVER = "C:\path\to\out\driver"
$env:SPINE_CPP_RENDER_DRIVER = "C:\path\to\out\render_driver"
$env:GOLDEN_ALL = "1"
cargo test --release --workspace
```

Use the file names your compiler actually produced; on Windows they usually end in `.exe`. The
build scripts use `clang++` unless the `CXX` environment variable names another compiler. Without
`GOLDEN_ALL` only a few skeletons are compared.

The golden tests have only been run on macOS arm64 with Apple's clang. The comparison is bit for bit,
so on another system or compiler the results can differ in the last bit of a float because the C
math library (`sin`, `cos`, `atan2`) is different. If they fail on Linux or Windows for that reason,
the port is not necessarily wrong; please report it with the first mismatch.

What they cover:

- Every example skeleton in spine-runtimes (43 exports, JSON and binary, every animation): bone
  poses, world transforms, slot attachments, sequence indices and colors, each frame.
- Render output: draw commands, bounds with and without clipping, bounding box hit tests.
- Hand-written fixtures for constraint options and for clipping (convex, inverse, concave, restarted).

What they do not cover: `AnimationState` mix frames, event payloads and deform timelines are only
checked by Rust unit tests.

The C++ reference is built like an optimized plugin: `-O3 -flto -ffp-contract=off`. Fused
multiply-add is switched off because it changes the last bit of a float. With link-time
optimization the compiler merges the sine and cosine of the same angle into one call that rounds
differently from two separate calls, and the Rust port does the same merge in optimized builds, so
run the golden tests with `--release`; a debug build skips them.

## Performance

Measured against the official C++ runtime on an Apple M3 Pro. Both sides are release builds
(C++ with `-O3 -flto -ffp-contract=off`, Rust with fat LTO and one codegen unit, no
machine-specific flags), the runs go one after another and alternate between the two, and each
number is the best of five. The checksum of the resulting skeleton poses (and of the draw data for
the render crate) is identical on both sides for every skeleton listed. The ratio is C++ time
divided by Rust time, so above 1 means the Rust port is faster. Skeletons are the Spine example
exports, using their `-pro` files (`dragon` has only `-ess`).

Core, per frame (`Skeleton::update_world_transform` after applying one looping animation), and the
ratios for the other operations:

| Skeleton | C++ ns | Rust ns | Per frame | Binary load | JSON load | Skeleton construction |
|---|---|---|---|---|---|---|
| spineboy | 1435 | 1442 | 0.99x | 1.33x | 0.52x | 3.38x |
| raptor | 2620 | 2459 | 1.07x | 1.20x | 0.46x | 3.66x |
| tank | 3862 | 4267 | 0.91x | 1.47x | 0.79x | 2.64x |
| hero | 937 | 906 | 1.03x | 1.30x | 0.50x | 3.23x |
| alien | 642 | 673 | 0.95x | 1.07x | 0.44x | 3.08x |
| goblins | 576 | 525 | 1.10x | 1.19x | 0.55x | 3.03x |
| coin | 277 | 248 | 1.11x | 1.28x | 0.75x | 2.58x |
| mix-and-match | 2564 | 2757 | 0.93x | 0.99x | 0.45x | 3.57x |
| owl | 1576 | 1449 | 1.09x | 1.26x | 0.51x | 2.74x |
| dragon | 787 | 691 | 1.14x | 1.30x | 0.78x | 2.90x |
| celestial-circus | 2337 | 2708 | 0.86x | 1.27x | 0.54x | 3.17x |

- Per frame the port is level with C++: ahead on six skeletons, within a few percent on two, and
  7 to 14 percent behind on celestial-circus, tank and mix-and-match. Tank spends roughly 40 percent
  of its frame in one path constraint with 36 bones, where both runtimes do the same square roots,
  arc tangents and sine/cosine pairs. The other two have not been profiled.
- Loading a binary skeleton is up to 1.47x faster, and building a `Skeleton` from loaded data is
  2.6x to 3.7x faster.
- Loading JSON is 1.3x to 2.3x slower than C++ (`serde_json` builds a tree first). Loading happens
  once, and the binary format is faster than C++.

Render crate (draw data, bounds with clipping, and a bounding box hit test), the same method:

| Measurement | Rust relative to C++ |
|---|---|
| Draw data | 1.08x to 1.27x faster on all ten skeletons |
| Bounds | 1.12x to 1.33x faster |
| Bounding box hit test | 1.18x to 4.93x faster |

These are one machine and a throwaway benchmark script, so read differences of a few percent as
noise. A repeatable frame-time comparison of the Godot extension with the official plugin is in
[`godot-bench`](godot-bench).

## Credits

Thanks to [Esoteric Software](https://esotericsoftware.com) for Spine and the official runtimes,
and to the authors of [glam](https://crates.io/crates/glam),
[smallvec](https://crates.io/crates/smallvec), [smol_str](https://crates.io/crates/smol_str),
[slotmap](https://crates.io/crates/slotmap), [rustc-hash](https://crates.io/crates/rustc-hash),
[serde_json](https://crates.io/crates/serde_json), [bytemuck](https://crates.io/crates/bytemuck) and
[godot-rust](https://github.com/godot-rust/gdext).
