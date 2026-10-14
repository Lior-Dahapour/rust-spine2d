//! `SpineSprite`: a `Node2D` that plays a Spine skeleton.
//!
//! The animation state lives in a [`Player`] shared with the wrapper objects scripts get
//! (`SpineBone`, `SpineTrackEntry`, ...). Every step of a frame takes its borrow of the player for
//! as short a time as possible and signals are emitted with no borrow held, so a script connected
//! to `before_world_transforms_change` can freely move bones.

use crate::api::{SpineBone, SpineSlot, SpineTrackEntry};
use crate::attachment::{merge_textures, Textures};
use crate::batch_mesh::{BatchStats, MeshBatch};
use crate::debug::{DebugFlags, DebugLayer};
use crate::materials::{material, registered_texture_alpha, MaterialKey};
use crate::resources::{LoadedSkeleton, SpineSkeletonDataResource};
use crate::skin::SpineSkin;
use crate::slot_node::SpineSlotNode;
use godot::classes::{INode2D, Node2D, RenderingServer};
use godot::obj::Singleton;
use godot::prelude::*;
use godot::register::info::{PropertyHint, PropertyHintInfo, PropertyInfo};
use spine2d_core::animation_state::{EntryId, EventType};
use spine2d_core::{AnimationState, Event, Physics, Skeleton, Skin, SlotId};
use spine2d_render::{compute_bounds, SkeletonBounds, SkeletonRenderer, SlotMarker};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc::{channel, Receiver};
use std::sync::Arc;

/// When a [`SpineSprite`] advances its animation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum UpdateMode {
    /// Every rendered frame.
    Process = 0,
    /// Every physics tick.
    Physics = 1,
    /// Only when `update_skeleton` is called.
    Manual = 2,
}

impl UpdateMode {
    fn from_i32(value: i32) -> Self {
        match value {
            1 => UpdateMode::Physics,
            2 => UpdateMode::Manual,
            _ => UpdateMode::Process,
        }
    }
}

/// An animation event captured while the state was updating, delivered as a signal afterwards.
#[derive(Clone, Debug)]
pub struct QueuedEvent {
    pub kind: EventType,
    pub entry: EntryId,
    pub event: Option<QueuedUserEvent>,
}

/// The payload of a user-defined animation event.
#[derive(Clone, Debug)]
pub struct QueuedUserEvent {
    pub name: String,
    pub time: f32,
    pub int_value: i32,
    pub float_value: f32,
    pub string_value: String,
    pub volume: f32,
    pub balance: f32,
}

impl QueuedUserEvent {
    fn from_event(event: &Event) -> Self {
        QueuedUserEvent {
            name: event.data.name.to_string(),
            time: event.time,
            int_value: event.int_value,
            float_value: event.float_value,
            string_value: event.string_value.to_string(),
            volume: event.volume,
            balance: event.balance,
        }
    }

    fn to_dictionary(&self) -> VarDictionary {
        let mut result = VarDictionary::new();
        result.set("name", &GString::from(self.name.as_str()));
        result.set("time", self.time);
        result.set("int_value", self.int_value);
        result.set("float_value", self.float_value);
        result.set("string_value", &GString::from(self.string_value.as_str()));
        result.set("volume", self.volume);
        result.set("balance", self.balance);
        result
    }
}

/// Measures one phase for [`PhaseTimings`]. Without the `profiling` feature it does nothing and
/// reports zero, so release builds pay no clock reads.
#[derive(Clone, Copy)]
struct Stopwatch {
    #[cfg(feature = "profiling")]
    started: std::time::Instant,
}

impl Stopwatch {
    fn start() -> Self {
        Stopwatch {
            #[cfg(feature = "profiling")]
            started: std::time::Instant::now(),
        }
    }

    /// Nanoseconds since [`start`](Self::start), or 0 without the `profiling` feature.
    fn elapsed_ns(self) -> u64 {
        #[cfg(feature = "profiling")]
        {
            self.started.elapsed().as_nanos() as u64
        }
        #[cfg(not(feature = "profiling"))]
        {
            0
        }
    }
}

/// Time spent per phase, summed over frames, in nanoseconds. Only filled in with the `profiling`
/// feature.
#[derive(Clone, Copy, Debug, Default)]
pub struct PhaseTimings {
    pub pose_ns: u64,
    pub render_ns: u64,
    pub upload_ns: u64,
    pub frames: u64,
}

/// Draw index of the first mesh. It is far below the index of any ordinary child, so the meshes
/// (and the slot nodes between them) sit behind the sprite's other children.
///
/// Every mesh needs its own index: Godot orders a canvas item's children by index and gives no
/// guarantee for equal ones, so meshes that all had index 0 were drawn in a scrambled order once a
/// skeleton had enough of them.
const SPLIT_BASE_INDEX: i32 = -(1 << 24);

/// Distance between the draw indices of consecutive meshes while the sprite has slot nodes. The
/// slot nodes between two meshes take the indices in between, in draw order.
const SPLIT_INDEX_STRIDE: i32 = 256;

/// The draw index of mesh number `n`, in draw order. Meshes are `SPLIT_INDEX_STRIDE` apart while
/// slot nodes need room between them, and next to each other otherwise.
fn mesh_draw_index(n: usize, split: bool) -> i32 {
    let stride = if split { SPLIT_INDEX_STRIDE } else { 1 };
    SPLIT_BASE_INDEX + n as i32 * stride
}

