// Spine Runtimes License Agreement
// Last updated April 5, 2025. Replaces all prior versions.
//
// Copyright (c) 2013-2025, Esoteric Software LLC
//
// Integration of the Spine Runtimes into software or otherwise creating
// derivative works of the Spine Runtimes is permitted under the terms and
// conditions of Section 2 of the Spine Editor License Agreement:
// http://esotericsoftware.com/spine-editor-license
//
// Otherwise, it is permitted to integrate the Spine Runtimes into software
// or otherwise create derivative works of the Spine Runtimes (collectively,
// "Products"), provided that each user of the Products must obtain their own
// Spine Editor license and redistribution of the Products in any form must
// include this license and copyright notice.
//
// THE SPINE RUNTIMES ARE PROVIDED BY ESOTERIC SOFTWARE LLC "AS IS" AND ANY
// EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED
// WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
// DISCLAIMED. IN NO EVENT SHALL ESOTERIC SOFTWARE LLC BE LIABLE FOR ANY
// DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES
// (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES,
// BUSINESS INTERRUPTION, OR LOSS OF USE, DATA, OR PROFITS) HOWEVER CAUSED AND
// ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT
// (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF
// THE SPINE RUNTIMES, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
//
// Unofficial Rust port of the Spine 4.3 runtime (spine-cpp). Not an official Spine runtime.

use super::{
    draw_order_from_changes, resolve_linked_meshes, Build, ConstraintKind, CurveSink, LinkedMesh,
    LoadError, SourceSkin, VERSION,
};
use crate::animation::{
    AlphaTimeline, Animation, AttachmentTimeline, Bone1Kind, Bone2Kind, BoneTimeline1,
    BoneTimeline2, ColorKind, ColorTimeline, Constraint1Kind, ConstraintTimeline1, DeformTimeline,
    DrawOrderFolderTimeline, DrawOrderTimeline, EventTimeline, IkConstraintTimeline,
    InheritTimeline, PathConstraintMixTimeline, PhysicsConstraintResetTimeline, SequenceTimeline,
    Timeline, TransformConstraintTimeline,
};
use crate::attachment::{Attachment, VertexAttachment};
use crate::attachment_loader::AttachmentLoader;
use crate::bone::BoneData;
use crate::color::Color;
use crate::constraint_data::*;
use crate::enums::{BlendMode, Inherit};
use crate::event::{Event, EventData};
use crate::json::{self, Json, JsonExt};
use crate::sequence::{Sequence, SequenceMode};
use crate::skeleton_data::SkeletonData;
use crate::skin::Skin;
use crate::slot::SlotData;
use smol_str::SmolStr;
use std::sync::Arc;

/// Reads skeleton data from the JSON export format.
pub struct SkeletonJson<'a> {
    attachment_loader: &'a dyn AttachmentLoader,
    scale: f32,
    warnings: Vec<String>,
}

impl<'a> SkeletonJson<'a> {
    pub fn new(attachment_loader: &'a dyn AttachmentLoader) -> Self {
        SkeletonJson {
            attachment_loader,
            scale: 1.0,
            warnings: Vec::new(),
        }
    }

    /// Scales bone lengths and translations, region sizes and other lengths while loading.
    pub fn set_scale(&mut self, scale: f32) {
        self.scale = scale;
    }

    /// Non-fatal problems found in the last read. Currently always empty; kept for loaders that
    /// skip unsupported data.
    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }

    pub fn read_skeleton_data_file(&mut self, path: &str) -> Result<SkeletonData, LoadError> {
        let text = std::fs::read_to_string(path)?;
        let mut data = self.read_skeleton_data(&text)?;
        data.name = file_stem(path);
        Ok(data)
    }

    pub fn read_skeleton_data(&mut self, text: &str) -> Result<SkeletonData, LoadError> {
        let root = json::parse(text)?;
        let mut r = Reader {
            loader: self.attachment_loader,
            scale: self.scale,
            b: Build::new(),
        };
        let result = r.read(&root);
        self.warnings = std::mem::take(&mut r.b.warnings);
        result.map(|()| r.b.finish())
    }
}

pub(super) fn file_stem(path: &str) -> SmolStr {
    let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
    match name.rfind('.') {
        Some(dot) if dot > 0 => name[..dot].into(),
        _ => name.into(),
    }
}

struct Reader<'a> {
    loader: &'a dyn AttachmentLoader,
    scale: f32,
    b: Build,
}

fn children(j: &Json) -> Vec<&Json> {
    j.children().map(|(_, v)| v).collect()
}

fn hex_color(hex: Option<&str>) -> Color {
    let mut c = Color::default();
    if let Some(h) = hex {
        c.set_hex(h);
    }
    c
}

/// An rgb color with alpha 0 where `valueOf` is not used (needs at least 6 characters).
fn hex_rgb(hex: Option<&str>) -> Color {
    let mut c = Color::default();
    if let Some(h) = hex.filter(|h| h.len() >= 6) {
        let mut full = Color::default();
        full.set_hex(h);
        c.r = full.r;
        c.g = full.g;
        c.b = full.b;
    }
    c
}

fn property_scale(name: &str, scale: f32) -> f32 {
    if name == "x" || name == "y" {
        scale
    } else {
        1.0
    }
}

