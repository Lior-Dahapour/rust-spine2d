//! `SpineSkeletonDataResource`: an atlas and a skeleton file combined, plus the mix times between
//! animations. This is what a sprite plays.

use super::atlas::SpineAtlasResource;
use super::skeleton_file::SpineSkeletonFileResource;
use super::skin::SpineSkin;
use godot::classes::Resource;
use godot::global::godot_error;
use godot::prelude::*;
use spine2d_core::{
    AnimationStateData, AtlasAttachmentLoader, BoneId, ConstraintData, SkeletonBinary,
    SkeletonData, SkeletonJson, SlotId,
};
use std::collections::HashMap;
use std::sync::Arc;

use crate::handles::constraints::{
    SpineIkConstraintData, SpinePathConstraintData, SpinePhysicsConstraintData,
    SpineTransformConstraintData,
};
use crate::handles::{SpineAnimation, SpineBoneData, SpineEventData, SpineSlotData};
use crate::resources::animation_mix::SpineAnimationMix;
use crate::runtime::store::{Owner, SkeletonDataId};
use crate::runtime::world;

/// An atlas and a skeleton file combined, plus the mix times between animations.
#[derive(GodotClass)]
#[class(base=Resource, tool)]
pub struct SpineSkeletonDataResource {
    base: Base<Resource>,
    #[export]
    #[var(set = set_atlas)]
    atlas: Option<Gd<SpineAtlasResource>>,
    /// The same as `atlas`, under the name the official plugin gives it. It is not stored.
    #[var(get = get_atlas_res, set = set_atlas_res, usage_flags = [NONE])]
    #[allow(dead_code)]
    atlas_res: Option<Gd<SpineAtlasResource>>,
    #[export]
    #[var(set = set_skeleton_file)]
    skeleton_file: Option<Gd<SpineSkeletonFileResource>>,
    /// The same as `skeleton_file`, under the name the official plugin gives it. It is not stored.
    #[var(get = get_skeleton_file_res, set = set_skeleton_file_res, usage_flags = [NONE])]
    #[allow(dead_code)]
    skeleton_file_res: Option<Gd<SpineSkeletonFileResource>>,
    /// Scale applied to the skeleton while loading.
    #[export]
    #[var(set = set_scale)]
    scale: f32,
    /// Crossfade time used between animations that have no specific mix.
    #[export]
    #[var(set = set_default_mix)]
    default_mix: f32,
    mixes: HashMap<(String, String), f32>,
    loaded: Option<Arc<LoadedSkeleton>>,
}

/// A parsed skeleton, which lives in the skeleton store, and the textures its atlas pages use.
pub struct LoadedSkeleton {
    /// Where the data is in the store. It is removed when the last share of this is dropped.
    pub id: SkeletonDataId,
    /// For each atlas page texture handle, whether the page is premultiplied alpha.
    pub premultiplied: HashMap<usize, bool>,
}

impl Drop for LoadedSkeleton {
    fn drop(&mut self) {
        world::remove_data(self.id);
    }
}

#[godot_api]
impl IResource for SpineSkeletonDataResource {
    fn init(base: Base<Resource>) -> Self {
        SpineSkeletonDataResource {
            base,
            atlas: None,
            atlas_res: None,
            skeleton_file: None,
            skeleton_file_res: None,
            scale: 1.0,
            default_mix: 0.0,
            mixes: HashMap::new(),
            loaded: None,
        }
    }
}

#[godot_api]
impl SpineSkeletonDataResource {
    #[func]
    fn set_atlas(&mut self, atlas: Option<Gd<SpineAtlasResource>>) {
        self.atlas = atlas;
        self.invalidate();
    }

    #[func]
    fn set_skeleton_file(&mut self, file: Option<Gd<SpineSkeletonFileResource>>) {
        self.skeleton_file = file;
        self.invalidate();
    }

    #[func]
    fn get_atlas_res(&self) -> Option<Gd<SpineAtlasResource>> {
        self.atlas.clone()
    }

    #[func]
    fn set_atlas_res(&mut self, atlas: Option<Gd<SpineAtlasResource>>) {
        self.set_atlas(atlas);
    }

    #[func]
    fn get_skeleton_file_res(&self) -> Option<Gd<SpineSkeletonFileResource>> {
        self.skeleton_file.clone()
    }

    #[func]
    fn set_skeleton_file_res(&mut self, file: Option<Gd<SpineSkeletonFileResource>>) {
        self.set_skeleton_file(file);
    }

    /// Emitted when the atlas, the skeleton file or the scale changed, so what was loaded from
    /// them is dropped and loads again.
    #[signal]
    fn skeleton_data_changed();