/// Everything that animates and draws one skeleton. It holds no Godot objects except the render
/// server handles of its meshes, so it could be updated off the main thread.
pub struct Player {
    pub skeleton: Skeleton,
    pub state: AnimationState,
    renderer: SkeletonRenderer,
    batches: Vec<MeshBatch>,
    loaded: Arc<LoadedSkeleton>,
    events: Receiver<QueuedEvent>,
    first_update: bool,
    pub timings: PhaseTimings,
    pub debug_flags: DebugFlags,
    debug: Option<DebugLayer>,
    /// Textures of every custom skin shown and attachment set from script since the slots were last
    /// reset. Switching skins can leave slots showing attachments of the old skin, so these stay
    /// alive until `set_slots_to_setup_pose` clears them.
    pub kept_textures: Textures,
    /// Textures of the custom skin that is active now.
    pub active_skin_textures: Textures,
    /// Slots (by index) the meshes are split after, for the slot nodes; empty when there are none.
    split_slots: Vec<bool>,
    /// Where the split slots fell among the groups of the last draw, in draw order.
    markers: Vec<SlotMarker>,
    disposed: bool,
}

impl Player {
    fn new(loaded: Arc<LoadedSkeleton>, resource: &SpineSkeletonDataResource) -> Self {
        let data = loaded.data.clone();
        let state_data = Arc::new(resource.state_data(&data));
        let (sender, events) = channel();
        let mut state = AnimationState::new(state_data);
        // The state calls this while it updates; the sprite delivers the events as signals later.
        state.set_listener(move |_, kind, entry, event| {
            let queued = QueuedEvent {
                kind,
                entry,
                event: event.map(QueuedUserEvent::from_event),
            };
            // The receiver only disappears with the player, and then nobody wants the event.
            let _ = sender.send(queued);
        });
        Player {
            skeleton: Skeleton::new(data),
            state,
            renderer: SkeletonRenderer::new(),
            batches: Vec::new(),
            loaded,
            events,
            first_update: true,
            timings: PhaseTimings::default(),
            debug_flags: DebugFlags::default(),
            debug: None,
            kept_textures: Textures::new(),
            active_skin_textures: Textures::new(),
            split_slots: Vec::new(),
            markers: Vec::new(),
            disposed: false,
        }
    }

    /// Shuts the player down: frees its meshes and debug overlay and stops it drawing. Handles that
    /// scripts still hold (bones, slots, track entries) keep the skeleton alive but report
    /// themselves invalid, so a replaced or freed sprite leaves nothing on screen.
    pub fn dispose(&mut self) {
        self.batches.clear();
        self.debug = None;
        self.disposed = true;
    }

    /// False once [`dispose`](Self::dispose) was called.
    pub fn is_alive(&self) -> bool {
        !self.disposed
    }

    /// Advances the skeleton's own clock (used by physics).
    fn begin(&mut self, delta: f32) {
        let start = Stopwatch::start();
        self.skeleton.update_time(delta);
        self.timings.pose_ns += start.elapsed_ns();
    }

    fn update_state(&mut self, delta: f32) {
        let start = Stopwatch::start();
        self.state.update(delta);
        self.timings.pose_ns += start.elapsed_ns();
    }

    /// Applies the animation state to the skeleton's local pose.
    fn apply(&mut self) {
        let start = Stopwatch::start();
        self.state.apply(&mut self.skeleton);
        self.timings.pose_ns += start.elapsed_ns();
    }

    /// Computes world transforms from the local pose.
    fn update_transforms(&mut self) {
        let start = Stopwatch::start();
        let physics = if std::mem::take(&mut self.first_update) {
            Physics::Reset
        } else {
            Physics::Update
        };
        self.skeleton.update_world_transform(physics);
        self.timings.pose_ns += start.elapsed_ns();
        self.timings.frames += 1;
    }

    /// Chooses the slots the meshes are split after at the next draw; none clears the split.
    fn set_split_slots(&mut self, slots: impl Iterator<Item = SlotId>) {
        self.split_slots.clear();
        for slot in slots {
            if self.split_slots.len() <= slot.0 {
                self.split_slots.resize(slot.0 + 1, false);
            }
            self.split_slots[slot.0] = true;
        }
    }

    /// The draw index for something drawn in the gap before group `groups_before`, as the
    /// `rank`-th item of that gap (0 first). Meaningful after a draw with split slots.
    fn gap_draw_index(groups_before: usize, rank: usize) -> i32 {
        SPLIT_BASE_INDEX + (groups_before as i32 - 1) * SPLIT_INDEX_STRIDE + 1 + rank as i32
    }

    /// Takes the events queued since the last call.
    fn take_events(&self) -> Vec<QueuedEvent> {
        self.events.try_iter().collect()
    }