impl Reader<'_> {
    fn read(&mut self, root: &Json) -> Result<(), LoadError> {
        let scale = self.scale;
        if let Some(skeleton) = root.get_ci("skeleton") {
            let d = &mut self.b.data;
            d.hash = skeleton.str("hash").unwrap_or("").into();
            d.version = skeleton.str("spine").unwrap_or("").into();
            if !d.version.starts_with(VERSION) {
                return Err(LoadError(format!(
                    "Skeleton version {} does not match runtime version {VERSION}",
                    d.version
                )));
            }
            d.x = skeleton.float("x", 0.0);
            d.y = skeleton.float("y", 0.0);
            d.width = skeleton.float("width", 0.0);
            d.height = skeleton.float("height", 0.0);
            d.reference_scale = skeleton.float("referenceScale", 100.0) * scale;
            d.fps = skeleton.float("fps", 30.0);
            d.audio_path = skeleton.str("audio").unwrap_or("").into();
            d.images_path = skeleton.str("images").unwrap_or("").into();
        }

        if let Some(bones) = root.get_ci("bones") {
            self.bones(bones)?;
        }
        if let Some(slots) = root.get_ci("slots") {
            self.slots(slots)?;
        }
        if let Some(constraints) = root.get_ci("constraints") {
            self.constraints(constraints)?;
        }
        if let Some(skins) = root.get_ci("skins") {
            self.skins(skins)?;
        }
        resolve_linked_meshes(&mut self.b, true)?;
        if let Some(events) = root.get_ci("events") {
            self.events(events);
        }
        if let Some(animations) = root.get_ci("animations") {
            for (name, map) in animations.children() {
                let animation = self
                    .animation(name, map)
                    .map_err(|e| LoadError(format!("Error reading animation: {name}: {e}")))?;
                self.b.data.animations.push(Arc::new(animation));
            }
        }

        // Slider animations.
        if let Some(constraints) = root.get_ci("constraints") {
            for (_, map) in constraints.children() {
                if map.str("type") != Some("slider") {
                    continue;
                }
                let Some(animation_name) = map.str("animation") else {
                    continue;
                };
                let animation = self
                    .b
                    .data
                    .find_animation(animation_name)
                    .cloned()
                    .ok_or_else(|| {
                        LoadError::new("Slider animation not found: ", animation_name)
                    })?;
                let name = map.str("name").unwrap_or("");
                if let Some(i) = self.b.find_constraint(ConstraintKind::Slider, name) {
                    if let ConstraintData::Slider(s) = &mut self.b.data.constraints[i] {
                        s.animation = Some(animation);
                    }
                }
            }
        }
        Ok(())
    }

    fn bones(&mut self, bones: &Json) -> Result<(), LoadError> {
        let scale = self.scale;
        for (index, map) in bones.children().map(|(_, v)| v).enumerate() {
            let parent = match map.str("parent") {
                Some(name) => Some(
                    self.b
                        .find_bone(name)
                        .ok_or_else(|| LoadError::new("Parent bone not found: ", name))?,
                ),
                None => None,
            };
            let mut data = BoneData::new(index, map.str("name").unwrap_or(""), parent);
            data.length = map.float("length", 0.0) * scale;
            let setup = &mut data.setup_pose;
            setup.x = map.float("x", 0.0) * scale;
            setup.y = map.float("y", 0.0) * scale;
            setup.rotation = map.float("rotation", 0.0);
            setup.scale_x = map.float("scaleX", 1.0);
            setup.scale_y = map.float("scaleY", 1.0);
            setup.shear_x = map.float("shearX", 0.0);
            setup.shear_y = map.float("shearY", 0.0);
            setup.inherit = Inherit::from_name(map.str("inherit").unwrap_or("normal"));
            data.skin_required = map.boolean("skin", false);
            if let Some(color) = map.str("color") {
                data.color.set_hex(color);
            }
            data.icon = map.str("icon").unwrap_or("").into();
            data.icon_size = map.float("iconSize", 1.0);
            data.icon_rotation = map.float("iconRotation", 0.0);
            data.visible = map.boolean("visible", true);
            self.b.push_bone(data);
        }
        Ok(())
    }

    fn slots(&mut self, slots: &Json) -> Result<(), LoadError> {
        for (index, map) in slots.children().map(|(_, v)| v).enumerate() {
            let bone_name = map.str("bone").unwrap_or("");
            let bone = self
                .b
                .find_bone(bone_name)
                .ok_or_else(|| LoadError::new("Slot bone not found: ", bone_name))?;
            let mut data = SlotData::new(
                index,
                map.str("name").unwrap_or(""),
                crate::bone::BoneId(bone),
            );
            if let Some(color) = map.str("color") {
                data.setup_pose.color.set_hex(color);
            }
            if let Some(dark) = map.str("dark") {
                data.setup_pose.dark_color = hex_color(Some(dark));
                data.setup_pose.has_dark_color = true;
            }
            data.attachment_name = map.str("attachment").unwrap_or("").into();
            data.blend_mode = BlendMode::from_name(map.str("blend").unwrap_or("normal"));
            data.visible = map.boolean("visible", true);
            self.b.push_slot(data);
        }
        Ok(())
    }

    fn bone_list(&self, map: &Json, what: &str) -> Result<Vec<usize>, LoadError> {
        let mut out = Vec::new();
        if let Some(list) = map.get_ci("bones") {
            for (_, entry) in list.children() {
                let name = entry.as_str().unwrap_or("");
                out.push(
                    self.b
                        .find_bone(name)
                        .ok_or_else(|| LoadError::new(what, name))?,
                );
            }
        }
        Ok(out)
    }

    fn constraints(&mut self, constraints: &Json) -> Result<(), LoadError> {
        let scale = self.scale;
        for (_, map) in constraints.children() {
            let name = map.str("name").unwrap_or("");
            let skin_required = map.boolean("skin", false);
            let Some(kind) = map.str("type") else {
                continue;
            };
            match kind {
                "ik" => {
                    let mut d = IkConstraintData::new(name);
                    d.skin_required = skin_required;
                    d.bones = self.bone_list(map, "IK bone not found: ")?;
                    let target = map.str("target").unwrap_or("");
                    d.target = self
                        .b
                        .find_bone(target)
                        .ok_or_else(|| LoadError::new("IK target bone not found: ", target))?;
                    if let Some(s) = map.str("scaleY") {
                        d.scale_y_mode = ScaleYMode::from_name(s);
                    }
                    let setup = &mut d.setup_pose;
                    setup.mix = map.float("mix", 1.0);
                    setup.softness = map.float("softness", 0.0) * scale;
                    setup.bend_direction = if map.boolean("bendPositive", true) {
                        1
                    } else {
                        -1
                    };
                    setup.compress = map.boolean("compress", false);
                    setup.stretch = map.boolean("stretch", false);
                    self.b.data.constraints.push(ConstraintData::Ik(d));
                }
                "transform" => {
                    let d = self.transform_constraint(map, name, skin_required)?;
                    self.b.data.constraints.push(ConstraintData::Transform(d));
                }
                "path" => {
                    let mut d = PathConstraintData::new(name);
                    d.skin_required = skin_required;
                    d.bones = self.bone_list(map, "Path bone not found: ")?;
                    let slot = map.str("slot").unwrap_or("");
                    d.slot = self
                        .b
                        .find_slot(slot)
                        .ok_or_else(|| LoadError::new("Path slot not found: ", slot))?;
                    d.position_mode =
                        PositionMode::from_name(map.str("positionMode").unwrap_or("percent"));
                    d.spacing_mode =
                        SpacingMode::from_name(map.str("spacingMode").unwrap_or("length"));
                    d.rotate_mode =
                        RotateMode::from_name(map.str("rotateMode").unwrap_or("tangent"));
                    d.offset_rotation = map.float("rotation", 0.0);
                    let setup = &mut d.setup_pose;
                    setup.position = map.float("position", 0.0);
                    if d.position_mode == PositionMode::Fixed {
                        setup.position *= scale;
                    }
                    setup.spacing = map.float("spacing", 0.0);
                    if matches!(d.spacing_mode, SpacingMode::Length | SpacingMode::Fixed) {
                        setup.spacing *= scale;
                    }
                    setup.mix_rotate = map.float("mixRotate", 1.0);
                    setup.mix_x = map.float("mixX", 1.0);
                    setup.mix_y = map.float("mixY", setup.mix_x);
                    self.b.data.constraints.push(ConstraintData::Path(d));
                }
                "physics" => {
                    let mut d = PhysicsConstraintData::new(name);
                    d.skin_required = skin_required;
                    let bone = map.str("bone").unwrap_or("");
                    d.bone = self
                        .b
                        .find_bone(bone)
                        .ok_or_else(|| LoadError::new("Physics bone not found: ", bone))?;
                    d.x = map.float("x", 0.0);
                    d.y = map.float("y", 0.0);
                    d.rotate = map.float("rotate", 0.0);
                    d.scale_x = map.float("scaleX", 0.0);
                    if let Some(s) = map.str("scaleY") {
                        d.scale_y_mode = ScaleYMode::from_name(s);
                    }
                    d.shear_x = map.float("shearX", 0.0);
                    d.limit = map.float("limit", 5000.0) * scale;
                    d.step = 1.0 / map.int("fps", 60) as f32;
                    let setup = &mut d.setup_pose;
                    setup.inertia = map.float("inertia", 0.5);
                    setup.strength = map.float("strength", 100.0);
                    setup.damping = map.float("damping", 0.85);
                    setup.mass_inverse = 1.0 / map.float("mass", 1.0);
                    setup.wind = map.float("wind", 0.0);
                    setup.gravity = map.float("gravity", 0.0);
                    setup.mix = map.float("mix", 1.0);
                    d.inertia_global = map.boolean("inertiaGlobal", false);
                    d.strength_global = map.boolean("strengthGlobal", false);
                    d.damping_global = map.boolean("dampingGlobal", false);
                    d.mass_global = map.boolean("massGlobal", false);
                    d.wind_global = map.boolean("windGlobal", false);
                    d.gravity_global = map.boolean("gravityGlobal", false);
                    d.mix_global = map.boolean("mixGlobal", false);
                    self.b.data.constraints.push(ConstraintData::Physics(d));
                }
                "slider" => {
                    let mut d = SliderData::new(name);
                    d.skin_required = skin_required;
                    d.additive = map.boolean("additive", false);
                    d.loop_ = map.boolean("loop", false);
                    d.setup_pose.mix = map.float("mix", 1.0);
                    if let Some(bone) = map.str("bone") {
                        d.bone = Some(
                            self.b
                                .find_bone(bone)
                                .ok_or_else(|| LoadError::new("Slider bone not found: ", bone))?,
                        );
                        let property = map.str("property").unwrap_or("");
                        d.property = BoneProperty::from_name(property).map(FromProperty::new);
                        if let Some(from) = &mut d.property {
                            let ps = property_scale(property, scale);
                            from.offset = map.float("from", 0.0) * ps;
                            d.offset = map.float("to", 0.0);
                            d.scale = map.float("scale", 1.0) / ps;
                            d.max = map.float("max", 0.0);
                            d.local = map.boolean("local", false);
                        }
                    } else {
                        d.setup_pose.time = map.float("time", 0.0);
                    }
                    self.b.data.constraints.push(ConstraintData::Slider(d));
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn transform_constraint(
        &self,
        map: &Json,
        name: &str,
        skin_required: bool,
    ) -> Result<TransformConstraintData, LoadError> {
        let scale = self.scale;
        let mut d = TransformConstraintData::new(name);
        d.skin_required = skin_required;
        d.bones = self.bone_list(map, "Transform constraint bone not found: ")?;
        let source = map.str("source").unwrap_or("");
        d.source = self.b.find_bone(source).ok_or_else(|| {
            LoadError::new("Transform constraint source bone not found: ", source)
        })?;
        d.local_source = map.boolean("localSource", false);
        d.local_target = map.boolean("localTarget", false);
        d.additive = map.boolean("additive", false);
        d.clamp = map.boolean("clamp", false);
        let (mut rotate, mut x, mut y, mut scale_x, mut scale_y, mut shear_y) =
            (false, false, false, false, false, false);
        if let Some(properties) = map.get_ci("properties") {
            for (from_name, from_map) in properties.children() {
                let property = BoneProperty::from_name(from_name).ok_or_else(|| {
                    LoadError::new("Invalid transform constraint from property: ", from_name)
                })?;
                let mut from = FromProperty::new(property);
                let from_scale = property_scale(from_name, scale);
                from.offset = from_map.float("offset", 0.0) * from_scale;
                if let Some(to_map) = from_map.get_ci("to") {
                    for (to_name, to_entry) in to_map.children() {
                        let to_property = BoneProperty::from_name(to_name).ok_or_else(|| {
                            LoadError::new("Invalid transform constraint to property: ", to_name)
                        })?;
                        match to_property {
                            BoneProperty::Rotate => rotate = true,
                            BoneProperty::X => x = true,
                            BoneProperty::Y => y = true,
                            BoneProperty::ScaleX => scale_x = true,
                            BoneProperty::ScaleY => scale_y = true,
                            BoneProperty::ShearY => shear_y = true,
                        }
                        let to_scale = if to_property.is_translation() {
                            scale
                        } else {
                            1.0
                        };
                        let mut to = ToProperty::new(to_property);
                        to.offset = to_entry.float("offset", 0.0) * to_scale;
                        to.max = to_entry.float("max", 1.0) * to_scale;
                        to.scale = to_entry.float("scale", 1.0) * to_scale / from_scale;
                        from.to.push(to);
                    }
                }
                if !from.to.is_empty() {
                    d.properties.push(from);
                }
            }
        }
        d.offsets[TransformConstraintData::ROTATION] = map.float("rotation", 0.0);
        d.offsets[TransformConstraintData::X] = map.float("x", 0.0) * scale;
        d.offsets[TransformConstraintData::Y] = map.float("y", 0.0) * scale;
        d.offsets[TransformConstraintData::SCALE_X] = map.float("scaleX", 0.0);
        d.offsets[TransformConstraintData::SCALE_Y] = map.float("scaleY", 0.0);
        d.offsets[TransformConstraintData::SHEAR_Y] = map.float("shearY", 0.0);
        let setup = &mut d.setup_pose;
        if rotate {
            setup.mix_rotate = map.float("mixRotate", 1.0);
        }
        if x {
            setup.mix_x = map.float("mixX", 1.0);
        }
        if y {
            setup.mix_y = map.float("mixY", setup.mix_x);
        }
        if scale_x {
            setup.mix_scale_x = map.float("mixScaleX", 1.0);
        }
        if scale_y {
            setup.mix_scale_y = map.float("mixScaleY", setup.mix_scale_x);
        }
        if shear_y {
            setup.mix_shear_y = map.float("mixShearY", 1.0);
        }
        Ok(d)
    }

    fn skins(&mut self, skins: &Json) -> Result<(), LoadError> {
        for (_, map) in skins.children() {
            let mut skin = Skin::new(map.str("name").unwrap_or(""));
            if let Some(list) = map.get_ci("bones") {
                for (_, entry) in list.children() {
                    let name = entry.as_str().unwrap_or("");
                    skin.bones.push(
                        self.b
                            .find_bone(name)
                            .ok_or_else(|| LoadError::new("Skin bone not found: ", name))?,
                    );
                }
            }
            for (key, kind, what) in [
                ("ik", ConstraintKind::Ik, "Skin IK constraint not found: "),
                (
                    "transform",
                    ConstraintKind::Transform,
                    "Skin transform constraint not found: ",
                ),
                (
                    "path",
                    ConstraintKind::Path,
                    "Skin path constraint not found: ",
                ),
                (
                    "physics",
                    ConstraintKind::Physics,
                    "Skin physics constraint not found: ",
                ),
                ("slider", ConstraintKind::Slider, "Skin slider not found: "),
            ] {
                if let Some(list) = map.get_ci(key) {
                    for (_, entry) in list.children() {
                        let name = entry.as_str().unwrap_or("");
                        skin.constraints.push(
                            self.b
                                .find_constraint(kind, name)
                                .ok_or_else(|| LoadError::new(what, name))?,
                        );
                    }
                }
            }
            if let Some(attachments) = map.get_ci("attachments") {
                for (slot_name, slot_map) in attachments.children() {
                    let slot = self
                        .b
                        .find_slot(slot_name)
                        .ok_or_else(|| LoadError::new("Skin slot not found: ", slot_name))?;
                    for (placeholder, att_map) in slot_map.children() {
                        let attachment = self
                            .attachment(att_map, &skin, slot, placeholder)?
                            .ok_or_else(|| {
                                LoadError::new("Error reading attachment: ", placeholder)
                            })?;
                        skin.set_attachment(slot, placeholder, Arc::new(attachment));
                    }
                }
            }
            if let Some(color) = map.str("color") {
                skin.color.set_hex(color);
            }
            if skin.name() == "default" {
                self.b.default_skin = Some(self.b.skins.len());
            }
            self.b.skins.push(skin);
        }
        Ok(())
    }

    fn sequence(&self, item: Option<&Json>) -> Sequence {
        match item {
            None => Sequence::new(1, false),
            Some(item) => {
                let mut s = Sequence::new(item.int("count", 0).max(0) as usize, true);
                s.start = item.int("start", 1);
                s.digits = item.int("digits", 0);
                s.setup_index = item.int("setup", 0);
                s
            }
        }
    }

    /// Reads vertices into `vertex`; `vertices_length` is the unweighted float count.
    fn vertices(&self, map: &Json, vertex: &mut VertexAttachment, vertices_length: usize) {
        let scale = self.scale;
        vertex.world_vertices_length = vertices_length;
        let Some(mut vertices) = map.get_ci("vertices").and_then(|v| v.float_array()) else {
            return;
        };
        if vertices.len() == vertices_length {
            if scale != 1.0 {
                for v in &mut vertices {
                    *v *= scale;
                }
            }
            vertex.vertices = vertices;
            return;
        }
        let mut bones = Vec::with_capacity(vertices_length * 3);
        let mut weights = Vec::with_capacity(vertices_length * 9);
        let n = vertices.len();
        let mut i = 0;
        while i < n {
            let bone_count = vertices[i] as i32 as usize;
            i += 1;
            bones.push(bone_count);
            for _ in 0..bone_count {
                if i + 3 >= n {
                    break;
                }
                bones.push(vertices[i] as i32 as usize);
                weights.push(vertices[i + 1] * scale);
                weights.push(vertices[i + 2] * scale);
                weights.push(vertices[i + 3]);
                i += 4;
            }
        }
        vertex.bones = bones;
        vertex.vertices = weights;
    }

    fn attachment(
        &mut self,
        map: &Json,
        skin: &Skin,
        slot_index: usize,
        placeholder: &str,
    ) -> Result<Option<Attachment>, LoadError> {
        let scale = self.scale;
        let name = map.str("name").unwrap_or(placeholder);
        let kind = map.str("type").unwrap_or("region");
        let color = map.str("color");
        Ok(Some(match kind {
            "boundingbox" => {
                let Some(mut a) = self
                    .loader
                    .new_bounding_box_attachment(skin, placeholder, name)
                else {
                    return Ok(None);
                };
                self.vertices(
                    map,
                    &mut a.vertex,
                    (map.int("vertexCount", 0) << 1) as usize,
                );
                if let Some(c) = color {
                    a.color.set_hex(c);
                }
                Attachment::BoundingBox(a)
            }
            "mesh" | "linkedmesh" => {
                let path = map.str("path").unwrap_or(name);
                let sequence = self.sequence(map.get_ci("sequence"));
                let Some(mut mesh) =
                    self.loader
                        .new_mesh_attachment(skin, placeholder, name, path, sequence)
                else {
                    return Ok(None);
                };
                mesh.path = path.into();
                if let Some(c) = color {
                    mesh.color.set_hex(c);
                }
                mesh.width = map.float("width", 0.0) * scale;
                mesh.height = map.float("height", 0.0) * scale;
                if let Some(source) = map.str("source") {
                    let source_slot = match map.str("slot") {
                        Some(slot) => self
                            .b
                            .find_slot(slot)
                            .ok_or_else(|| LoadError::new("Source mesh slot not found: ", slot))?,
                        None => slot_index,
                    };
                    self.b.linked_meshes.push(LinkedMesh {
                        skin: self.b.skins.len(),
                        slot_index,
                        placeholder: placeholder.into(),
                        source_skin: match map.str("skin") {
                            Some(s) => SourceSkin::Name(s.into()),
                            None => SourceSkin::Default,
                        },
                        source_slot,
                        source: source.into(),
                        inherit_timelines: map.boolean("timelines", true),
                    });
                    return Ok(Some(Attachment::Mesh(mesh)));
                }
                let uvs = map
                    .get_ci("uvs")
                    .and_then(|v| v.float_array())
                    .ok_or_else(|| LoadError::new("Mesh uvs missing: ", name))?;
                self.vertices(map, &mut mesh.vertex, uvs.len());
                mesh.triangles = map
                    .get_ci("triangles")
                    .and_then(|v| v.u16_array())
                    .ok_or_else(|| LoadError::new("Mesh triangles missing: ", name))?;
                mesh.region_uvs = uvs;
                if map.int("hull", 0) != 0 {
                    mesh.hull_length = (map.int("hull", 0) << 1) as usize;
                }
                if let Some(edges) = map.get_ci("edges").and_then(|v| v.u16_array()) {
                    mesh.edges = edges;
                }
                mesh.update_sequence();
                Attachment::Mesh(mesh)
            }
            "path" => {
                let Some(mut a) = self.loader.new_path_attachment(skin, placeholder, name) else {
                    return Ok(None);
                };
                a.closed = map.boolean("closed", false);
                a.constant_speed = map.boolean("constantSpeed", true);
                self.vertices(
                    map,
                    &mut a.vertex,
                    (map.int("vertexCount", 0) << 1) as usize,
                );
                a.lengths = map
                    .get_ci("lengths")
                    .and_then(|v| v.float_array())
                    .ok_or_else(|| LoadError::new("Path lengths missing: ", name))?;
                for l in &mut a.lengths {
                    *l *= scale;
                }
                if let Some(c) = color {
                    a.color.set_hex(c);
                }
                Attachment::Path(a)
            }
            "point" => {
                let Some(mut a) = self.loader.new_point_attachment(skin, placeholder, name) else {
                    return Ok(None);
                };
                a.x = map.float("x", 0.0) * scale;
                a.y = map.float("y", 0.0) * scale;
                a.rotation = map.float("rotation", 0.0);
                if let Some(c) = color {
                    a.color.set_hex(c);
                }
                Attachment::Point(a)
            }
            "clipping" => {
                let Some(mut a) = self.loader.new_clipping_attachment(skin, placeholder, name)
                else {
                    return Ok(None);
                };
                if let Some(end) = map.str("end") {
                    let Some(slot) = self.b.find_slot(end) else {
                        return Ok(None);
                    };
                    a.end_slot = Some(slot);
                }
                a.convex = map.boolean("convex", false);
                a.inverse = map.boolean("inverse", false);
                self.vertices(
                    map,
                    &mut a.vertex,
                    (map.int("vertexCount", 0) << 1) as usize,
                );
                if let Some(c) = color {
                    a.color.set_hex(c);
                }
                Attachment::Clipping(a)
            }
            // "region" and unknown types.
            _ => {
                let path = map.str("path").unwrap_or(name);
                let sequence = self.sequence(map.get_ci("sequence"));
                let Some(mut a) =
                    self.loader
                        .new_region_attachment(skin, placeholder, name, path, sequence)
                else {
                    return Ok(None);
                };
                a.path = path.into();
                a.x = map.float("x", 0.0) * scale;
                a.y = map.float("y", 0.0) * scale;
                a.scale_x = map.float("scaleX", 1.0);
                a.scale_y = map.float("scaleY", 1.0);
                a.rotation = map.float("rotation", 0.0);
                a.width = map.float("width", 0.0) * scale;
                a.height = map.float("height", 0.0) * scale;
                if let Some(c) = color {
                    a.color.set_hex(c);
                }
                a.update_sequence();
                Attachment::Region(a)
            }
        }))
    }

    fn events(&mut self, events: &Json) {
        for (name, map) in events.children() {
            let mut d = EventData::new(name);
            d.int_value = map.int("int", 0);
            d.float_value = map.float("float", 0.0);
            d.string_value = map.str("string").unwrap_or("").into();
            d.audio_path = map.str("audio").unwrap_or("").into();
            if !d.audio_path.is_empty() {
                d.volume = map.float("volume", 1.0);
                d.balance = map.float("balance", 0.0);
            }
            self.b.data.events.push(Arc::new(d));
        }
    }

    fn animation(&mut self, name: &str, map: &Json) -> Result<Animation, LoadError> {
        let scale = self.scale;
        let mut timelines: Vec<Timeline> = Vec::new();
        let mut bones: Vec<usize> = Vec::new();

        // Slot timelines.
        if let Some(slots) = map.get_ci("slots") {
            for (slot_name, slot_map) in slots.children() {
                let slot = self
                    .b
                    .find_slot(slot_name)
                    .ok_or_else(|| LoadError::new("Slot not found: ", slot_name))?;
                for (timeline_name, timeline_map) in slot_map.children() {
                    let keys = children(timeline_map);
                    if keys.is_empty() {
                        continue;
                    }
                    let frames = keys.len();
                    match timeline_name {
                        "attachment" => {
                            let mut t = AttachmentTimeline::new(frames, slot);
                            for (frame, key) in keys.iter().enumerate() {
                                t.set_frame(
                                    frame,
                                    key.float("time", 0.0),
                                    key.str("name").unwrap_or(""),
                                );
                            }
                            timelines.push(Timeline::Attachment(t));
                        }
                        "rgba" => {
                            let mut t =
                                ColorTimeline::new(ColorKind::Rgba, frames, frames << 2, slot);
                            read_curved(
                                &mut t,
                                &keys,
                                |k| {
                                    let c = hex_color(k.str("color"));
                                    [c.r, c.g, c.b, c.a]
                                },
                                [1.0; 4],
                                |t, f, time, v| t.set_frame(f, time, &v),
                            )?;
                            timelines.push(Timeline::Color(t));
                        }
                        "rgb" => {
                            let mut t =
                                ColorTimeline::new(ColorKind::Rgb, frames, frames * 3, slot);
                            read_curved(
                                &mut t,
                                &keys,
                                |k| {
                                    let c = hex_rgb(k.str("color"));
                                    [c.r, c.g, c.b]
                                },
                                [1.0; 3],
                                |t, f, time, v| t.set_frame(f, time, &v),
                            )?;
                            timelines.push(Timeline::Color(t));
                        }
                        "alpha" => {
                            let mut t = AlphaTimeline::new(frames, frames, slot);
                            read_curved(
                                &mut t,
                                &keys,
                                |k| [k.float("value", 0.0)],
                                [1.0],
                                |t, f, time, v| t.set_frame(f, time, v[0]),
                            )?;
                            timelines.push(Timeline::Alpha(t));
                        }
                        "rgba2" => {
                            let mut t =
                                ColorTimeline::new(ColorKind::Rgba2, frames, frames * 7, slot);
                            read_curved(
                                &mut t,
                                &keys,
                                |k| {
                                    let light = hex_color(k.str("light"));
                                    let dark = hex_rgb(k.str("dark"));
                                    [light.r, light.g, light.b, light.a, dark.r, dark.g, dark.b]
                                },
                                [1.0; 7],
                                |t, f, time, v| t.set_frame(f, time, &v),
                            )?;
                            timelines.push(Timeline::Color(t));
                        }
                        "rgb2" => {
                            let mut t =
                                ColorTimeline::new(ColorKind::Rgb2, frames, frames * 6, slot);
                            read_curved(
                                &mut t,
                                &keys,
                                |k| {
                                    let light = hex_rgb(k.str("light"));
                                    let dark = hex_rgb(k.str("dark"));
                                    [light.r, light.g, light.b, dark.r, dark.g, dark.b]
                                },
                                [1.0; 6],
                                |t, f, time, v| t.set_frame(f, time, &v),
                            )?;
                            timelines.push(Timeline::Color(t));
                        }
                        other => return Err(LoadError::new("Invalid slot timeline: ", other)),
                    }
                }
            }
        }

        // Bone timelines.
        if let Some(bone_maps) = map.get_ci("bones") {
            for (bone_name, bone_map) in bone_maps.children() {
                let bone = self
                    .b
                    .find_bone(bone_name)
                    .ok_or_else(|| LoadError::new("Bone not found: ", bone_name))?;
                bones.push(bone);
                for (timeline_name, timeline_map) in bone_map.children() {
                    let keys = children(timeline_map);
                    if keys.is_empty() {
                        continue;
                    }
                    let frames = keys.len();
                    let one = |kind: Bone1Kind,
                               default: f32,
                               value_scale: f32,
                               timelines: &mut Vec<Timeline>|
                     -> Result<(), LoadError> {
                        let mut t = BoneTimeline1::new(kind, frames, frames, bone);
                        read_curved(
                            &mut t,
                            &keys,
                            |k| [k.float("value", default) * value_scale],
                            [value_scale],
                            |t, f, time, v| t.set_frame(f, time, v[0]),
                        )?;
                        timelines.push(Timeline::Bone1(t));
                        Ok(())
                    };
                    let two = |kind: Bone2Kind,
                               default: f32,
                               value_scale: f32,
                               timelines: &mut Vec<Timeline>|
                     -> Result<(), LoadError> {
                        let mut t = BoneTimeline2::new(kind, frames, frames << 1, bone);
                        read_curved(
                            &mut t,
                            &keys,
                            |k| {
                                [
                                    k.float("x", default) * value_scale,
                                    k.float("y", default) * value_scale,
                                ]
                            },
                            [value_scale; 2],
                            |t, f, time, v| t.set_frame(f, time, v[0], v[1]),
                        )?;
                        timelines.push(Timeline::Bone2(t));
                        Ok(())
                    };
                    match timeline_name {
                        "rotate" => one(Bone1Kind::Rotate, 0.0, 1.0, &mut timelines)?,
                        "translate" => two(Bone2Kind::Translate, 0.0, scale, &mut timelines)?,
                        "translatex" => one(Bone1Kind::X, 0.0, scale, &mut timelines)?,
                        "translatey" => one(Bone1Kind::Y, 0.0, scale, &mut timelines)?,
                        "scale" => two(Bone2Kind::Scale, 1.0, 1.0, &mut timelines)?,
                        "scalex" => one(Bone1Kind::ScaleX, 1.0, 1.0, &mut timelines)?,
                        "scaley" => one(Bone1Kind::ScaleY, 1.0, 1.0, &mut timelines)?,
                        "shear" => two(Bone2Kind::Shear, 0.0, 1.0, &mut timelines)?,
                        "shearx" => one(Bone1Kind::ShearX, 0.0, 1.0, &mut timelines)?,
                        "sheary" => one(Bone1Kind::ShearY, 0.0, 1.0, &mut timelines)?,
                        "inherit" => {
                            let mut t = InheritTimeline::new(frames, bone);
                            for (frame, key) in keys.iter().enumerate() {
                                t.set_frame(
                                    frame,
                                    key.float("time", 0.0),
                                    Inherit::from_name(key.str("inherit").unwrap_or("normal")),
                                );
                            }
                            timelines.push(Timeline::Inherit(t));
                        }
                        other => return Err(LoadError::new("Invalid bone timeline: ", other)),
                    }
                }
            }
        }

        // IK constraint timelines.
        if let Some(group) = map.get_ci("ik") {
            for (constraint_name, timeline_map) in group.children() {
                let keys = children(timeline_map);
                if keys.is_empty() {
                    continue;
                }
                let ci = self
                    .b
                    .find_constraint(ConstraintKind::Ik, constraint_name)
                    .ok_or_else(|| LoadError::new("IK constraint not found: ", constraint_name))?;
                let frames = keys.len();
                let mut t = IkConstraintTimeline::new(frames, frames << 1, ci);
                let mut time = keys[0].float("time", 0.0);
                let mut mix = keys[0].float("mix", 1.0);
                let mut softness = keys[0].float("softness", 0.0) * scale;
                let mut bezier = 0;
                for frame in 0..frames {
                    let key = keys[frame];
                    t.set_frame(
                        frame,
                        time,
                        mix,
                        softness,
                        if key.boolean("bendPositive", true) {
                            1
                        } else {
                            -1
                        },
                        key.boolean("compress", false),
                        key.boolean("stretch", false),
                    );
                    let Some(next) = keys.get(frame + 1) else {
                        break;
                    };
                    let time2 = next.float("time", 0.0);
                    let mix2 = next.float("mix", 1.0);
                    let softness2 = next.float("softness", 0.0) * scale;
                    if let Some(curve) = key.get_ci("curve") {
                        bezier = read_curve(
                            curve,
                            &mut t.curve,
                            bezier,
                            frame,
                            0,
                            time,
                            time2,
                            mix,
                            mix2,
                            1.0,
                        )?;
                        bezier = read_curve(
                            curve,
                            &mut t.curve,
                            bezier,
                            frame,
                            1,
                            time,
                            time2,
                            softness,
                            softness2,
                            scale,
                        )?;
                    }
                    time = time2;
                    mix = mix2;
                    softness = softness2;
                }
                timelines.push(Timeline::Ik(t));
            }
        }

        // Transform constraint timelines.
        if let Some(group) = map.get_ci("transform") {
            for (constraint_name, timeline_map) in group.children() {
                let keys = children(timeline_map);
                if keys.is_empty() {
                    continue;
                }
                let ci = self
                    .b
                    .find_constraint(ConstraintKind::Transform, constraint_name)
                    .ok_or_else(|| {
                        LoadError::new("Transform constraint not found: ", constraint_name)
                    })?;
                let frames = keys.len();
                let mut t = TransformConstraintTimeline::new(frames, frames * 6, ci);
                read_curved(
                    &mut t,
                    &keys,
                    |k| {
                        let mix_x = k.float("mixX", 1.0);
                        [
                            k.float("mixRotate", 1.0),
                            mix_x,
                            k.float("mixY", mix_x),
                            k.float("mixScaleX", 1.0),
                            k.float("mixScaleY", 1.0),
                            k.float("mixShearY", 1.0),
                        ]
                    },
                    [1.0; 6],
                    |t, f, time, v| t.set_frame(f, time, v[0], v[1], v[2], v[3], v[4], v[5]),
                )?;
                timelines.push(Timeline::Transform(t));
            }
        }

        // Path constraint timelines.
        if let Some(group) = map.get_ci("path") {
            for (constraint_name, constraint_map) in group.children() {
                let ci = self
                    .b
                    .find_constraint(ConstraintKind::Path, constraint_name)
                    .ok_or_else(|| {
                        LoadError::new("Path constraint not found: ", constraint_name)
                    })?;
                let ConstraintData::Path(pd) = &self.b.data.constraints[ci] else {
                    continue;
                };
                let (position_scale, spacing_scale) = (
                    if pd.position_mode == PositionMode::Fixed {
                        scale
                    } else {
                        1.0
                    },
                    if matches!(pd.spacing_mode, SpacingMode::Length | SpacingMode::Fixed) {
                        scale
                    } else {
                        1.0
                    },
                );
                for (timeline_name, timeline_map) in constraint_map.children() {
                    let keys = children(timeline_map);
                    if keys.is_empty() {
                        continue;
                    }
                    let frames = keys.len();
                    match timeline_name {
                        "position" | "spacing" => {
                            let (kind, value_scale) = if timeline_name == "position" {
                                (Constraint1Kind::PathPosition, position_scale)
                            } else {
                                (Constraint1Kind::PathSpacing, spacing_scale)
                            };
                            let mut t = ConstraintTimeline1::new(kind, frames, frames, Some(ci));
                            read_curved(
                                &mut t,
                                &keys,
                                |k| [k.float("value", 0.0) * value_scale],
                                [value_scale],
                                |t, f, time, v| t.set_frame(f, time, v[0]),
                            )?;
                            timelines.push(Timeline::Constraint1(t));
                        }
                        "mix" => {
                            let mut t = PathConstraintMixTimeline::new(frames, frames * 3, ci);
                            read_curved(
                                &mut t,
                                &keys,
                                |k| {
                                    let mix_x = k.float("mixX", 1.0);
                                    [k.float("mixRotate", 1.0), mix_x, k.float("mixY", mix_x)]
                                },
                                [1.0; 3],
                                |t, f, time, v| t.set_frame(f, time, v[0], v[1], v[2]),
                            )?;
                            timelines.push(Timeline::PathMix(t));
                        }
                        _ => {}
                    }
                }
            }
        }

        // Physics constraint timelines.
        if let Some(group) = map.get_ci("physics") {
            for (constraint_name, constraint_map) in group.children() {
                let ci = if constraint_name.is_empty() {
                    None
                } else {
                    Some(
                        self.b
                            .find_constraint(ConstraintKind::Physics, constraint_name)
                            .ok_or_else(|| {
                                LoadError::new("Physics constraint not found: ", constraint_name)
                            })?,
                    )
                };
                for (timeline_name, timeline_map) in constraint_map.children() {
                    let keys = children(timeline_map);
                    if keys.is_empty() {
                        continue;
                    }
                    let frames = keys.len();
                    if timeline_name == "reset" {
                        let mut t = PhysicsConstraintResetTimeline::new(frames, ci);
                        for (frame, key) in keys.iter().enumerate() {
                            t.set_frame(frame, key.float("time", 0.0));
                        }
                        timelines.push(Timeline::PhysicsReset(t));
                        continue;
                    }
                    let (kind, default) = match timeline_name {
                        "inertia" => (Constraint1Kind::PhysicsInertia, 0.0),
                        "strength" => (Constraint1Kind::PhysicsStrength, 0.0),
                        "damping" => (Constraint1Kind::PhysicsDamping, 0.0),
                        "mass" => (Constraint1Kind::PhysicsMass, 0.0),
                        "wind" => (Constraint1Kind::PhysicsWind, 0.0),
                        "gravity" => (Constraint1Kind::PhysicsGravity, 0.0),
                        "mix" => (Constraint1Kind::PhysicsMix, 1.0),
                        _ => continue,
                    };
                    let mut t = ConstraintTimeline1::new(kind, frames, frames, ci);
                    read_curved(
                        &mut t,
                        &keys,
                        |k| [k.float("value", default)],
                        [1.0],
                        |t, f, time, v| t.set_frame(f, time, v[0]),
                    )?;
                    timelines.push(Timeline::Constraint1(t));
                }
            }
        }

        // Slider timelines.
        if let Some(group) = map.get_ci("slider") {
            for (constraint_name, constraint_map) in group.children() {
                let ci = self
                    .b
                    .find_constraint(ConstraintKind::Slider, constraint_name)
                    .ok_or_else(|| LoadError::new("Slider not found: ", constraint_name))?;
                for (timeline_name, timeline_map) in constraint_map.children() {
                    let keys = children(timeline_map);
                    if keys.is_empty() {
                        continue;
                    }
                    let kind = match timeline_name {
                        "time" => Constraint1Kind::SliderTime,
                        "mix" => Constraint1Kind::SliderMix,
                        _ => continue,
                    };
                    let mut t = ConstraintTimeline1::new(kind, keys.len(), keys.len(), Some(ci));
                    read_curved(
                        &mut t,
                        &keys,
                        |k| [k.float("value", 1.0)],
                        [1.0],
                        |t, f, time, v| t.set_frame(f, time, v[0]),
                    )?;
                    timelines.push(Timeline::Constraint1(t));
                }
            }
        }

        // Attachment timelines (deform and sequence).
        if let Some(skins) = map.get_ci("attachments") {
            for (skin_name, skin_map) in skins.children() {
                let skin_index = self
                    .b
                    .find_skin(skin_name)
                    .ok_or_else(|| LoadError::new("Skin not found: ", skin_name))?;
                for (slot_name, slot_map) in skin_map.children() {
                    let slot = self
                        .b
                        .find_slot(slot_name)
                        .ok_or_else(|| LoadError::new("Attachment slot not found: ", slot_name))?;
                    for (att_name, att_map) in slot_map.children() {
                        let attachment = self.b.skins[skin_index]
                            .attachment(slot, att_name)
                            .cloned()
                            .ok_or_else(|| {
                                LoadError::new("Timeline attachment not found: ", att_name)
                            })?;
                        for (timeline_name, timeline_map) in att_map.children() {
                            let keys = children(timeline_map);
                            let frames = keys.len();
                            if timeline_name == "deform" {
                                timelines.push(Timeline::Deform(self.deform_timeline(
                                    &keys,
                                    slot,
                                    &attachment,
                                )?));
                            } else if timeline_name == "sequence" {
                                let mut t = SequenceTimeline::new(frames, slot, attachment.clone());
                                let mut last_delay = 0.0;
                                for (frame, key) in keys.iter().enumerate() {
                                    let delay = key.float("delay", last_delay);
                                    t.set_frame(
                                        frame,
                                        key.float("time", 0.0),
                                        SequenceMode::from_name(key.str("mode").unwrap_or("hold")),
                                        key.int("index", 0),
                                        delay,
                                    );
                                    last_delay = delay;
                                }
                                timelines.push(Timeline::Sequence(t));
                            }
                        }
                    }
                }
            }
        }

        // Draw order.
        if let Some(draw_order) = map.get_ci("drawOrder") {
            let keys = children(draw_order);
            let mut t = DrawOrderTimeline::new(keys.len());
            let slot_count = self.b.data.slots.len();
            for (frame, key) in keys.iter().enumerate() {
                let order = self.draw_order(key, slot_count, None)?;
                t.set_frame(
                    frame,
                    key.float("time", 0.0),
                    if order.is_empty() { None } else { Some(&order) },
                );
            }
            timelines.push(Timeline::DrawOrder(t));
        }

        // Draw order folders.
        if let Some(folders) = map.get_ci("drawOrderFolder") {
            for (_, folder) in folders.children() {
                let mut folder_slots = Vec::new();
                if let Some(slot_list) = folder.get_ci("slots") {
                    for (_, entry) in slot_list.children() {
                        let slot_name = entry.as_str().unwrap_or("");
                        folder_slots.push(self.b.find_slot(slot_name).ok_or_else(|| {
                            LoadError::new("Draw order folder slot not found: ", slot_name)
                        })?);
                    }
                }
                let keys = folder.get_ci("keys").map(children).unwrap_or_default();
                let mut t = DrawOrderFolderTimeline::new(
                    keys.len(),
                    folder_slots.clone(),
                    self.b.data.slots.len(),
                );
                for (frame, key) in keys.iter().enumerate() {
                    let order = self.draw_order(key, folder_slots.len(), Some(&folder_slots))?;
                    t.set_frame(
                        frame,
                        key.float("time", 0.0),
                        if order.is_empty() { None } else { Some(&order) },
                    );
                }
                timelines.push(Timeline::DrawOrderFolder(t));
            }
        }

        // Events.
        if let Some(events) = map.get_ci("events") {
            let keys = children(events);
            if !keys.is_empty() {
                let mut t = EventTimeline::new(keys.len());
                for (frame, key) in keys.iter().enumerate() {
                    let event_name = key.str("name").unwrap_or("");
                    let data = self
                        .b
                        .data
                        .find_event(event_name)
                        .cloned()
                        .ok_or_else(|| LoadError::new("Event not found: ", event_name))?;
                    let mut e = Event::new(key.float("time", 0.0), data.clone());
                    e.int_value = key.int("int", data.int_value);
                    e.float_value = key.float("float", data.float_value);
                    e.string_value = key.str("string").unwrap_or(&data.string_value).into();
                    if !data.audio_path.is_empty() {
                        e.volume = key.float("volume", data.volume);
                        e.balance = key.float("balance", data.balance);
                    }
                    t.set_frame(frame, Arc::new(e));
                }
                timelines.push(Timeline::Event(t));
            }
        }

        let duration = timelines.iter().fold(0.0f32, |d, t| d.max(t.duration()));
        let mut animation = Animation::new_with_bones(name, timelines, duration, bones);
        if let Some(color) = map.str("color") {
            animation.color.set_hex(color);
        }
        Ok(animation)
    }

    fn deform_timeline(
        &self,
        keys: &[&Json],
        slot: usize,
        attachment: &Arc<Attachment>,
    ) -> Result<DeformTimeline, LoadError> {
        let scale = self.scale;
        let vertex = attachment.as_vertex().ok_or_else(|| {
            LoadError::new(
                "Deform attachment is not a vertex attachment: ",
                attachment.name(),
            )
        })?;
        let weighted = !vertex.bones.is_empty();
        let setup = &vertex.vertices;
        let deform_length = if weighted {
            setup.len() / 3 * 2
        } else {
            setup.len()
        };
        let frames = keys.len();
        let mut t = DeformTimeline::new(frames, frames, slot, attachment.clone());
        let mut time = keys[0].float("time", 0.0);
        let mut bezier = 0;
        for frame in 0..frames {
            let key = keys[frame];
            let deform = match key.get_ci("vertices") {
                None if weighted => vec![0.0; deform_length],
                None => setup.clone(),
                Some(values) => {
                    let mut deform = vec![0.0f32; deform_length];
                    let start = key.int("offset", 0).max(0) as usize;
                    for (i, (_, v)) in values.children().enumerate() {
                        if let Some(d) = deform.get_mut(start + i) {
                            *d = v.as_float();
                        }
                    }
                    if scale != 1.0 {
                        for (i, _) in values.children().enumerate() {
                            if let Some(d) = deform.get_mut(start + i) {
                                *d *= scale;
                            }
                        }
                    }
                    if !weighted {
                        for (d, s) in deform.iter_mut().zip(setup) {
                            *d += s;
                        }
                    }
                    deform
                }
            };
            t.set_frame(frame, time, &deform);
            let Some(next) = keys.get(frame + 1) else {
                break;
            };
            let time2 = next.float("time", 0.0);
            if let Some(curve) = key.get_ci("curve") {
                bezier = read_curve(curve, &mut t, bezier, frame, 0, time, time2, 0.0, 1.0, 1.0)?;
            }
            time = time2;
        }
        Ok(t)
    }

    /// Reads a draw order key's `offsets`; `folder` maps positions to slot indices for folders.
    fn draw_order(
        &self,
        key: &Json,
        slot_count: usize,
        folder: Option<&[usize]>,
    ) -> Result<Vec<usize>, LoadError> {
        let Some(offsets) = key.get_ci("offsets") else {
            return Ok(Vec::new());
        };
        let mut changes = Vec::new();
        for (_, offset_map) in offsets.children() {
            let slot_name = offset_map.str("slot").unwrap_or("");
            let slot = self
                .b
                .find_slot(slot_name)
                .ok_or_else(|| LoadError::new("Draw order slot not found: ", slot_name))?;
            let index = match folder {
                None => slot,
                Some(folder_slots) => folder_slots
                    .iter()
                    .take(slot_count)
                    .position(|&s| s == slot)
                    .ok_or_else(|| LoadError::new("Slot not in folder: ", slot_name))?,
            };
            changes.push((index, offset_map.int("offset", 0)));
        }
        draw_order_from_changes(slot_count, &changes)
    }
}

/// Reads the keys of a curve timeline: `extract` pulls the `N` values out of a key, `set_frame`
/// stores a frame, and curves between keys are read into `t`. `scales` scale the bezier y values.
fn read_curved<T: CurveSink, const N: usize>(
    t: &mut T,
    keys: &[&Json],
    extract: impl Fn(&Json) -> [f32; N],
    scales: [f32; N],
    set_frame: impl Fn(&mut T, usize, f32, [f32; N]),
) -> Result<(), LoadError> {
    let mut time = keys[0].float("time", 0.0);
    let mut values = extract(keys[0]);
    let mut bezier = 0;
    for frame in 0..keys.len() {
        set_frame(t, frame, time, values);
        let Some(next) = keys.get(frame + 1) else {
            break;
        };
        let time2 = next.float("time", 0.0);
        let values2 = extract(next);
        if let Some(curve) = keys[frame].get_ci("curve") {
            for k in 0..N {
                bezier = read_curve(
                    curve, t, bezier, frame, k, time, time2, values[k], values2[k], scales[k],
                )?;
            }
        }
        time = time2;
        values = values2;
    }
    Ok(())
}

/// Reads the curve for `value` of the key at `frame` (a "stepped" string or bezier control points).
#[allow(clippy::too_many_arguments)]
fn read_curve(
    curve: &Json,
    sink: &mut dyn CurveSink,
    bezier: usize,
    frame: usize,
    value: usize,
    time1: f32,
    time2: f32,
    value1: f32,
    value2: f32,
    scale: f32,
) -> Result<usize, LoadError> {
    if let Json::String(s) = curve {
        if s == "stepped" {
            sink.set_stepped(frame);
        }
        return Ok(bezier);
    }
    let points = curve
        .as_array()
        .ok_or_else(|| LoadError::new("Invalid curve", ""))?;
    let at = |i: usize| {
        points
            .get(value * 4 + i)
            .map(|j| j.as_float())
            .ok_or_else(|| LoadError::new("Curve is too short", ""))
    };
    let (cx1, cy1, cx2, cy2) = (at(0)?, at(1)? * scale, at(2)?, at(3)? * scale);
    sink.set_bezier(
        bezier, frame, value, time1, value1, cx1, cy1, cx2, cy2, time2, value2,
    );
    Ok(bezier + 1)
}
