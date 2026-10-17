//! `SpineSprite`: a `Node2D` that plays a Spine skeleton.
//!
//! The skeleton lives as an entity in the shared ECS world, and the wrapper objects scripts get
//! (`SpineBone`, `SpineTrackEntry`, ...) hold that entity. The world is only open while a step of a
//! frame runs, and signals are emitted with it closed, so a script connected to
//! `before_world_transforms_change` can freely move bones.

use crate::handles::skeleton::bounds_of;
use crate::handles::{
    SpineAnimationState, SpineBone, SpineEvent, SpineSkeleton, SpineSlot, SpineTrackEntry,
};
use crate::nodes::bone::SpineBoneNode;
use crate::nodes::slot::SpineSlotNode;
use crate::resources::attachment::{merge_textures, Textures};
use crate::resources::skin::SpineSkin;
use crate::resources::SpineSkeletonDataResource;
use crate::runtime::store::GodotSource;
use crate::runtime::world::{self, Frame};
use crate::systems::render::debug::DebugFlags;
use crate::systems::render::view::SpineView;
use crate::systems::render::SpineRender;
use godot::classes::notify::CanvasItemNotification;
use godot::classes::object::ConnectFlags;
use godot::classes::{INode2D, Material, Node2D, Texture2D};
use godot::prelude::*;
use godot::register::info::{PropertyHint, PropertyHintInfo, PropertyInfo};
use spine2d_core::{is_y_down, Physics, Skin};
use spine2d_ecs::bevy_ecs::entity::Entity;
use spine2d_ecs::glam::{Affine2, Vec2};
use spine2d_ecs::systems::{advance_skeleton, set_bone_world_transform};
use spine2d_ecs::{spawn_skeleton, SpineAnimation, SpineInitial, SpinePhysics, SpineProcess};
use spine2d_render::SkeletonBounds;
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

    /// The frame that moves a sprite in this mode; none for manual.
    fn frame(self) -> Option<Frame> {
        match self {
            UpdateMode::Process => Some(Frame::Process),
            UpdateMode::Physics => Some(Frame::Physics),
            UpdateMode::Manual => None,
        }
    }
}

/// Plays and draws a Spine skeleton.
///
/// [b]Warning:[/b] in a handler of any signal of this node, never call [code]free()[/code] on a
/// [SpineSprite], a [SpineBoneNode] or a [SpineSlotNode]. Call [code]queue_free()[/code], which
/// frees it after the frame. The signals of one update go to all sprites in turn, and a node freed
/// in the middle of that loop is still waiting for its signal, which then fails.
#[derive(GodotClass)]
#[class(base=Node2D, tool)]
pub struct SpineSprite {
    base: Base<Node2D>,
    /// The skeleton to play.
    #[export]
    #[var(set = set_skeleton_data)]
    skeleton_data: Option<Gd<SpineSkeletonDataResource>>,
    /// The same as `skeleton_data`, under the name the official plugin gives it, so scenes and
    /// scripts written for that plugin work. It is not stored; `skeleton_data` is.
    #[var(get = get_skeleton_data_res, set = set_skeleton_data_res, usage_flags = [NONE])]
    #[allow(dead_code)]
    skeleton_data_res: Option<Gd<SpineSkeletonDataResource>>,
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
    #[var(get = get_time_scale, set = set_time_scale)]
    time_scale: f32,
    /// When the animation advances: 0 = every frame, 1 = every physics tick, 2 = manually.
    #[export(enum = (Process = 0, Physics = 1, Manual = 2))]
    #[var(set = set_update_mode)]
    update_mode: i32,
    /// Draws the root bone.
    #[export_group(name = "Debug")]
    #[export]
    #[var(set = set_debug_root)]
    debug_root: bool,
    /// The color of the root bone.
    #[export]
    #[var(set = set_debug_root_color)]
    debug_root_color: Color,
    /// Draws every bone as a diamond.
    #[export]
    #[var(set = set_debug_bones)]
    debug_bones: bool,
    /// The color of the bones.
    #[export]
    #[var(set = set_debug_bones_color)]
    debug_bones_color: Color,
    /// The half width of the shape drawn for a bone.
    #[export]
    #[var(set = set_debug_bones_thickness)]
    debug_bones_thickness: f32,
    /// Draws the outline of every region attachment.
    #[export]
    #[var(set = set_debug_regions)]
    debug_regions: bool,
    /// The color of the region outlines.
    #[export]
    #[var(set = set_debug_regions_color)]
    debug_regions_color: Color,
    /// Draws the triangles of every drawn mesh.
    #[export]
    #[var(set = set_debug_meshes)]
    debug_meshes: bool,
    /// The color of the mesh triangles.
    #[export]
    #[var(set = set_debug_meshes_color)]
    debug_meshes_color: Color,
    /// Draws the outlines of bounding box attachments.
    #[export]
    #[var(set = set_debug_bounding_boxes)]
    debug_bounding_boxes: bool,
    /// The color of the bounding box outlines.
    #[export]
    #[var(set = set_debug_bounding_boxes_color)]
    debug_bounding_boxes_color: Color,
    /// Kept for scripts written for the official plugin, which does not draw paths either.
    #[export]
    #[var(set = set_debug_paths)]
    debug_paths: bool,
    /// The color of paths.
    #[export]
    #[var(set = set_debug_paths_color)]
    debug_paths_color: Color,
    /// Draws the polygon of every clipping attachment.
    #[export]
    #[var(set = set_debug_clipping)]
    debug_clipping: bool,
    /// The color of the clipping polygons.
    #[export]
    #[var(set = set_debug_clipping_color)]
    debug_clipping_color: Color,
    /// Skips drawing while the sprite is outside every viewport. The animation keeps running, so
    /// events and physics are unaffected; the meshes catch up the frame it comes back.
    #[export]
    #[var(set = set_cull_offscreen)]
    cull_offscreen: bool,
    /// The material for the meshes that blend normally; the built-in one if empty.
    #[export_group(name = "Materials")]
    #[export]
    #[var(set = set_normal_material)]
    normal_material: Option<Gd<Material>>,
    /// The material for the meshes that blend additively.
    #[export]
    #[var(set = set_additive_material)]
    additive_material: Option<Gd<Material>>,
    /// The material for the meshes that blend by multiplying.
    #[export]
    #[var(set = set_multiply_material)]
    multiply_material: Option<Gd<Material>>,
    /// The material for the meshes that blend as screen.
    #[export]
    #[var(set = set_screen_material)]
    screen_material: Option<Gd<Material>>,
    /// The skeleton in the world, once one is loaded.
    entity: Option<Entity>,
    /// The meshes and debug overlay of the skeleton. They are Godot objects, so they live on the
    /// node and not in the world.
    /// Textures of every custom skin shown and attachment set from script since the slots were last
    /// reset. Switching skins can leave slots showing attachments of the old skin, so these stay
    /// alive until `set_slots_to_setup_pose` clears them. They are Godot objects, so they live on
    /// the node and not in the world.
    kept_textures: Textures,
    /// Textures of the custom skin that is active now.
    active_skin_textures: Textures,
}