    /// Draws the current pose, reusing the meshes of earlier frames. Returns the bounds of what was
    /// drawn, if anything.
    fn draw(&mut self, parent_item: Rid) -> Option<Rect2> {
        if self.disposed {
            return None;
        }
        let Player {
            skeleton,
            renderer,
            batches,
            loaded,
            timings,
            debug_flags,
            debug,
            split_slots,
            markers,
            ..
        } = self;
        let start = Stopwatch::start();
        let rendered = renderer.render_batches_split(skeleton, split_slots);
        markers.clear();
        markers.extend_from_slice(rendered.markers);
        timings.render_ns += start.elapsed_ns();
        let start = Stopwatch::start();
        while batches.len() < rendered.groups.len() {
            batches.push(MeshBatch::new(parent_item));
        }
        let split = !split_slots.is_empty();
        for (n, (batch, group)) in batches.iter_mut().zip(rendered.groups).enumerate() {
            batch.set_draw_index(mesh_draw_index(n, split));
            let texture = Rid::new(group.texture as u64);
            let premultiplied = loaded
                .premultiplied
                .get(&group.texture)
                .copied()
                .or_else(|| registered_texture_alpha(group.texture))
                .unwrap_or(false);
            let wants_two_color = group.dark_color != 0xff00_0000;
            batch.upload(
                group,
                rendered.group_commands(group),
                wants_two_color,
                texture,
                |two_color| material(MaterialKey::new(group.blend_mode, premultiplied, two_color)),
            );
        }
        for batch in batches.iter_mut().skip(rendered.groups.len()) {
            batch.set_visible(false);
        }
        timings.upload_ns += start.elapsed_ns();
        if debug_flags.any() {
            debug
                .get_or_insert_with(|| DebugLayer::new(parent_item))
                .draw(*debug_flags, skeleton, rendered.commands);
        } else if let Some(layer) = debug {
            layer.clear();
        }
        batches
            .iter()
            .filter_map(MeshBatch::drawn_bounds)
            .reduce(Rect2::merge)
    }

    /// Counters summed over all meshes.
    pub fn stats(&self) -> BatchStats {
        let mut total = BatchStats::default();
        for batch in &self.batches {
            total.add(batch.stats());
        }
        total
    }
}

/// A canvas item that tells the render server to report when the sprite is on screen, so drawing
/// can be skipped while it is not. It handles cameras, canvas layers and sub-viewports.
struct Culler {
    item: Rid,
    area: Option<Rect2>,
}

/// Fraction of the bounds added on every side, so a moving skeleton rarely updates the area.
const CULL_MARGIN: f32 = 0.5;

impl Culler {
    fn new(parent_item: Rid) -> Self {
        let mut rs = RenderingServer::singleton();
        let item = rs.canvas_item_create();
        rs.canvas_item_set_parent(item, parent_item);
        Culler { item, area: None }
    }

    /// Makes sure the watched area encloses `bounds`. `callables` creates the enter and exit
    /// callbacks and only runs when the area actually changes.
    fn watch(&mut self, bounds: Rect2, callables: impl FnOnce() -> (Callable, Callable)) {
        if self.area.is_some_and(|area| area.encloses(bounds)) {
            return;
        }
        let margin = bounds.size * CULL_MARGIN;
        let area = Rect2::new(bounds.position - margin, bounds.size + margin * 2.0);
        let (enter, exit) = callables();
        RenderingServer::singleton()
            .canvas_item_set_visibility_notifier(self.item, true, area, &enter, &exit);
        self.area = Some(area);
    }
}

impl Drop for Culler {
    fn drop(&mut self) {
        RenderingServer::singleton().free_rid(self.item);
    }
}

/// A [`Player`] shared between a sprite and the wrapper objects scripts hold.
pub type SharedPlayer = Rc<RefCell<Player>>;

/// Plays and draws a Spine skeleton.
#[derive(GodotClass)]
#[class(base=Node2D, tool)]
pub struct SpineSprite {
    base: Base<Node2D>,
    /// The skeleton to play.
    #[export]
    #[var(set = set_skeleton_data)]
    skeleton_data: Option<Gd<SpineSkeletonDataResource>>,
    /// Animation to start playing on track 0 when the skeleton loads.
    #[export]
    #[var(set = set_initial_animation)]
    initial_animation: GString,
    /// Skin to use; empty keeps the default skin.
    #[export]
    #[var(set = set_skin)]
    skin: GString,
    /// Whether the initial animation loops.
    #[export]
    looping: bool,
    /// Playback speed multiplier.
    #[export(range = (0.0, 4.0, 0.01, or_greater))]
    time_scale: f32,
    /// When the animation advances: 0 = every frame, 1 = every physics tick, 2 = manually.
    #[export(enum = (Process = 0, Physics = 1, Manual = 2))]
    #[var(set = set_update_mode)]
    update_mode: i32,
    /// Draws every bone as a line with a dot at its origin.
    #[export]
    #[var(set = set_debug_bones)]
    debug_bones: bool,
    /// Draws the outlines of bounding box attachments.
    #[export]
    #[var(set = set_debug_bounding_boxes)]
    debug_bounding_boxes: bool,
    /// Draws the triangles of every drawn mesh.
    #[export]
    #[var(set = set_debug_meshes)]
    debug_meshes: bool,
    /// Skips drawing while the sprite is outside every viewport. The animation keeps running, so
    /// events and physics are unaffected; the meshes catch up the frame it comes back.
    #[export]
    cull_offscreen: bool,
    on_screen: bool,
    culler: Option<Culler>,
    player: Option<SharedPlayer>,
    /// The `SpineSlotNode` children, which register themselves when they enter the tree.
    slot_nodes: Vec<Gd<SpineSlotNode>>,
}

