# spine2d-godot

A Godot 4 extension that plays [Spine](https://esotericsoftware.com) 4.3 animations, built on
`spine2d-core` and `spine2d-render`.

- Godot 4.5 or newer, 2D only (`SpineSprite` is a `Node2D`).
- Spine 4.3 exports: JSON (`.spine-json`) or binary (`.skel`), plus the `.atlas`.
- Tested on Godot 4.7.1, macOS arm64, Compatibility and Forward+ renderers.

The Spine Runtimes license applies; see the [root README](../../README.md#license).

## Platforms

| Platform | Library in the addon | Status |
|---|---|---|
| macOS | `librust_spine2d.dylib`, universal (arm64 and x86_64) | Tested on arm64; the x86_64 half is not loaded anywhere yet |
| Linux | `librust_spine2d.so` for x86_64 and arm64 | x86_64 is built by `.github/workflows/godot-addon.yml`; arm64 you build with `package.sh` on an ARM machine. Not tested yet |
| Windows | `rust_spine2d.dll` for x86_64 | Built by `.github/workflows/godot-addon.yml`; not tested yet |

The extension file `rust_spine2d.gdextension` lists all of them. Web, Android and iOS are not
supported. Only macOS arm64 has been tried so far, with Godot 4.7.1.

## Install

You need Godot 4.5 or newer. There are two ways to get the addon.

**Option 1: download it.** The `godot-addon` workflow runs on GitHub on every push to `main` that
changes the code and builds the Linux (x86_64) and Windows (x86_64) libraries. Open the run in the
Actions tab and download the `rust-spine2d-addon` artifact: a zip with an `addons` folder at its
root. The macOS library is built only when you start the workflow by hand ("Run workflow", tick
"Also build the macOS library"), or you build it on a Mac with Option 2 and add the file to the
addon folder.

**Option 2: build it on your own machine.** The library is built for the system you run on, so
build on each system you want to ship to.

| System | Install first |
|---|---|
| macOS | Xcode Command Line Tools (`xcode-select --install`) and Rust from [rustup](https://rustup.rs). For a universal library also run `rustup target add aarch64-apple-darwin x86_64-apple-darwin`; without both targets you get the architecture of your machine |
| Linux | A C compiler and linker (`build-essential` on Debian and Ubuntu, `gcc` on Fedora) and Rust from rustup |
| Windows | Rust from rustup with the default MSVC target, the Visual Studio Build Tools with "Desktop development with C++", and Git for Windows (for Git Bash) |

Then, from the repository root:

```sh
sh crates/spine2d-godot/package.sh       # macOS and Linux terminal, or Git Bash on Windows
```

It writes `target/package/addons/rust_spine2d/`. Without a POSIX shell (plain PowerShell or `cmd`),
build and copy by hand:

```sh
cargo build -p spine2d-godot --release
```

and put the files together like this, using the names for your system:

| System | Cargo writes (`target/release/`) | Copy it to `addons/rust_spine2d/bin/` as |
|---|---|---|
| macOS | `libspine2d_godot.dylib` | `librust_spine2d.dylib` |
| Linux | `libspine2d_godot.so` | `librust_spine2d.so` |
| Windows | `spine2d_godot.dll` | `rust_spine2d.dll` |

Also copy `crates/spine2d-godot/rust_spine2d.gdextension` to `addons/rust_spine2d/`, next to `bin`.

**Use it in a Godot project:** copy the `addons` folder into your project and restart Godot.
`SpineSprite` and the other classes show up in the "Create New Node" dialog. When you export a game
for a system, the addon needs the library of that system in `bin/`, so use the merged addon from
Option 1 or build on that system.

## Quick start

Put the export (`hero.atlas`, its `.png`, and `hero.skel` or `hero.spine-json`) in the project.
`load("res://hero/hero.atlas")` returns a `SpineAtlasResource`.

In the editor: add a `SpineSprite`, create a `SpineSkeletonDataResource` in `Skeleton Data`, assign
the atlas and the skeleton file, and pick an `Initial Animation` from the dropdown.

In code:

```gdscript
var data := SpineSkeletonDataResource.new()
data.atlas = load("res://hero/hero.atlas")
data.skeleton_file = load("res://hero/hero.skel")
data.default_mix = 0.2                       # crossfade seconds
data.set_mix("run", "jump", 0.5)             # or per pair

var hero := SpineSprite.new()
hero.skeleton_data = data
add_child(hero)

hero.set_animation("run", true, 0)           # name, loop, track
hero.add_animation("idle", 0.0, true, 0)     # queued after the current one
hero.animation_event.connect(func(entry, event): print(event.name))
hero.animation_completed.connect(func(entry): print(entry.get_animation_name()))

# Move a bone after animations are applied and before world transforms are computed:
var head := hero.get_bone("head")
hero.before_world_transforms_change.connect(func(): head.set_rotation(head.get_rotation() + 20.0))
```

## Classes

| Class | Purpose |
|---|---|
| `SpineSprite` | The node. Plays and draws a skeleton. Update mode: process, physics or manual. |
| `SpineSkeletonDataResource` | Atlas, skeleton file and mix times. Shared by every sprite that uses it. |
| `SpineAtlasResource`, `SpineSkeletonFileResource` | The loaded `.atlas`, `.skel` and `.spine-json` files. The data is stored inside the saved resource, so exported games do not need the originals. |
| `SpineSkin` | A skin of the skeleton, or a new empty one. Combine skins for equipment and mix-and-match. See [Skins](#skins). |
| `SpineTrackEntry`, `SpineBone`, `SpineSlot` | Handles a script gets from the sprite. `is_valid()` turns false when the sprite reloads or is freed. |
| `SpineBoneNode` | A child of a `SpineSprite` that follows one bone. |
| `SpineSlotNode` | A child of a `SpineSprite` that follows a slot's bone and draws at the slot's place in the draw order. See [Slot nodes](#slot-nodes). |

`SpineSprite` also has `set_animation`, `add_animation`, `set_empty_animation`,
`add_empty_animation`, `set_empty_animations`, `clear_track(s)`, `get_current`, `get_bone`,
`get_slot`, `get_bone_names`, `get_slot_names`, `set_attachment`, `set_skin`, skeleton color,
`get_bounds`, `get_bounding_box_at`, `update_skeleton` (manual mode), debug drawing for bones,
bounding boxes and meshes, and the signals `animation_started`, `animation_interrupted`,
`animation_ended`, `animation_disposed`, `animation_completed`, `animation_event`,
`before_animation_state_update`, `animation_state_updated`, `before_world_transforms_change` and
`world_transforms_changed`.

## Rendering

Every run of attachments that share a texture, blend mode and tint becomes one mesh, so one draw
call. Meshes are created once and updated in place, and only the buffers that changed are uploaded.
Sprites outside every viewport skip drawing but keep animating (`cull_offscreen`). Normal, additive
and multiply blending, two-color tint, clipping and premultiplied-alpha atlases are supported.

## Skins

Spine skins can be combined at run time, which is how characters get swappable hair, clothes and
equipment. Later skins replace attachments of earlier ones for the same slot. Switching skins works
as in Spine: slots that show an attachment of the old skin switch to the same-named one of the new
skin, and nothing else changes. Call `set_slots_to_setup_pose()` to drop everything else.

```gdscript
# By name:
hero.set_skins(PackedStringArray(["skin-base", "hair/pink", "clothes/dress-blue", "legs/boots-pink"]))

# Or build a skin object, to keep or change it later:
var outfit := SpineSkin.create("outfit")
for name in ["skin-base", "nose/short", "eyes/violet"]:
    outfit.add_skin(data.find_skin(name))
outfit.add_skin(data.find_skin("hair/blue"))     # swap the hair later
hero.set_custom_skin(outfit)
hero.set_slots_to_setup_pose()     # drops attachments left over from the previous outfit
```

- `SpineSkeletonDataResource.find_skin(name)` returns a `SpineSkin` (or `null`). Changing it never
  changes the skeleton data, so other sprites are unaffected.
- `set_skins` and `set_custom_skin` return `false` and change nothing if a skin name is unknown or
  the skin is from a different skeleton.
- `copy_skin` is like `add_skin` but copies the attachments, so they can be changed independently.
- A `SpineSkin` is a snapshot: adding to a skin after `set_custom_skin` does not change a sprite
  that already shows it. Call `set_custom_skin` again.
- `SpineSkin` also has `get_attachment(slot_index, name)`, `set_attachment(slot_index, name,
  attachment)`, `remove_attachment`, `find_names_for_slot` and `get_attachments()` (a list of
  `SpineSkinEntry`). Attachments are `SpineAttachment` objects, shared between skins unless you
  call `copy()`.
- Skins and the skeleton data refer to slots and bones by number. `SpineSkeletonDataResource` has
  `get_slot_index(name)` and `get_bone_index(name)` (-1 if there is none).
- Setting the `skin` property (a single name) works as before.

## Attachments from script

Besides the attachments of the skeleton's own skins, a script can make flat image attachments from
any texture it has loaded, for a held item or a badge that does not need to bend with the body:

```gdscript
var sword := SpineAttachment.create_region("sword", load("res://items/sword.png"))
sword.set_position(Vector2(40, -10))         # in the axes of the slot's bone
sword.set_scale(Vector2(0.5, 0.5))

var item := SpineSkin.create("sword")
item.set_attachment(data.get_slot_index("gun"), "sword", sword)
hero.set_custom_skin(item)
hero.get_slot("gun").set_attachment("sword")
```

- `create_region(name, texture, rect, size, premultiplied)`: `rect` is the part of the texture to
  draw in pixels (empty means all of it), `size` how big it is in skeleton units (zero means the
  pixel size). `create_region_from_atlas(name, atlas, region_name, size)` takes the image from a
  `SpineAtlasResource` instead.
- To change one sprite's slot without a skin, use `slot.set_attachment_object(attachment)`. It
  stays until `slot.set_attachment(name)` or `sprite.set_slots_to_setup_pose()` replaces it, or an
  animation that keys the slot's attachment does. Switching skins leaves it alone.
  `slot.get_attachment_object()` returns what a slot shows. A mesh that uses a bone the skeleton
  does not have is refused.
- Meshes, which deform with the bones, come from arrays: `create_mesh(name, texture, uvs, triangles,
  vertices, hull_vertices)` and `create_mesh_from_atlas`. `uvs` has 2 values per vertex across the
  image, `triangles` 3 vertex numbers per triangle. `vertices` holds 2 values per vertex for an
  unweighted mesh, or Spine's weighted layout: per vertex the number of bones, then for each bone
  its number (`get_bone_index`), `x`, `y` and weight. Arrays that do not fit together give `null`
  and a message; a bone number that does not exist is refused when the skin is set.
- `create_linked_mesh(name, parent, texture)` (and `create_linked_mesh_from_atlas`) copies a mesh of
  the skeleton with a different image: same shape and weights, and it follows the parent's deform
  animations. Give it an image with the same layout as the parent's.
- Some bones and constraints (a skirt's control bones, say) are only active while a skin that lists
  them is. Skins taken from the skeleton already list theirs. For a skin you fill with meshes
  weighted to such bones, declare them with `skin.add_bone(bone_index)` and
  `skin.add_constraint(constraint_index)` (numbers from `get_bone_index` and
  `get_constraint_index`; `skin.get_bones()` and `skin.get_constraints()` list them). Without that
  the bones stay inactive and the mesh collapses.
- Edit a region attachment with `set_position`, `set_rotation`, `set_scale`, `set_size` and
  `set_color` before you put it into a skin; one that is already shared cannot be edited. `copy()` gives an
  independent one.
- Where the texture comes from is up to the script: a loaded resource, or an `ImageTexture` made
  from a downloaded file. The attachment, the skin that holds it and the sprite that shows it keep
  the texture alive; when none of them is left, Godot frees it. Because switching skins can leave
  old attachments showing, a sprite keeps the textures of the skins it has worn until you call
  `set_slots_to_setup_pose()`.
- A texture that is not from the skeleton's atlas is a separate mesh and draw call.

## Slot nodes

A `SpineSlotNode` draws its children between the slots of the skeleton: in front of the slots drawn
before its slot and behind the ones drawn after it. Use one for a weapon that must pass behind an
arm, or an effect that belongs between two layers. Add it as a direct child of a `SpineSprite`, pick
the `Slot Name` in the inspector, and put your nodes under it.

```gdscript
var node := SpineSlotNode.new()
node.slot_name = "torso"
node.add_child(my_sprite_or_particles)
hero.add_child(node)
```

- The node follows the bone its slot is attached to, like `SpineBoneNode`.
- The order comes from the skeleton, not from the order of the nodes in the tree, and follows draw
  order animations.
- Each slot that has a slot node splits the sprite's meshes there, which costs one more draw call
  per distinct slot. Sprites without slot nodes are unaffected.
- Slot nodes and the sprite's meshes sit behind the sprite's other children.

## Limitations

- No 3D node, no script classes for constraints (IK, transform,
  path, physics, slider), no timeline preview in the editor.
- Screen blend falls back to additive, because Godot has no screen blend.
- Premultiplied-alpha atlases work, but Godot's default texture import ("Fix Alpha Border") leaves
  a faint rim. Turn it off for those textures, or export a non-premultiplied atlas.
- No HDR-2D tint conversion, normal or specular maps, or light masks.
- Plain `.json` is not claimed by the loader because Godot owns that extension. Rename the file to
  `.spine-json`.
- Windows and Linux libraries come from `.github/workflows/godot-addon.yml`, which has not been
  run yet. The x86_64 half of the macOS universal library is built but was not loaded on the arm64
  test machine.
- Godot 4.7.1 crashes on exit when a scripted run quits right after startup
  (`godot --headless --import --quit` on a fresh project, exit code 134). This happens with any
  class registered by a gdext 0.5.5 extension built against API 4.5, even an empty `Node2D`, so it
  is not specific to Spine. Editor use and running games are not affected. To import in CI use
  `godot --headless --editor --path <project> --quit-after 120`.

## Performance

The official plugin creates one mesh per slot. This extension merges consecutive slots that share a
texture, blend mode and tint into one mesh and writes the geometry into persistent buffers in
place, uploading only the regions that changed, so a skeleton costs a few draw calls instead of
about one per slot. [`godot-bench`](../../godot-bench) compares the two plugins in release builds
of both; run it on your own hardware, as the result depends on the machine and the renderer.

## Building and testing

```sh
cargo test --release --workspace
```

works on every system. The GDScript suites run inside Godot and need a clone of
[spine-runtimes](https://github.com/EsotericSoftware/spine-runtimes) (only its `examples/` are
used) and the path of the Godot 4.5+ executable. Give the path in the `GODOT` environment variable,
or put the executable on your `PATH` as `godot`; on macOS `/Applications/Godot.app/Contents/MacOS/Godot`
is the default.

```sh
# macOS and Linux
GODOT=/path/to/godot crates/spine2d-godot/tests/godot/run.sh <spine-runtimes checkout>
GODOT=/path/to/godot crates/spine2d-godot/package.sh --verify <spine-runtimes checkout>
```

On Windows run the same two commands in Git Bash, with the path in Git Bash style, for example
`GODOT=/c/Godot/Godot_v4.5-stable_win64.exe`. `run.sh` builds the extension and runs the
suites; `package.sh --verify` packages the addon and tests it in a clean project. Some suites
(`cull_test`, `stale_test`, `slot_test`, `attachment_test`, `rebuilt_mesh_test`) open a real window,
so on a Linux machine without a display run them under `xvfb-run`. The `profiling` cargo feature
fills in `SpineSprite.take_timings()` for the benchmark scenes. These scripts have only been run on
macOS.

## Design

`SpineSprite` owns one `Player` (skeleton, animation state, renderer, meshes) that is shared with
the bone, slot and track-entry handles scripts hold. Each frame the sprite advances the state,
applies it, computes world transforms and draws, emitting its signals between the steps with no
borrow held, so scripts can call back into the sprite from any signal.