#[godot_api]
impl INode2D for SpineSprite {
    fn init(base: Base<Node2D>) -> Self {
        let defaults = DebugFlags::default();
        SpineSprite {
            base,
            skeleton_data: None,
            skeleton_data_res: None,
            initial_animation: GString::new(),
            skin: GString::new(),
            looping: true,
            time_scale: 1.0,
            update_mode: UpdateMode::Process as i32,
            debug_root: defaults.root,
            debug_root_color: defaults.root_color,
            debug_bones: defaults.bones,
            debug_bones_color: defaults.bones_color,
            debug_bones_thickness: defaults.bones_thickness,
            debug_regions: defaults.regions,
            debug_regions_color: defaults.regions_color,
            debug_meshes: defaults.meshes,
            debug_meshes_color: defaults.meshes_color,
            debug_bounding_boxes: defaults.bounding_boxes,
            debug_bounding_boxes_color: defaults.bounding_boxes_color,
            debug_paths: defaults.paths,
            debug_paths_color: defaults.paths_color,
            debug_clipping: defaults.clipping,
            debug_clipping_color: defaults.clipping_color,
            cull_offscreen: true,
            normal_material: None,
            additive_material: None,
            multiply_material: None,
            screen_material: None,
            entity: None,
            kept_textures: Textures::new(),
            active_skin_textures: Textures::new(),
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
        self.rebuild();
    }

    fn enter_tree(&mut self) {
        self.activate();
        self.show_in_world(self.base().is_visible_in_tree());
    }

    fn exit_tree(&mut self) {
        self.deactivate();
        self.show_in_world(false);
    }

    fn on_notification(&mut self, what: CanvasItemNotification) {
        if matches!(what, CanvasItemNotification::VISIBILITY_CHANGED) {
            self.show_in_world(self.base().is_visible_in_tree());
        }
    }
}

#[godot_api]
impl SpineSprite {
    /// Emitted when an animation starts applying.
    #[signal]
    fn animation_started(
        spine_sprite: Gd<SpineSprite>,
        animation_state: Gd<SpineAnimationState>,
        track_entry: Gd<SpineTrackEntry>,
    );
    /// Emitted when another animation replaces this one on its track.
    #[signal]
    fn animation_interrupted(
        spine_sprite: Gd<SpineSprite>,
        animation_state: Gd<SpineAnimationState>,
        track_entry: Gd<SpineTrackEntry>,
    );
    /// Emitted when an animation will never be applied again.
    #[signal]
    fn animation_ended(
        spine_sprite: Gd<SpineSprite>,
        animation_state: Gd<SpineAnimationState>,
        track_entry: Gd<SpineTrackEntry>,
    );
    /// Emitted just before a track entry is disposed.
    #[signal]
    fn animation_disposed(
        spine_sprite: Gd<SpineSprite>,
        animation_state: Gd<SpineAnimationState>,
        track_entry: Gd<SpineTrackEntry>,
    );
    /// Emitted when an animation finishes a loop or reaches its end.
    #[signal]
    fn animation_completed(
        spine_sprite: Gd<SpineSprite>,
        animation_state: Gd<SpineAnimationState>,
        track_entry: Gd<SpineTrackEntry>,
    );
    /// Emitted for user-defined animation events.
    #[signal]
    fn animation_event(
        spine_sprite: Gd<SpineSprite>,
        animation_state: Gd<SpineAnimationState>,
        track_entry: Gd<SpineTrackEntry>,
        event: Gd<SpineEvent>,
    );
    /// Emitted before the animation state advances.
    #[signal]
    fn before_animation_state_update(spine_sprite: Gd<SpineSprite>);
    /// Emitted after the animation state advanced, before it is applied to the skeleton.
    #[signal]
    fn before_animation_state_apply(spine_sprite: Gd<SpineSprite>);
    /// Emitted after animations were applied to the local pose and before world transforms are
    /// computed: the place to override bones.
    #[signal]
    fn before_world_transforms_change(spine_sprite: Gd<SpineSprite>);
    /// Emitted after world transforms were computed.
    #[signal]
    fn world_transforms_changed(spine_sprite: Gd<SpineSprite>);