    #[func]
    fn set_scale(&mut self, scale: f32) {
        self.scale = scale;
        self.invalidate();
    }

    /// Sets the crossfade time when switching from animation `from` to animation `to`.
    #[func]
    fn set_mix(&mut self, from: GString, to: GString, duration: f32) {
        self.mixes
            .insert((from.to_string(), to.to_string()), duration);
        if let Some(loaded) = &self.loaded {
            world::data_mut(loaded.id, |stored| {
                stored.mixes.set_mix_with(
                    &stored.data,
                    &from.to_string(),
                    &to.to_string(),
                    duration,
                )
            });
        }
    }

    /// Sets the crossfade time used between animations that have no specific mix.
    #[func]
    fn set_default_mix(&mut self, default_mix: f32) {
        self.default_mix = default_mix;
        if let Some(loaded) = &self.loaded {
            world::data_mut(loaded.id, |stored| stored.mixes.default_mix = default_mix);
        }
    }

    /// The crossfade times set between specific pairs of animations, as `SpineAnimationMix`
    /// resources. Changing the returned resources changes nothing; pass them to
    /// `set_animation_mixes`.
    #[func]
    fn get_animation_mixes(&self) -> VarArray {
        let mut mixes: Vec<_> = self.mixes.iter().collect();
        mixes.sort_by(|a, b| a.0.cmp(b.0));
        mixes
            .into_iter()
            .map(|((from, to), mix)| SpineAnimationMix::create(from, to, *mix).to_variant())
            .collect()
    }

    /// Replaces the crossfade times of specific pairs of animations with `animation_mixes`, an
    /// array of `SpineAnimationMix`. Pairs naming an animation the skeleton does not have are
    /// kept but have no effect until it loads.
    #[func]
    fn set_animation_mixes(&mut self, animation_mixes: VarArray) {
        self.mixes = animation_mixes
            .iter_shared()
            .filter_map(|mix| mix.try_to::<Gd<SpineAnimationMix>>().ok())
            .map(|mix| {
                let (from, to, mix) = mix.bind().values();
                ((from, to), mix)
            })
            .collect();
        if let Some(loaded) = self.loaded.clone() {
            world::edit_mixes(loaded.id, |data| self.mix_table(data));
        }
    }

    /// The animation called `animation_name`, or null.
    #[func]
    fn find_animation(&mut self, animation_name: GString) -> Option<Gd<SpineAnimation>> {
        let owner = self.owner()?;
        let name = animation_name.to_string();
        let index = world::owner_data(owner, |data| {
            data.animations.iter().position(|a| a.name() == name)
        })??;
        Some(SpineAnimation::create(owner, Some(index)))
    }

    /// The bone called `bone_name`, or null.
    #[func]
    fn find_bone(&mut self, bone_name: GString) -> Option<Gd<SpineBoneData>> {
        let owner = self.owner()?;
        let index = world::owner_data(owner, |data| {
            data.find_bone(&bone_name.to_string())
                .map(|bone| bone.index)
        })??;
        Some(SpineBoneData::create(owner, BoneId(index)))
    }

    /// The slot called `slot_name`, or null.
    #[func]
    fn find_slot(&mut self, slot_name: GString) -> Option<Gd<SpineSlotData>> {
        let owner = self.owner()?;
        let index = world::owner_data(owner, |data| {
            data.find_slot(&slot_name.to_string())
                .map(|slot| slot.index)
        })??;
        Some(SpineSlotData::create(owner, SlotId(index)))
    }

    /// The event called `event_data_name`, or null.
    #[func]
    fn find_event(&mut self, event_data_name: GString) -> Option<Gd<SpineEventData>> {
        let owner = self.owner()?;
        let name = event_data_name.to_string();
        let index = world::owner_data(owner, |data| {
            data.events.iter().position(|e| e.name == name)
        })??;
        Some(SpineEventData::create(owner, index))
    }

    /// The data of the IK constraint called `constraint_name`, or null.
    #[func]
    fn find_ik_constraint_data(
        &mut self,
        constraint_name: GString,
    ) -> Option<Gd<SpineIkConstraintData>> {
        let (owner, index) = self.find_constraint_data(&constraint_name, is_ik)?;
        Some(SpineIkConstraintData::create(owner, index))
    }

    /// The data of the transform constraint called `constraint_name`, or null.
    #[func]
    fn find_transform_constraint_data(
        &mut self,
        constraint_name: GString,
    ) -> Option<Gd<SpineTransformConstraintData>> {
        let (owner, index) = self.find_constraint_data(&constraint_name, is_transform)?;
        Some(SpineTransformConstraintData::create(owner, index))
    }