#[godot_api]
impl INode2D for SpineSprite {
    fn init(base: Base<Node2D>) -> Self {
        SpineSprite {
            base,
            skeleton_data: None,
            initial_animation: GString::new(),
            skin: GString::new(),
            looping: true,
            time_scale: 1.0,
            update_mode: UpdateMode::Process as i32,
            debug_bones: false,
            debug_bounding_boxes: false,
            debug_meshes: false,
            cull_offscreen: true,
            on_screen: true,
            culler: None,
            player: None,
            slot_nodes: Vec::new(),
        }
    }

    fn on_validate_property(&self, property: &mut PropertyInfo) {
        let names = match property.property_name.to_string().as_str() {
            "initial_animation" => self
                .skeleton_data
                .clone()
                .map(|mut d| d.bind_mut().get_animation_names()),
            "skin" => self
                .skeleton_data
                .clone()
                .map(|mut d| d.bind_mut().get_skin_names()),
            _ => return,
        };
        let list: Vec<String> = names
            .unwrap_or_default()
            .to_vec()
            .iter()
            .map(|n| n.to_string())
            .collect();
        property.hint_info = PropertyHintInfo {
            hint: PropertyHint::ENUM_SUGGESTION,
            hint_string: GString::from(list.join(",").as_str()),
        };
    }

    fn ready(&mut self) {
        let callable = Callable::from_object_method(&self.to_gd(), "on_child_order_changed");
        self.base_mut().connect("child_order_changed", &callable);
        self.apply_update_mode();
        self.rebuild();
    }

    #[func(gd_self)]
    fn process(this: Gd<Self>, delta: f64) {
        let mode = UpdateMode::from_i32(this.bind().update_mode);
        if mode == UpdateMode::Process {
            Self::tick(this, delta as f32);
        }
    }

    #[func(gd_self)]
    fn physics_process(this: Gd<Self>, delta: f64) {
        let mode = UpdateMode::from_i32(this.bind().update_mode);
        if mode == UpdateMode::Physics {
            Self::tick(this, delta as f32);
        }
    }
}

#[godot_api]
impl SpineSprite {
    /// Emitted when an animation starts applying.
    #[signal]
    fn animation_started(track_entry: Gd<SpineTrackEntry>);
    /// Emitted when another animation replaces this one on its track.
    #[signal]
    fn animation_interrupted(track_entry: Gd<SpineTrackEntry>);
    /// Emitted when an animation will never be applied again.
    #[signal]
    fn animation_ended(track_entry: Gd<SpineTrackEntry>);
    /// Emitted just before a track entry is disposed.
    #[signal]
    fn animation_disposed(track_entry: Gd<SpineTrackEntry>);
    /// Emitted when an animation finishes a loop or reaches its end.
    #[signal]
    fn animation_completed(track_entry: Gd<SpineTrackEntry>);
    /// Emitted for user-defined animation events. `event` has `name`, `time`, `int_value`,
    /// `float_value`, `string_value`, `volume` and `balance`.
    #[signal]
    fn animation_event(track_entry: Gd<SpineTrackEntry>, event: VarDictionary);
    /// Emitted before the animation state advances, with the (time-scaled) delta.
    #[signal]
    fn before_animation_state_update(delta: f32);
    /// Emitted after the animation state advanced, before it is applied to the skeleton.
    #[signal]
    fn animation_state_updated(delta: f32);
    /// Emitted after animations were applied to the local pose and before world transforms are
    /// computed: the place to override bones.
    #[signal]
    fn before_world_transforms_change();
    /// Emitted after world transforms were computed.
    #[signal]
    fn world_transforms_changed();

    /// Called by the render server when the sprite enters a viewport.
    #[func]
    fn on_screen_entered(&mut self) {
        self.on_screen = true;
    }

    /// Called by the render server when the sprite leaves every viewport.
    #[func]
    fn on_screen_exited(&mut self) {
        self.on_screen = false;
    }

    /// True while the render server reports the sprite as inside a viewport.
    #[func]
    fn is_on_screen(&self) -> bool {
        self.on_screen
    }

    /// Godot resets the draw index of every child when the child list changes, so the slot nodes
    /// get theirs again afterwards.
    #[func]
    fn on_child_order_changed(&mut self) {
        if !self.slot_nodes.is_empty() {
            self.base_mut().call_deferred("reapply_slot_nodes", &[]);
        }
    }

    #[func]
    fn reapply_slot_nodes(&mut self) {
        for node in &mut self.slot_nodes {
            if node.is_instance_valid() {
                node.bind_mut().forget_draw_index();
            }
        }
        self.refresh_slot_nodes();
    }

    #[func]
    fn set_skeleton_data(&mut self, data: Option<Gd<SpineSkeletonDataResource>>) {
        self.skeleton_data = data;
        if self.base().is_inside_tree() {
            self.rebuild();
        }
        self.base_mut().notify_property_list_changed();
    }

    #[func]
    fn set_initial_animation(&mut self, name: GString) {
        self.initial_animation = name;
        if self.player.is_some() {
            self.start_initial_animation();
        }
    }

    #[func]
    fn set_skin(&mut self, name: GString) {
        self.skin = name;
        self.apply_skin();
    }

    #[func]
    fn set_debug_bones(&mut self, value: bool) {
        self.debug_bones = value;
        self.apply_debug_flags();
    }

    #[func]
    fn set_debug_bounding_boxes(&mut self, value: bool) {
        self.debug_bounding_boxes = value;
        self.apply_debug_flags();
    }