    /// Called by the render server when the sprite enters a viewport.
    #[func]
    fn on_screen_entered(&mut self) {
        self.report_on_screen(true);
    }

    /// Called by the render server when the sprite leaves every viewport.
    #[func]
    fn on_screen_exited(&mut self) {
        self.report_on_screen(false);
    }

    /// True while the render server reports the sprite as inside a viewport.
    #[func]
    fn is_on_screen(&self) -> bool {
        self.entity
            .and_then(|entity| world::read::<SpineView, _>(entity, |view| view.on_screen))
            .unwrap_or(true)
    }

    #[func]
    fn set_cull_offscreen(&mut self, value: bool) {
        self.cull_offscreen = value;
        if let Some(entity) = self.entity {
            world::write::<SpineView, _>(entity, |view| view.cull = value);
        }
    }

    /// Godot resets the draw index of every child when the child list changes, so the slot nodes
    /// publish theirs again.
    #[func]
    fn on_child_order_changed(&mut self) {
        for child in self.base().get_children().iter_shared() {
            if let Ok(mut node) = child.try_cast::<SpineSlotNode>() {
                node.bind_mut().refresh_order();
            }
        }
    }

    #[func]
    fn set_skeleton_data(&mut self, data: Option<Gd<SpineSkeletonDataResource>>) {
        let callable = Callable::from_object_method(&self.to_gd(), "on_skeleton_data_changed");
        if let Some(old) = self.skeleton_data.as_mut() {
            if old.is_connected("skeleton_data_changed", &callable) {
                old.disconnect("skeleton_data_changed", &callable);
            }
        }
        self.skeleton_data = data;
        if let Some(new) = self.skeleton_data.as_mut() {
            // Deferred, because the resource is being changed when it announces the change.
            new.connect_flags("skeleton_data_changed", &callable, ConnectFlags::DEFERRED);
        }
        if self.base().is_inside_tree() {
            self.rebuild();
        }
        self.base_mut().notify_property_list_changed();
    }

    #[func]
    fn set_initial_animation(&mut self, name: GString) {
        self.initial_animation = name;
        if self.entity.is_some() {
            self.start_initial_animation();
        }
    }

    #[func]
    fn set_skin(&mut self, name: GString) {
        self.skin = name;
        self.apply_skin();
    }

    #[func]
    fn set_debug_root(&mut self, value: bool) {
        self.debug_root = value;
        self.apply_debug_flags();
    }

    #[func]
    fn set_debug_root_color(&mut self, value: Color) {
        self.debug_root_color = value;
        self.apply_debug_flags();
    }

    #[func]
    fn set_debug_bones(&mut self, value: bool) {
        self.debug_bones = value;
        self.apply_debug_flags();
    }

    #[func]
    fn set_debug_bones_color(&mut self, value: Color) {
        self.debug_bones_color = value;
        self.apply_debug_flags();
    }

    #[func]
    fn set_debug_bones_thickness(&mut self, value: f32) {
        self.debug_bones_thickness = value;
        self.apply_debug_flags();
    }

    #[func]
    fn set_debug_regions(&mut self, value: bool) {
        self.debug_regions = value;
        self.apply_debug_flags();
    }

