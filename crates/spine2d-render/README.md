# spine2d-render

Turns a posed skeleton from `spine2d-core` into engine-agnostic draw data: batched draw commands
with positions, UVs, colors, indices, blend mode and texture handle, plus clipping, two-color tint,
bounds and bounding box hit tests. It is part of an unofficial Rust port of the
[Spine](https://esotericsoftware.com) 4.3 runtime and has no engine dependency.

This is not an official Spine runtime and is not affiliated with Esoteric Software. The
[Spine Runtimes License Agreement](../../LICENSE-SPINE) applies; see the
[root README](../../README.md#license) for what that means for your project.

## Use it

The crate is not published on crates.io. Depend on it from git or by path, together with the core:

```toml
[dependencies]
spine2d-core = { git = "https://github.com/Lior-Dahapour/rust-spine2d" }
spine2d-render = { git = "https://github.com/Lior-Dahapour/rust-spine2d" }
```

[`SkeletonRenderer::render`](src/renderer.rs) returns the commands to draw in order; your engine
uploads each one as a mesh. The [root README](../../README.md#usage) shows the whole loop.

## Build, test and try it

These commands are the same on macOS, Linux and Windows (terminal, PowerShell or `cmd`). The tools
to install first for each system are listed in the
[root README](../../README.md#requirements-and-build).

```sh
cargo build --release -p spine2d-render
cargo test --release -p spine2d-render
cargo run --release -p spine2d-render --example quickstart -- <skeleton.json|.skel> <atlas> <animation>
```

`quickstart` loads a skeleton from a Spine export, plays the animation and prints the draw data of
each frame. The tests that compare the output with the official C++ runtime are skipped unless you
set up the reference; see [Testing](../../README.md#testing) in the root README. They have only been
run on macOS arm64.

`examples/render_bench.rs` and `benches/cpp/bench.cpp` measure the same work in Rust and in C++.