    #[func]
    fn set_debug_meshes(&mut self, value: bool) {
        self.debug_meshes = value;
        self.apply_debug_flags();
    }

    #[func]
    fn set_update_mode(&mut self, mode: i32) {
        self.update_mode = mode;
        if self.base().is_inside_tree() {
            self.apply_update_mode();
        }
    }

    /// Advances the animation by `delta` seconds and redraws. Called automatically unless the
    /// update mode is manual.
    #[func(gd_self)]
    fn update_skeleton(this: Gd<Self>, delta: f32) {
        Self::tick(this, delta);
    }

    /// Starts `name` on `track`, replacing whatever plays there. Returns its track entry.
    #[func]
    fn set_animation(
        &mut self,
        name: GString,
        looping: bool,
        track: i32,
    ) -> Option<Gd<SpineTrackEntry>> {
        let player = self.player.clone()?;
        let entry = player.borrow_mut().state.set_animation_by_name(
            track.max(0) as usize,
            &name.to_string(),
            looping,
        );
        match entry {
            Ok(entry) => Some(SpineTrackEntry::create(player, entry)),
            Err(error) => {
                godot::global::godot_warn!("Spine: {error}");
                None
            }
        }
    }

    /// Queues `name` on `track` to start `delay` seconds after the current animation ends (or, if
    /// `delay` is not positive, relative to its duration). Returns its track entry.
    #[func]
    fn add_animation(
        &mut self,
        name: GString,
        delay: f32,
        looping: bool,
        track: i32,
    ) -> Option<Gd<SpineTrackEntry>> {
        let player = self.player.clone()?;
        let entry = player.borrow_mut().state.add_animation_by_name(
            track.max(0) as usize,
            &name.to_string(),
            looping,
            delay,
        );
        match entry {
            Ok(entry) => Some(SpineTrackEntry::create(player, entry)),
            Err(error) => {
                godot::global::godot_warn!("Spine: {error}");
                None
            }
        }
    }

    /// Mixes `track` out to the setup pose over `mix_duration` seconds.
    #[func]
    fn set_empty_animation(
        &mut self,
        track: i32,
        mix_duration: f32,
    ) -> Option<Gd<SpineTrackEntry>> {
        let player = self.player.clone()?;
        let entry = player
            .borrow_mut()
            .state
            .set_empty_animation(track.max(0) as usize, mix_duration);
        Some(SpineTrackEntry::create(player, entry))
    }

    /// Queues a mix out to the setup pose on `track`.
    #[func]
    fn add_empty_animation(
        &mut self,
        track: i32,
        mix_duration: f32,
        delay: f32,
    ) -> Option<Gd<SpineTrackEntry>> {
        let player = self.player.clone()?;
        let entry = player.borrow_mut().state.add_empty_animation(
            track.max(0) as usize,
            mix_duration,
            delay,
        );
        Some(SpineTrackEntry::create(player, entry))
    }

    /// Mixes every track out to the setup pose.
    #[func]
    fn set_empty_animations(&mut self, mix_duration: f32) {
        if let Some(player) = &self.player {
            player.borrow_mut().state.set_empty_animations(mix_duration);
        }
    }

    /// Removes the animations on `track`.
    #[func]
    fn clear_track(&mut self, track: i32) {
        if let Some(player) = &self.player {
            player.borrow_mut().state.clear_track(track.max(0) as usize);
        }
    }

    /// Removes all animations and resets the skeleton to its setup pose.
    #[func]
    fn clear_tracks(&mut self) {
        if let Some(player) = &self.player {
            player.borrow_mut().state.clear_tracks();
        }
    }

    /// The entry currently playing on `track`, or null.
    #[func]
    fn get_current(&self, track: i32) -> Option<Gd<SpineTrackEntry>> {
        let player = self.player.clone()?;
        let entry = player.borrow().state.track(track.max(0) as usize)?;
        Some(SpineTrackEntry::create(player, entry))
    }

    /// The bone called `name`, or null.
    #[func]
    fn get_bone(&self, name: GString) -> Option<Gd<SpineBone>> {
        let player = self.player.clone()?;
        let id = player.borrow().skeleton.find_bone(&name.to_string())?;
        Some(SpineBone::create(player, id))
    }

    /// The slot called `name`, or null.
    #[func]
    fn get_slot(&self, name: GString) -> Option<Gd<SpineSlot>> {
        let player = self.player.clone()?;
        let id = player.borrow().skeleton.find_slot(&name.to_string())?;
        Some(SpineSlot::create(player, id))
    }

    /// Names of all bones, in skeleton order.
    #[func]
    fn get_bone_names(&self) -> PackedStringArray {
        let Some(player) = &self.player else {
            return PackedStringArray::new();
        };
        let player = player.borrow();
        player
            .skeleton
            .data()
            .bones
            .iter()
            .map(|b| GString::from(b.name.as_str()))
            .collect()
    }

    /// Switches to the combination of the skins called `names`, later names replacing attachments
    /// of earlier ones. Like `Skeleton.setSkin` in Spine this does not reset the slots; call
    /// `set_slots_to_setup_pose` for that. Returns false and changes nothing if a name is unknown.
    #[func]
    fn set_skins(&mut self, names: PackedStringArray) -> bool {
        let Some(player) = self.player.clone() else {
            return false;
        };
        let combined = {
            let player = player.borrow();
            let data = player.skeleton.data();
            let mut combined = Skin::new("custom-skin");
            for name in names.to_vec() {
                let Some(skin) = data.find_skin(&name.to_string()) else {
                    godot::global::godot_warn!("Spine: unknown skin '{name}'");
                    return false;
                };
                combined.add_skin(skin);
            }
            combined
        };
        self.install_skin(Arc::new(combined), Textures::new())
    }