    #[func]
    fn set_debug_regions_color(&mut self, value: Color) {
        self.debug_regions_color = value;
        self.apply_debug_flags();
    }

    #[func]
    fn set_debug_meshes(&mut self, value: bool) {
        self.debug_meshes = value;
        self.apply_debug_flags();
    }

    #[func]
    fn set_debug_meshes_color(&mut self, value: Color) {
        self.debug_meshes_color = value;
        self.apply_debug_flags();
    }

    #[func]
    fn set_debug_bounding_boxes(&mut self, value: bool) {
        self.debug_bounding_boxes = value;
        self.apply_debug_flags();
    }

    #[func]
    fn set_debug_bounding_boxes_color(&mut self, value: Color) {
        self.debug_bounding_boxes_color = value;
        self.apply_debug_flags();
    }

    #[func]
    fn set_debug_paths(&mut self, value: bool) {
        self.debug_paths = value;
        self.apply_debug_flags();
    }

    #[func]
    fn set_debug_paths_color(&mut self, value: Color) {
        self.debug_paths_color = value;
        self.apply_debug_flags();
    }

    #[func]
    fn set_debug_clipping(&mut self, value: bool) {
        self.debug_clipping = value;
        self.apply_debug_flags();
    }

    #[func]
    fn set_debug_clipping_color(&mut self, value: Color) {
        self.debug_clipping_color = value;
        self.apply_debug_flags();
    }

    #[func]
    fn set_update_mode(&mut self, mode: i32) {
        self.update_mode = mode;
        self.activate();
    }

    /// Advances the animation by `delta` seconds. Called automatically unless the update mode is
    /// manual.
    ///
    /// The step happens at the next frame, not inside this call, and several calls before that
    /// frame add up into one step. This differs from the official plugin, which updates at once.
    #[func]
    fn update_skeleton(&mut self, delta: f32) {
        if let Some(entity) = self.entity {
            world::step(entity, delta);
        }
    }