    /// The data of the path constraint called `constraint_name`, or null.
    #[func]
    fn find_path_constraint_data(
        &mut self,
        constraint_name: GString,
    ) -> Option<Gd<SpinePathConstraintData>> {
        let (owner, index) = self.find_constraint_data(&constraint_name, is_path)?;
        Some(SpinePathConstraintData::create(owner, index))
    }

    /// The data of the physics constraint called `constraint_name`, or null.
    #[func]
    fn find_physics_constraint_data(
        &mut self,
        constraint_name: GString,
    ) -> Option<Gd<SpinePhysicsConstraintData>> {
        let (owner, index) = self.find_constraint_data(&constraint_name, is_physics)?;
        Some(SpinePhysicsConstraintData::create(owner, index))
    }

    /// The data of every animation.
    #[func]
    fn get_animations(&mut self) -> VarArray {
        let Some(owner) = self.owner() else {
            return VarArray::new();
        };
        let count = world::owner_data(owner, |data| data.animations.len()).unwrap_or_default();
        (0..count)
            .map(|index| SpineAnimation::create(owner, Some(index)).to_variant())
            .collect()
    }

    /// The data of every bone, parents before children.
    #[func]
    fn get_bones(&mut self) -> VarArray {
        let Some(owner) = self.owner() else {
            return VarArray::new();
        };
        let count = world::owner_data(owner, |data| data.bones.len()).unwrap_or_default();
        (0..count)
            .map(|index| SpineBoneData::create(owner, BoneId(index)).to_variant())
            .collect()
    }

    /// The data of every slot, in setup order.
    #[func]
    fn get_slots(&mut self) -> VarArray {
        let Some(owner) = self.owner() else {
            return VarArray::new();
        };
        let count = world::owner_data(owner, |data| data.slots.len()).unwrap_or_default();
        (0..count)
            .map(|index| SpineSlotData::create(owner, SlotId(index)).to_variant())
            .collect()
    }

    /// Every skin.
    #[func]
    fn get_skins(&mut self) -> VarArray {
        self.with_data(|data| {
            data.skins
                .iter()
                .map(|skin| SpineSkin::wrap(skin.clone()).to_variant())
                .collect()
        })
        .unwrap_or_default()
    }

    /// The data of every event.
    #[func]
    fn get_events(&mut self) -> VarArray {
        let Some(owner) = self.owner() else {
            return VarArray::new();
        };
        let count = world::owner_data(owner, |data| data.events.len()).unwrap_or_default();
        (0..count)
            .map(|index| SpineEventData::create(owner, index).to_variant())
            .collect()
    }

    /// The data of every IK constraint, in skeleton order.
    #[func]
    fn get_ik_constraints(&mut self) -> VarArray {
        self.constraint_data(is_ik, |owner, index| {
            SpineIkConstraintData::create(owner, index).to_variant()
        })
    }

    /// The data of every transform constraint, in skeleton order.
    #[func]
    fn get_transform_constraints(&mut self) -> VarArray {
        self.constraint_data(is_transform, |owner, index| {
            SpineTransformConstraintData::create(owner, index).to_variant()
        })
    }

    /// The data of every path constraint, in skeleton order.
    #[func]
    fn get_path_constraints(&mut self) -> VarArray {
        self.constraint_data(is_path, |owner, index| {
            SpinePathConstraintData::create(owner, index).to_variant()
        })
    }

    /// The data of every physics constraint, in skeleton order.
    #[func]
    fn get_physics_constraints(&mut self) -> VarArray {
        self.constraint_data(is_physics, |owner, index| {
            SpinePhysicsConstraintData::create(owner, index).to_variant()
        })
    }

    /// The skin used when no other is set, or null.
    #[func]
    fn get_default_skin(&mut self) -> Option<Gd<SpineSkin>> {
        let skin = self.with_data(|data| data.default_skin.clone())??;
        Some(SpineSkin::wrap(skin))
    }

    /// Sets the skin used when no other is set, or none for null.
    #[func]
    fn set_default_skin(&mut self, default_skin: Option<Gd<SpineSkin>>) {
        let skin = default_skin.map(|skin| skin.bind().skin().clone());
        if let Some(owner) = self.owner() {
            world::edit_data(owner, |data| data.default_skin = skin);
        }
    }

    /// The frame rate the animations were authored at.
    #[func]
    fn get_fps(&mut self) -> f32 {
        self.with_data(|data| data.fps).unwrap_or_default()
    }