    /// Switches to `skin` (see `SpineSkin`). Like `Skeleton.setSkin` in Spine this does not reset
    /// the slots; call `set_slots_to_setup_pose` for that. Returns false and changes nothing if the
    /// skin does not belong to this skeleton.
    #[func]
    fn set_custom_skin(&mut self, skin: Gd<SpineSkin>) -> bool {
        let (skin, textures) = {
            let skin = skin.bind();
            (
                skin.skin().clone(),
                skin.textures().iter().cloned().collect(),
            )
        };
        self.install_skin(skin, textures)
    }

    /// Puts every slot back to its setup pose (color, attachment and draw order), dropping
    /// attachments that were set from script or left over from a previous skin.
    #[func]
    fn set_slots_to_setup_pose(&mut self) {
        let Some(player) = self.player.clone() else {
            return;
        };
        {
            let mut player = player.borrow_mut();
            player.skeleton.setup_pose_slots();
            // Only the active skin can be shown after a reset.
            player.kept_textures = player.active_skin_textures.clone();
        }
        self.show_changes();
    }

    /// Names of all slots, in setup order.
    #[func]
    fn get_slot_names(&self) -> PackedStringArray {
        let Some(player) = &self.player else {
            return PackedStringArray::new();
        };
        let player = player.borrow();
        player
            .skeleton
            .data()
            .slots
            .iter()
            .map(|s| GString::from(s.name.as_str()))
            .collect()
    }

    /// Shows the attachment `attachment` in `slot` (an empty name hides it). Returns false if
    /// either name is unknown.
    #[func]
    fn set_attachment(&mut self, slot: GString, attachment: GString) -> bool {
        let Some(player) = &self.player else {
            return false;
        };
        let attachment = attachment.to_string();
        let slot = slot.to_string();
        let mut player = player.borrow_mut();
        player.skeleton.set_attachment(&slot, &attachment)
    }

    /// The tint applied to the whole skeleton.
    #[func]
    fn get_skeleton_color(&self) -> Color {
        self.player.as_ref().map_or(Color::WHITE, |p| {
            let c = p.borrow().skeleton.color;
            Color::from_rgba(c.r, c.g, c.b, c.a)
        })
    }

    #[func]
    fn set_skeleton_color(&mut self, color: Color) {
        if let Some(player) = &self.player {
            player
                .borrow_mut()
                .skeleton
                .color
                .set(color.r, color.g, color.b, color.a);
        }
    }

    /// The bounding rectangle of everything drawn, in the sprite's local coordinates.
    #[func]
    fn get_bounds(&self) -> Rect2 {
        let Some(player) = &self.player else {
            return Rect2::default();
        };
        let bounds = compute_bounds(&player.borrow().skeleton, &mut Vec::new(), None);
        if bounds.width < 0.0 {
            return Rect2::default();
        }
        Rect2::new(
            Vector2::new(bounds.x, bounds.y),
            Vector2::new(bounds.width, bounds.height),
        )
    }

    /// The name of the bounding box attachment that contains `point` (in the sprite's local
    /// coordinates), or an empty string.
    #[func]
    fn get_bounding_box_at(&self, point: Vector2) -> GString {
        let Some(player) = &self.player else {
            return GString::new();
        };
        let mut bounds = SkeletonBounds::new();
        bounds.update(&player.borrow().skeleton, true);
        bounds
            .contains_point(point.x, point.y)
            .map(|a| GString::from(a.name()))
            .unwrap_or_default()
    }

    /// Average microseconds per frame spent posing, building render commands, and uploading, as
    /// `{pose_us, render_us, upload_us, frames}`. Resets the counters.
    #[func]
    fn take_timings(&mut self) -> VarDictionary {
        let timings = self
            .player
            .as_ref()
            .map(|p| std::mem::take(&mut p.borrow_mut().timings))
            .unwrap_or_default();
        let per_frame = |ns: u64| {
            if timings.frames == 0 {
                0.0
            } else {
                ns as f64 / timings.frames as f64 / 1000.0
            }
        };
        let mut result = VarDictionary::new();
        result.set("pose_us", per_frame(timings.pose_ns));
        result.set("render_us", per_frame(timings.render_ns));
        result.set("upload_us", per_frame(timings.upload_ns));
        result.set("frames", timings.frames as i64);
        result
    }

    /// Counts of expensive mesh operations since the sprite was created, as
    /// `{rebuilds, detaches, index_uploads, vertex_uploads, attribute_uploads}`.
    #[func]
    fn get_mesh_stats(&self) -> VarDictionary {
        let stats = self
            .player
            .as_ref()
            .map(|p| p.borrow().stats())
            .unwrap_or_default();
        let mut result = VarDictionary::new();
        result.set("rebuilds", stats.rebuilds as i64);
        result.set("detaches", stats.detaches as i64);
        result.set("index_uploads", stats.index_uploads as i64);
        result.set("vertex_uploads", stats.vertex_uploads as i64);
        result.set("attribute_uploads", stats.attribute_uploads as i64);
        result
    }
}