    /// Starts `name` on `track`, replacing whatever plays there. Returns its track entry.
    #[func]
    fn set_animation(
        &mut self,
        name: GString,
        looping: bool,
        track: i32,
    ) -> Option<Gd<SpineTrackEntry>> {
        let entity = self.entity?;
        let entry = world::state_mut(entity, |state| {
            state.set_animation_by_name(track.max(0) as usize, &name.to_string(), looping)
        })?;
        match entry {
            Ok(entry) => Some(SpineTrackEntry::create(entity, entry)),
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
        let entity = self.entity?;
        let entry = world::state_mut(entity, |state| {
            state.add_animation_by_name(track.max(0) as usize, &name.to_string(), looping, delay)
        })?;
        match entry {
            Ok(entry) => Some(SpineTrackEntry::create(entity, entry)),
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
        let entity = self.entity?;
        let entry = world::state_mut(entity, |state| {
            state.set_empty_animation(track.max(0) as usize, mix_duration)
        })?;
        Some(SpineTrackEntry::create(entity, entry))
    }

    /// Queues a mix out to the setup pose on `track`.
    #[func]
    fn add_empty_animation(
        &mut self,
        track: i32,
        mix_duration: f32,
        delay: f32,
    ) -> Option<Gd<SpineTrackEntry>> {
        let entity = self.entity?;
        let entry = world::state_mut(entity, |state| {
            state.add_empty_animation(track.max(0) as usize, mix_duration, delay)
        })?;
        Some(SpineTrackEntry::create(entity, entry))
    }

    /// Mixes every track out to the setup pose.
    #[func]
    fn set_empty_animations(&mut self, mix_duration: f32) {
        if let Some(entity) = self.entity {
            world::state_mut(entity, |state| state.set_empty_animations(mix_duration));
        }
    }

    /// Removes the animations on `track`.
    #[func]
    fn clear_track(&mut self, track: i32) {
        if let Some(entity) = self.entity {
            world::state_mut(entity, |state| state.clear_track(track.max(0) as usize));
        }
    }

    /// Removes all animations and resets the skeleton to its setup pose.
    #[func]
    fn clear_tracks(&mut self) {
        if let Some(entity) = self.entity {
            world::state_mut(entity, |state| state.clear_tracks());
        }
    }

    /// The entry currently playing on `track`, or null.
    #[func]
    fn get_current(&self, track: i32) -> Option<Gd<SpineTrackEntry>> {
        let entity = self.entity?;
        let entry = world::state(entity, |state| state.track(track.max(0) as usize))??;
        Some(SpineTrackEntry::create(entity, entry))
    }

    /// The animation state of this sprite, or null before it has one.
    #[func]
    fn get_animation_state(&self) -> Option<Gd<SpineAnimationState>> {
        Some(SpineAnimationState::create(self.entity?))
    }

    /// The skeleton of this sprite, or null before it has one.
    #[func]
    fn get_skeleton(&self) -> Option<Gd<SpineSkeleton>> {
        Some(SpineSkeleton::create(self.entity?))
    }

    /// The bone called `name`, or null.
    #[func]
    pub fn get_bone(&self, name: GString) -> Option<Gd<SpineBone>> {
        let entity = self.entity?;
        let id = world::skeleton(entity, |skeleton| skeleton.find_bone(&name.to_string()))??;
        Some(SpineBone::create(entity, id))
    }

    /// The slot called `name`, or null.
    #[func]
    fn get_slot(&self, name: GString) -> Option<Gd<SpineSlot>> {
        let entity = self.entity?;
        let id = world::skeleton(entity, |skeleton| skeleton.find_slot(&name.to_string()))??;
        Some(SpineSlot::create(entity, id))
    }

    /// Names of all bones, in skeleton order.
    #[func]
    fn get_bone_names(&self) -> PackedStringArray {
        let Some(entity) = self.entity else {
            return PackedStringArray::new();
        };
        world::skeleton(entity, |skeleton| {
            skeleton
                .data()
                .bones
                .iter()
                .map(|b| GString::from(b.name.as_str()))
                .collect()
        })
        .unwrap_or_default()
    }

    /// Switches to the combination of the skins called `names`, later names replacing attachments
    /// of earlier ones. Like `Skeleton.setSkin` in Spine this does not reset the slots; call
    /// `set_slots_to_setup_pose` for that. Returns false and changes nothing if a name is unknown.
    #[func]
    fn set_skins(&mut self, names: PackedStringArray) -> bool {
        let Some(entity) = self.entity else {
            return false;
        };
        let combined = world::skeleton(entity, |skeleton| {
            let data = skeleton.data();
            let mut combined = Skin::new("custom-skin");
            for name in names.to_vec() {
                let Some(skin) = data.find_skin(&name.to_string()) else {
                    godot::global::godot_warn!("Spine: unknown skin '{name}'");
                    return None;
                };
                combined.add_skin(skin);
            }
            Some(combined)
        })
        .flatten();
        let Some(combined) = combined else {
            return false;
        };
        self.install_skin(Arc::new(combined), Textures::new())
    }

    /// Switches to `skin` (see `SpineSkin`). Like `Skeleton.setSkin` in Spine this does not reset
    /// the slots; call `set_slots_to_setup_pose` for that. Returns false and changes nothing if the
    /// skin does not belong to this skeleton.
    #[func]
    pub fn set_custom_skin(&mut self, skin: Gd<SpineSkin>) -> bool {
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
        let Some(entity) = self.entity else {
            return;
        };
        world::skeleton_mut(entity, |skeleton| skeleton.setup_pose_slots());
        // Only the active skin can be shown after a reset.
        self.kept_textures = self.active_skin_textures.clone();
    }

    /// Names of all slots, in setup order.
    #[func]
    fn get_slot_names(&self) -> PackedStringArray {
        let Some(entity) = self.entity else {
            return PackedStringArray::new();
        };
        world::skeleton(entity, |skeleton| {
            skeleton
                .data()
                .slots
                .iter()
                .map(|s| GString::from(s.name.as_str()))
                .collect()
        })
        .unwrap_or_default()
    }

    /// Shows the attachment `attachment` in `slot` (an empty name hides it). Returns false if
    /// either name is unknown.
    #[func]
    fn set_attachment(&mut self, slot: GString, attachment: GString) -> bool {
        let Some(entity) = self.entity else {
            return false;
        };
        let attachment = attachment.to_string();
        let slot = slot.to_string();
        world::skeleton_mut(entity, |skeleton| {
            skeleton.set_attachment(&slot, &attachment)
        })
        .unwrap_or(false)
    }

    /// The tint applied to the whole skeleton.
    #[func]
    fn get_skeleton_color(&self) -> Color {
        self.entity
            .and_then(|entity| {
                world::skeleton(entity, |skeleton| {
                    let c = skeleton.color;
                    Color::from_rgba(c.r, c.g, c.b, c.a)
                })
            })
            .unwrap_or(Color::WHITE)
    }

    #[func]
    fn set_skeleton_color(&mut self, color: Color) {
        if let Some(entity) = self.entity {
            world::skeleton_mut(entity, |skeleton| {
                skeleton.color.set(color.r, color.g, color.b, color.a);
            });
        }
    }

    /// The bounding rectangle of everything drawn, in the sprite's local coordinates.
    #[func]
    fn get_bounds(&self) -> Rect2 {
        self.entity.map(bounds_of).unwrap_or_default()
    }

    /// The name of the bounding box attachment that contains `point` (in the sprite's local
    /// coordinates), or an empty string.
    #[func]
    fn get_bounding_box_at(&self, point: Vector2) -> GString {
        let Some(entity) = self.entity else {
            return GString::new();
        };
        world::skeleton(entity, |skeleton| {
            let mut bounds = SkeletonBounds::new();
            bounds.update(skeleton, true);
            bounds
                .contains_point(point.x, point.y)
                .map(|a| GString::from(a.name()))
        })
        .flatten()
        .unwrap_or_default()
    }

    /// Counts of expensive mesh operations since the sprite was created, as
    /// `{rebuilds, detaches, index_uploads, vertex_uploads, attribute_uploads}`.
    #[func]
    fn get_mesh_stats(&self) -> VarDictionary {
        let stats = self
            .entity
            .and_then(|entity| world::render(entity, |render| render.stats()))
            .unwrap_or_default();
        let mut result = VarDictionary::new();
        result.set("rebuilds", stats.rebuilds as i64);
        result.set("detaches", stats.detaches as i64);
        result.set("index_uploads", stats.index_uploads as i64);
        result.set("vertex_uploads", stats.vertex_uploads as i64);
        result.set("attribute_uploads", stats.attribute_uploads as i64);
        result
    }

    /// The speed multiplier now, which is the one of the animation state if a script changed it
    /// there.
    #[func]
    fn get_time_scale(&self) -> f32 {
        self.entity
            .and_then(|entity| world::state(entity, |state| state.time_scale))
            .unwrap_or(self.time_scale)
    }

    /// Builds the skeleton again, because the atlas, the skeleton file or the scale of the
    /// skeleton data changed.
    #[func]
    fn on_skeleton_data_changed(&mut self) {
        if self.base().is_inside_tree() {
            self.rebuild();
        }
    }

    #[func]
    fn get_skeleton_data_res(&self) -> Option<Gd<SpineSkeletonDataResource>> {
        self.skeleton_data.clone()
    }

    #[func]
    fn set_skeleton_data_res(&mut self, data: Option<Gd<SpineSkeletonDataResource>>) {
        self.set_skeleton_data(data);
    }

    /// A new, empty skin called `name`, to fill with attachments and show with
    /// `SpineSkeleton.set_skin`.
    #[func]
    fn new_skin(&self, name: GString) -> Option<Gd<SpineSkin>> {
        SpineSkin::create(name)
    }

    /// The world transform of the bone called `bone_name` in the coordinates of the scene. Returns
    /// the sprite's own transform if there is no such bone.
    #[func]
    fn get_global_bone_transform(&self, bone_name: GString) -> Transform2D {
        let own = self.base().get_global_transform();
        let Some(entity) = self.entity else {
            return own;
        };
        let visible = self.base().is_visible_in_tree();
        let local = world::skeleton(entity, |skeleton| {
            let id = skeleton.find_bone(&bone_name.to_string())?;
            let bone = skeleton.bone(id);
            Some(if visible {
                let pose = bone.applied_pose();
                // The skeleton is y-down, as Godot is, but a Node2D should not look mirrored.
                let flip = if is_y_down() { -1.0 } else { 1.0 };
                Transform2D::from_cols(
                    Vector2::new(pose.a, pose.c),
                    Vector2::new(pose.b * flip, pose.d * flip),
                    Vector2::new(pose.world_x, pose.world_y),
                )
            } else {
                let pose = bone.pose.local;
                let rotation_x = (pose.rotation + pose.shear_x).to_radians();
                let rotation_y = (pose.rotation + 90.0 + pose.shear_y).to_radians();
                Transform2D::from_cols(
                    Vector2::new(rotation_x.cos(), rotation_x.sin()) * pose.scale_x,
                    Vector2::new(rotation_y.cos(), rotation_y.sin()) * pose.scale_y,
                    Vector2::new(pose.x, pose.y),
                )
            })
        })
        .flatten();
        match local {
            Some(local) if visible => own * local,
            Some(local) => local,
            None => own,
        }
    }

    /// Moves the bone called `bone_name` so that its world transform in the coordinates of the
    /// scene is `global_transform`. The bone keeps the new placement until the next time the
    /// animation poses it. Does nothing if there is no such bone or the sprite is not shown.
    #[func]
    pub fn set_global_bone_transform(&mut self, bone_name: GString, global_transform: Transform2D) {
        let Some(entity) = self.entity else { return };
        if !self.base().is_visible_in_tree() {
            return;
        }
        let local = self.base().get_global_transform().affine_inverse() * global_transform;
        let flip = if is_y_down() { -1.0 } else { 1.0 };
        let placement = Affine2::from_cols(
            Vec2::new(local.a.x, local.a.y),
            Vec2::new(local.b.x * flip, local.b.y * flip),
            Vec2::new(local.origin.x, local.origin.y),
        );
        world::skeleton_mut(entity, |skeleton| {
            if let Some(id) = skeleton.find_bone(&bone_name.to_string()) {
                set_bone_world_transform(skeleton, id, placement);
            }
        });
    }

    #[func]
    fn set_normal_material(&mut self, material: Option<Gd<Material>>) {
        self.normal_material = material;
        self.apply_materials();
    }

    #[func]
    fn set_additive_material(&mut self, material: Option<Gd<Material>>) {
        self.additive_material = material;
        self.apply_materials();
    }

    #[func]
    fn set_multiply_material(&mut self, material: Option<Gd<Material>>) {
        self.multiply_material = material;
        self.apply_materials();
    }

    #[func]
    fn set_screen_material(&mut self, material: Option<Gd<Material>>) {
        self.screen_material = material;
        self.apply_materials();
    }

    #[func]
    fn set_time_scale(&mut self, scale: f32) {
        self.time_scale = scale;
        if let Some(entity) = self.entity {
            world::state_mut(entity, |state| state.time_scale = scale);
        }
    }
}

impl SpineSprite {
    /// The skeleton of this sprite in the world, if one is loaded.
    pub fn entity(&self) -> Option<Entity> {
        self.entity
    }

    /// Keeps `textures` alive until the slots are reset.
    pub fn keep_textures(&mut self, textures: &[Gd<Texture2D>]) {
        merge_textures(&mut self.kept_textures, textures);
    }

    /// The textures kept alive for attachments shown from script.
    pub fn kept_textures(&self) -> Textures {
        self.kept_textures.clone()
    }

    /// The materials of the sprite by blend mode, as the render server knows them.
    fn material_rids(&self) -> [Rid; 4] {
        [
            &self.normal_material,
            &self.additive_material,
            &self.multiply_material,
            &self.screen_material,
        ]
        .map(|material| material.as_ref().map_or(Rid::Invalid, |m| m.get_rid()))
    }

    /// Gives the skeleton in the world the materials the sprite has now.
    fn apply_materials(&self) {
        if let Some(entity) = self.entity {
            let materials = self.material_rids();
            world::write::<SpineView, _>(entity, |view| view.set_materials(materials));
        }
    }

    /// Tells the world whether the sprite is shown.
    fn show_in_world(&self, visible: bool) {
        if let Some(entity) = self.entity {
            world::write::<SpineView, _>(entity, |view| view.visible = visible);
        }
    }

    /// Tells the world what the render server reported about the sprite.
    fn report_on_screen(&self, on_screen: bool) {
        if let Some(entity) = self.entity {
            world::write::<SpineView, _>(entity, |view| view.on_screen = on_screen);
        }
    }

    /// Rebuilds the skeleton in the world from the current skeleton data.
    fn rebuild(&mut self) {
        self.follow_children_in(None);
        self.release();
        let Some(mut resource) = self.skeleton_data.clone() else {
            return;
        };
        let Some(loaded) = resource.bind_mut().loaded() else {
            return;
        };
        let time_scale = self.time_scale;
        let this = self.to_gd();
        let mut view = SpineView::new(
            self.base().get_canvas_item(),
            self.base().is_visible_in_tree(),
            self.cull_offscreen,
            self.debug_flags(),
        );
        view.set_materials(self.material_rids());
        let on_screen = (
            Callable::from_object_method(&this, "on_screen_entered"),
            Callable::from_object_method(&this, "on_screen_exited"),
        );
        let spawned = world::with(|spine| {
            let entity = spawn_skeleton::<GodotSource>(
                spine.world_mut(),
                loaded.id,
                &SpineInitial::default(),
            )?;
            spine.world_mut().entity_mut(entity).insert(view);
            spine.add_sprite(entity, this);
            spine.add_render(entity, SpineRender::new(loaded, on_screen));
            if let Some(mut animation) = spine.world_mut().get_mut::<SpineAnimation>(entity) {
                animation.time_scale = time_scale;
            }
            Some(entity)
        });
        let entity = match spawned {
            Ok(Some(entity)) => entity,
            Ok(None) => {
                godot::global::godot_error!("Spine: the skeleton data is not in the store");
                return;
            }
            Err(error) => {
                godot::global::godot_error!("Spine: {error}");
                return;
            }
        };
        self.entity = Some(entity);
        self.apply_skin();
        self.start_initial_animation();
        world::pose_mut(entity, |pose, state| {
            advance_skeleton(pose, state, 0.0);
            state.apply(pose);
            pose.update_world_transform(Physics::Reset);
        });
        self.follow_children_in(Some(entity));
        self.activate();
    }

    /// Points the bone and slot nodes among the children at the skeleton `entity`, or at nothing.
    fn follow_children_in(&self, entity: Option<Entity>) {
        for child in self.base().get_children().iter_shared() {
            let child = match child.try_cast::<SpineBoneNode>() {
                Ok(mut node) => {
                    node.bind_mut().attach_to(entity);
                    continue;
                }
                Err(child) => child,
            };
            if let Ok(mut node) = child.try_cast::<SpineSlotNode>() {
                node.bind_mut().attach_to(entity);
            }
        }
    }

    fn debug_flags(&self) -> DebugFlags {
        DebugFlags {
            root: self.debug_root,
            root_color: self.debug_root_color,
            bones: self.debug_bones,
            bones_color: self.debug_bones_color,
            bones_thickness: self.debug_bones_thickness,
            regions: self.debug_regions,
            regions_color: self.debug_regions_color,
            meshes: self.debug_meshes,
            meshes_color: self.debug_meshes_color,
            bounding_boxes: self.debug_bounding_boxes,
            bounding_boxes_color: self.debug_bounding_boxes_color,
            paths: self.debug_paths,
            paths_color: self.debug_paths_color,
            clipping: self.debug_clipping,
            clipping_color: self.debug_clipping_color,
        }
    }

    /// Tells the world which debug overlays to draw.
    fn apply_debug_flags(&mut self) {
        let flags = self.debug_flags();
        if let Some(entity) = self.entity {
            world::write::<SpineView, _>(entity, |view| view.set_debug(flags));
        }
    }

    /// The skeleton data resource this sprite plays.
    pub fn skeleton_data(&self) -> Option<Gd<SpineSkeletonDataResource>> {
        self.skeleton_data.clone()
    }

    /// Removes the skeleton from the world, if there is one.
    fn release(&mut self) {
        let Some(entity) = self.entity.take() else {
            return;
        };
        // The world is gone while the extension shuts down; there is nothing to remove then.
        let _ = world::with(|spine| {
            spine.remove_sprite(entity);
            spine.remove_render(entity);
            spine.world_mut().despawn(entity);
        });
    }

    /// Lets the frame of the update mode move the skeleton, once the sprite is in the tree.
    fn activate(&mut self) {
        let Some(entity) = self.entity else { return };
        if !self.base().is_inside_tree() {
            return;
        }
        let frame = UpdateMode::from_i32(self.update_mode).frame();
        set_markers(entity, frame);
    }

    /// Stops every frame from moving the skeleton. It keeps its state while the sprite is out of
    /// the tree.
    fn deactivate(&mut self) {
        let Some(entity) = self.entity else { return };
        set_markers(entity, None);
    }

    /// Makes the sprite show `skin` if it fits the skeleton, like Spine's `setSkin`: slots that show
    /// an attachment of the old skin switch to the same-named one of the new skin, and nothing else
    /// changes. Clears the `skin` property, which no longer says what is shown.
    fn install_skin(&mut self, skin: Arc<Skin>, textures: Textures) -> bool {
        let Some(entity) = self.entity else {
            return false;
        };
        let fits = world::skeleton_mut(entity, |skeleton| {
            if !skin.fits(skeleton.data()) {
                godot::global::godot_warn!(
                    "Spine: skin '{}' is not from this skeleton",
                    skin.name()
                );
                return false;
            }
            skeleton.set_skin(Some(skin));
            true
        })
        .unwrap_or(false);
        if !fits {
            return false;
        }
        self.skin = GString::new();
        merge_textures(&mut self.kept_textures, &textures);
        self.active_skin_textures = textures;
        true
    }

    fn apply_skin(&mut self) {
        let name = self.skin.to_string();
        let Some(entity) = self.entity else { return };
        world::skeleton_mut(entity, |skeleton| {
            if name.is_empty() {
                // No active skin: lookups fall back to the default skin, as in the reference
                // runtime.
                skeleton.set_skin(None);
            } else if !skeleton.set_skin_by_name(&name) {
                godot::global::godot_warn!("Spine: unknown skin '{name}'");
            }
            skeleton.setup_pose_slots();
        });
    }

    fn start_initial_animation(&mut self) {
        let name = self.initial_animation.to_string();
        if name.is_empty() {
            return;
        }
        let looping = self.looping;
        let Some(entity) = self.entity else { return };
        let failed = world::state_mut(entity, |state| {
            state.set_animation_by_name(0, &name, looping).is_err()
        });
        if failed == Some(true) {
            godot::global::godot_warn!("Spine: unknown animation '{name}'");
        }
    }
}

/// Gives the skeleton the marker of `frame`, and none of the others.
fn set_markers(entity: Entity, frame: Option<Frame>) {
    let _ = world::with(|spine| {
        let Ok(mut entity) = spine.world_mut().get_entity_mut(entity) else {
            return;
        };
        entity.remove::<(SpineProcess, SpinePhysics)>();
        match frame {
            Some(Frame::Process) => {
                entity.insert(SpineProcess);
            }
            Some(Frame::Physics) => {
                entity.insert(SpinePhysics);
            }
            None => {}
        }
    });
}

impl Drop for SpineSprite {
    fn drop(&mut self) {
        self.release();
    }
}
