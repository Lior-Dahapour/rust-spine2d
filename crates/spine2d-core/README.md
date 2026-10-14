# spine2d-core

The engine-agnostic core of an unofficial Rust port of the [Spine](https://esotericsoftware.com)
4.3 runtime, translated from `spine-cpp`. It has no engine dependency.

It contains skeletons, bones with the full world and local transform math, slots, skins,
attachments, every animation timeline, `AnimationState`, the atlas parser, the JSON and binary
skeleton loaders, and the IK, transform, path, physics and slider constraints.

This is not an official Spine runtime and is not affiliated with Esoteric Software. The
[Spine Runtimes License Agreement](../../LICENSE-SPINE) applies; see the
[root README](../../README.md#license) for what that means for your project.

## Use it

The crate is not published on crates.io. Depend on it from git or by path:

```toml
[dependencies]
spine2d-core = { git = "https://github.com/Lior-Dahapour/rust-spine2d" }
```

A short example that loads a skeleton, plays an animation and renders it is in the
[root README](../../README.md#usage). The world is y-down, as in Spine 4.3.

## Build and test

These commands are the same on macOS, Linux and Windows (terminal, PowerShell or `cmd`). The tools
you need to install first for each system are listed in the
[root README](../../README.md#requirements-and-build).

```sh
cargo build --release -p spine2d-core
cargo test --release -p spine2d-core
```

The tests that compare the port with the official C++ runtime are skipped unless you set up the
reference; see [Testing](../../README.md#testing) in the root README. They have only been run on
macOS arm64.

## Examples

```sh
cargo run --release -p spine2d-core --example bench -- <skeleton.skel|.json> <atlas> <animation> <frames> <loads>
```

loads a skeleton, then prints the load time, the time to build a `Skeleton`, the time per frame and
a checksum of the resulting pose. `benches/cpp/bench.cpp` is the same measurement for the C++
runtime, used to compare the two.