impl SpineSprite {
    /// One frame: update, apply, transform and draw, emitting signals between the steps.
    /// No borrow of the sprite or the player is held while a signal is emitted.
    fn tick(mut this: Gd<Self>, delta: f32) {
        let (player, time_scale, draws) = {
            let sprite = this.bind();
            (
                sprite.player.clone(),
                sprite.time_scale,
                !sprite.cull_offscreen || sprite.on_screen,
            )
        };
        let Some(player) = player else { return };
        let delta = delta * time_scale;

        player.borrow_mut().begin(delta);
        this.emit_signal("before_animation_state_update", &[delta.to_variant()]);
        player.borrow_mut().update_state(delta);
        this.emit_signal("animation_state_updated", &[delta.to_variant()]);
        let events = {
            let mut player = player.borrow_mut();
            player.apply();
            player.take_events()
        };
        Self::emit_events(&mut this, &player, events);
        this.emit_signal("before_world_transforms_change", &[]);
        player.borrow_mut().update_transforms();
        this.emit_signal("world_transforms_changed", &[]);
        if draws && this.is_visible_in_tree() {
            let item = this.get_canvas_item();
            let bounds = this.bind_mut().redraw();
            Self::watch_bounds(&mut this, item, bounds);
        }
    }

    /// Keeps the visibility notifier around what was last drawn.
    fn watch_bounds(this: &mut Gd<Self>, item: Rid, bounds: Option<Rect2>) {
        let Some(bounds) = bounds else { return };
        if !this.bind().cull_offscreen {
            return;
        }
        let existing = this.bind_mut().culler.take();
        if existing.is_none() {
            // The server only reports transitions, so a sprite that starts off screen would never
            // say so. Assume off screen until it reports entering; it was just drawn once.
            this.bind_mut().on_screen = false;
        }
        let mut culler = existing.unwrap_or_else(|| Culler::new(item));
        culler.watch(bounds, || {
            (
                Callable::from_object_method(&*this, "on_screen_entered"),
                Callable::from_object_method(&*this, "on_screen_exited"),
            )
        });
        this.bind_mut().culler = Some(culler);
    }

    fn emit_events(this: &mut Gd<Self>, player: &SharedPlayer, events: Vec<QueuedEvent>) {
        for queued in events {
            let entry = SpineTrackEntry::create(player.clone(), queued.entry);
            if let Some(event) = queued.event {
                this.emit_signal(
                    "animation_event",
                    &[entry.to_variant(), event.to_dictionary().to_variant()],
                );
                continue;
            }
            let signal = match queued.kind {
                EventType::Start => "animation_started",
                EventType::Interrupt => "animation_interrupted",
                EventType::End => "animation_ended",
                EventType::Dispose => "animation_disposed",
                EventType::Complete => "animation_completed",
                EventType::Event => continue,
            };
            this.emit_signal(signal, &[entry.to_variant()]);
        }
    }

    /// Rebuilds the player from the current skeleton data.
    fn rebuild(&mut self) {
        self.release_player();
        let Some(mut resource) = self.skeleton_data.clone() else {
            return;
        };
        let player = {
            let mut bound = resource.bind_mut();
            let Some(loaded) = bound.loaded() else { return };
            Player::new(loaded, &bound)
        };
        let mut player = player;
        player.debug_flags = self.debug_flags();
        self.player = Some(Rc::new(RefCell::new(player)));
        self.apply_skin();
        self.start_initial_animation();
        if let Some(player) = self.player.clone() {
            let mut player = player.borrow_mut();
            player.begin(0.0);
            player.apply();
            player.update_transforms();
            drop(player);
            self.redraw();
        }
    }

    fn debug_flags(&self) -> DebugFlags {
        DebugFlags {
            bones: self.debug_bones,
            bounding_boxes: self.debug_bounding_boxes,
            meshes: self.debug_meshes,
        }
    }

    /// Pushes the debug flags to the player and redraws once, so toggling works while paused.
    fn apply_debug_flags(&mut self) {
        let flags = self.debug_flags();
        let Some(player) = self.player.clone() else {
            return;
        };
        player.borrow_mut().debug_flags = flags;
        if self.base().is_inside_tree() {
            self.redraw();
        }
    }

    /// The skeleton data resource this sprite plays.
    pub fn skeleton_data(&self) -> Option<Gd<SpineSkeletonDataResource>> {
        self.skeleton_data.clone()
    }

    /// Adds a slot node to the ones this sprite draws between its meshes.
    pub fn register_slot_node(&mut self, node: Gd<SpineSlotNode>) {
        if !self.slot_nodes.contains(&node) {
            self.slot_nodes.push(node);
        }
        self.refresh_slot_nodes();
    }

    /// Removes a slot node and puts the meshes back together if it was the last one.
    pub fn unregister_slot_node(&mut self, node: &Gd<SpineSlotNode>) {
        self.slot_nodes.retain(|registered| registered != node);
        self.refresh_slot_nodes();
    }

    /// Redraws the current pose so changes to the slot nodes show at once, even while paused.
    pub fn refresh_slot_nodes(&mut self) {
        self.show_changes();
    }

    /// Redraws now, so a change made while the sprite is paused or updated manually shows at once.
    fn show_changes(&mut self) {
        if self.player.is_some() && self.base().is_inside_tree() {
            self.redraw();
        }
    }