    /// The hash of the skeleton file, as exported.
    #[func]
    fn get_hash(&mut self) -> GString {
        self.with_data(|data| GString::from(data.hash.as_str()))
            .unwrap_or_default()
    }

    /// The Spine editor version that exported the file.
    #[func]
    fn get_version(&mut self) -> GString {
        self.with_data(|data| GString::from(data.version.as_str()))
            .unwrap_or_default()
    }

    /// The x of the skeleton's bounding box, as exported.
    #[func]
    fn get_x(&mut self) -> f32 {
        self.with_data(|data| data.x).unwrap_or_default()
    }

    /// The y of the skeleton's bounding box, as exported.
    #[func]
    fn get_y(&mut self) -> f32 {
        self.with_data(|data| data.y).unwrap_or_default()
    }

    /// The name of the skeleton, as exported.
    #[func]
    fn get_skeleton_name(&mut self) -> GString {
        self.with_data(|data| GString::from(data.name.as_str()))
            .unwrap_or_default()
    }

    /// The folder the images were exported from, as exported.
    #[func]
    fn get_images_path(&mut self) -> GString {
        self.with_data(|data| GString::from(data.images_path.as_str()))
            .unwrap_or_default()
    }

    /// The folder the audio was exported from, as exported.
    #[func]
    fn get_audio_path(&mut self) -> GString {
        self.with_data(|data| GString::from(data.audio_path.as_str()))
            .unwrap_or_default()
    }

    /// The scale of one unit of the editor in pixels, as exported.
    #[func]
    fn get_reference_scale(&mut self) -> f32 {
        self.with_data(|data| data.reference_scale)
            .unwrap_or_default()
    }

    #[func]
    fn set_reference_scale(&mut self, reference_scale: f32) {
        if let Some(owner) = self.owner() {
            world::edit_data(owner, |data| data.reference_scale = reference_scale);
        }
    }

    /// Reloads the skeleton data from the atlas and the skeleton file. Sprites that use it build
    /// their skeleton again, and handles to the old one stop being valid.
    #[func]
    fn update_skeleton_data(&mut self) {
        self.invalidate();
    }

    /// True once the atlas and skeleton file load into a skeleton.
    #[func]
    fn is_skeleton_data_loaded(&mut self) -> bool {
        self.loaded().is_some()
    }

    /// Names of all animations.
    #[func]
    pub fn get_animation_names(&mut self) -> PackedStringArray {
        self.with_data(|data| {
            data.animations
                .iter()
                .map(|a| GString::from(a.name()))
                .collect()
        })
        .unwrap_or_default()
    }

    /// The number of the slot called `name` (what skins use to refer to slots), or -1.
    #[func]
    fn get_slot_index(&mut self, name: GString) -> i32 {
        self.with_data(|data| data.find_slot(&name.to_string()).map(|s| s.index as i32))
            .flatten()
            .unwrap_or(-1)
    }

    /// The number of the bone called `name` (what weighted meshes use to refer to bones), or -1.
    #[func]
    fn get_bone_index(&mut self, name: GString) -> i32 {
        self.with_data(|data| data.find_bone(&name.to_string()).map(|b| b.index as i32))
            .flatten()
            .unwrap_or(-1)
    }

    /// The number of the constraint called `name` (what skins use to refer to constraints), or -1.
    #[func]
    fn get_constraint_index(&mut self, name: GString) -> i32 {
        let name = name.to_string();
        self.with_data(|data| data.constraints.iter().position(|c| c.name() == name))
            .flatten()
            .map_or(-1, |index| index as i32)
    }

    /// Names of all slots, in setup order.
    #[func]
    pub fn get_slot_names(&mut self) -> PackedStringArray {
        self.with_data(|data| {
            data.slots
                .iter()
                .map(|s| GString::from(s.name.as_str()))
                .collect()
        })
        .unwrap_or_default()
    }

    /// The skin called `name`, or `null` if there is none. The skin is a copy-on-write handle:
    /// changing it does not change this skeleton data.
    #[func]
    fn find_skin(&mut self, name: GString) -> Option<Gd<SpineSkin>> {
        let skin = self.with_data(|data| data.find_skin(&name.to_string()).cloned())??;
        Some(SpineSkin::wrap(skin))
    }

    /// Names of all skins.
    #[func]
    pub fn get_skin_names(&mut self) -> PackedStringArray {
        self.with_data(|data| data.skins.iter().map(|s| GString::from(s.name())).collect())
            .unwrap_or_default()
    }

    /// Length in seconds of the animation called `name`, or 0 if there is none.
    #[func]
    fn get_animation_duration(&mut self, name: GString) -> f32 {
        self.with_data(|data| data.find_animation(&name.to_string()).map(|a| a.duration))
            .flatten()
            .unwrap_or(0.0)
    }