    /// Draws the current pose. The meshes are split at the slots that have a slot node, and the
    /// nodes are given draw indices between them. Returns the bounds of what was drawn.
    fn redraw(&mut self) -> Option<Rect2> {
        let player = self.player.clone()?;
        let item = self.base().get_canvas_item();
        self.slot_nodes.retain(|node| node.is_instance_valid());
        if self.slot_nodes.is_empty() {
            player.borrow_mut().set_split_slots(std::iter::empty());
            return player.borrow_mut().draw(item);
        }
        let slots: Vec<Option<SlotId>> = self
            .slot_nodes
            .iter_mut()
            .map(|node| node.bind_mut().resolve(&player))
            .collect();
        player
            .borrow_mut()
            .set_split_slots(slots.iter().flatten().copied());
        let bounds = player.borrow_mut().draw(item);

        // Markers come in draw order, so a gap's items are numbered in the order of their slots.
        let player = player.borrow();
        let mut placed = vec![false; slots.len()];
        let (mut gap, mut rank) = (usize::MAX, 0);
        for marker in &player.markers {
            if marker.groups_before != gap {
                (gap, rank) = (marker.groups_before, 0);
            }
            for (n, node) in self.slot_nodes.iter_mut().enumerate() {
                if slots[n] == Some(marker.slot) {
                    let index = Player::gap_draw_index(gap, rank);
                    node.bind_mut().set_draw_index(Some(index));
                    placed[n] = true;
                    rank += 1;
                }
            }
        }
        for (node, placed) in self.slot_nodes.iter_mut().zip(placed) {
            if !placed {
                node.bind_mut().set_draw_index(None);
            }
        }
        bounds
    }

    /// The player this sprite animates, if a skeleton is loaded.
    pub fn player(&self) -> Option<SharedPlayer> {
        self.player.clone()
    }

    /// Disposes the current player (if any) and forgets it.
    fn release_player(&mut self) {
        if let Some(player) = self.player.take() {
            player.borrow_mut().dispose();
        }
    }

    fn apply_update_mode(&mut self) {
        let mode = UpdateMode::from_i32(self.update_mode);
        self.base_mut().set_process(mode == UpdateMode::Process);
        self.base_mut()
            .set_physics_process(mode == UpdateMode::Physics);
    }

    /// Makes `skin` the active skin if it fits the skeleton, like Spine's `setSkin`: slots that show
    /// an attachment of the old skin switch to the same-named one of the new skin, and nothing else
    /// changes. Clears the `skin` property, which no longer says what is shown.
    fn install_skin(&mut self, skin: Arc<Skin>, textures: Textures) -> bool {
        let Some(player) = self.player.clone() else {
            return false;
        };
        {
            let mut player = player.borrow_mut();
            if !skin.fits(player.skeleton.data()) {
                godot::global::godot_warn!(
                    "Spine: skin '{}' is not from this skeleton",
                    skin.name()
                );
                return false;
            }
            player.skeleton.set_skin(Some(skin));
        }
        self.skin = GString::new();
        {
            let mut player = player.borrow_mut();
            merge_textures(&mut player.kept_textures, &textures);
            player.active_skin_textures = textures;
        }
        self.show_changes();
        true
    }

    fn apply_skin(&mut self) {
        let name = self.skin.to_string();
        let Some(player) = &self.player else { return };
        let mut player = player.borrow_mut();
        if name.is_empty() {
            // No active skin: lookups fall back to the default skin, as in the reference runtime.
            player.skeleton.set_skin(None);
        } else if !player.skeleton.set_skin_by_name(&name) {
            godot::global::godot_warn!("Spine: unknown skin '{name}'");
        }
        player.skeleton.setup_pose_slots();
    }

    fn start_initial_animation(&mut self) {
        let name = self.initial_animation.to_string();
        if name.is_empty() {
            return;
        }
        let looping = self.looping;
        let Some(player) = &self.player else { return };
        if player
            .borrow_mut()
            .state
            .set_animation_by_name(0, &name, looping)
            .is_err()
        {
            godot::global::godot_warn!("Spine: unknown animation '{name}'");
        }
    }
}

impl Drop for SpineSprite {
    fn drop(&mut self) {
        self.release_player();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn batch_index(n: i32) -> i32 {
        SPLIT_BASE_INDEX + n * SPLIT_INDEX_STRIDE
    }

    #[test]
    fn every_mesh_gets_its_own_index_in_draw_order_behind_ordinary_children() {
        for split in [false, true] {
            let indices: Vec<i32> = (0..300).map(|n| mesh_draw_index(n, split)).collect();
            assert!(
                indices.windows(2).all(|pair| pair[0] < pair[1]),
                "split: {split}"
            );
            assert!(indices.iter().all(|&index| index < 0), "split: {split}");
        }
    }

    #[test]
    fn slot_nodes_sit_between_the_meshes_around_their_gap() {
        for gap in 1..5usize {
            let first = Player::gap_draw_index(gap, 0);
            let last = Player::gap_draw_index(gap, 10);
            assert!(batch_index(gap as i32 - 1) < first);
            assert!(first < last);
            assert!(last < batch_index(gap as i32));
        }
    }

    #[test]
    fn a_gap_before_the_first_mesh_is_below_every_mesh() {
        assert!(Player::gap_draw_index(0, 0) < batch_index(0));
    }
}