    /// Skeleton width as exported (in the skeleton's units).
    #[func]
    fn get_width(&mut self) -> f32 {
        self.with_data(|data| data.width).unwrap_or(0.0)
    }

    /// Skeleton height as exported (in the skeleton's units).
    #[func]
    fn get_height(&mut self) -> f32 {
        self.with_data(|data| data.height).unwrap_or(0.0)
    }
}

impl SpineSkeletonDataResource {
    /// What the handles of this resource's data belong to, loading the data on first use.
    fn owner(&mut self) -> Option<Owner> {
        Some(Owner::Data(self.loaded()?.id))
    }

    /// The owner and the index of the constraint called `name` if `keep` accepts its kind.
    fn find_constraint_data(
        &mut self,
        name: &GString,
        keep: fn(&ConstraintData) -> bool,
    ) -> Option<(Owner, usize)> {
        let owner = self.owner()?;
        let name = name.to_string();
        let index = world::owner_data(owner, |data| {
            data.find_constraint(&name)
                .filter(|&index| keep(&data.constraints[index]))
        })??;
        Some((owner, index))
    }

    /// The handles of every constraint whose kind `keep` accepts, in skeleton order.
    fn constraint_data(
        &mut self,
        keep: fn(&ConstraintData) -> bool,
        create: fn(Owner, usize) -> Variant,
    ) -> VarArray {
        let Some(owner) = self.owner() else {
            return VarArray::new();
        };
        let indices: Vec<usize> = world::owner_data(owner, |data| {
            (0..data.constraints.len())
                .filter(|&index| keep(&data.constraints[index]))
                .collect()
        })
        .unwrap_or_default();
        indices
            .into_iter()
            .map(|index| create(owner, index))
            .collect()
    }

    fn invalidate(&mut self) {
        self.loaded = None;
        self.base_mut().emit_changed();
        self.signals().skeleton_data_changed().emit();
    }

    /// The parsed skeleton, loading it on first use. Errors are printed once per load attempt.
    pub fn loaded(&mut self) -> Option<Arc<LoadedSkeleton>> {
        if self.loaded.is_none() {
            self.loaded = self.load().map(Arc::new);
        }
        self.loaded.clone()
    }

    /// Runs `f` with the skeleton data, loading it on first use. `None` if it does not load.
    fn with_data<R>(&mut self, f: impl FnOnce(&SkeletonData) -> R) -> Option<R> {
        let loaded = self.loaded()?;
        world::data(loaded.id, |stored| f(&stored.data))
    }

    fn load(&self) -> Option<LoadedSkeleton> {
        let atlas = self.atlas.as_ref()?.bind().parse()?;
        let file = self.skeleton_file.as_ref()?.bind();
        let bytes = file.bytes().to_vec();
        if bytes.is_empty() {
            return None;
        }
        let attachments = AtlasAttachmentLoader::new(&atlas);
        let loaded = if file.is_binary() {
            let mut loader = SkeletonBinary::new(&attachments);
            loader.set_scale(self.scale);
            loader.read_skeleton_data(&bytes)
        } else {
            let mut loader = SkeletonJson::new(&attachments);
            loader.set_scale(self.scale);
            loader.read_skeleton_data(&String::from_utf8_lossy(&bytes))
        };
        match loaded {
            Ok(data) => {
                let premultiplied = atlas
                    .pages()
                    .iter()
                    .map(|page| (page.texture, page.pma))
                    .collect();
                let mixes = self.mix_table(&data);
                let id = world::register_data(data, mixes)?;
                Some(LoadedSkeleton { id, premultiplied })
            }
            Err(error) => {
                godot_error!("Spine skeleton: {error}");
                None
            }
        }
    }

    /// The mix settings of this resource for `data`, to be stored with it.
    fn mix_table(&self, data: &SkeletonData) -> AnimationStateData {
        let mut mixes = AnimationStateData::detached();
        mixes.default_mix = self.default_mix;
        for ((from, to), duration) in &self.mixes {
            mixes.set_mix_with(data, from, to, *duration);
        }
        mixes
    }
}

fn is_ik(data: &ConstraintData) -> bool {
    matches!(data, ConstraintData::Ik(_))
}

fn is_transform(data: &ConstraintData) -> bool {
    matches!(data, ConstraintData::Transform(_))
}

fn is_path(data: &ConstraintData) -> bool {
    matches!(data, ConstraintData::Path(_))
}

fn is_physics(data: &ConstraintData) -> bool {
    matches!(data, ConstraintData::Physics(_))
}
