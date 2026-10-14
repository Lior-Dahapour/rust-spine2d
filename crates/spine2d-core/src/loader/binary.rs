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

use super::json::file_stem;
use super::{
    draw_order_from_changes, resolve_linked_meshes, Build, CurveSink, LinkedMesh, LoadError,
    SourceSkin, VERSION,
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
use crate::sequence::{Sequence, SequenceMode};
use crate::skeleton_data::SkeletonData;
use crate::skin::Skin;
use crate::slot::SlotData;
use smol_str::SmolStr;
use std::sync::Arc;

// Timeline type ids of the binary format.
const BONE_ROTATE: u8 = 0;
const BONE_TRANSLATE: u8 = 1;
const BONE_TRANSLATEX: u8 = 2;
const BONE_TRANSLATEY: u8 = 3;
const BONE_SCALE: u8 = 4;
const BONE_SCALEX: u8 = 5;
const BONE_SCALEY: u8 = 6;
const BONE_SHEAR: u8 = 7;
const BONE_SHEARX: u8 = 8;
const BONE_SHEARY: u8 = 9;
const BONE_INHERIT: u8 = 10;
const SLOT_ATTACHMENT: u8 = 0;
const SLOT_RGBA: u8 = 1;
const SLOT_RGB: u8 = 2;
const SLOT_RGBA2: u8 = 3;
const SLOT_RGB2: u8 = 4;
const SLOT_ALPHA: u8 = 5;
const CONSTRAINT_IK: u8 = 0;
const CONSTRAINT_PATH: u8 = 1;
const CONSTRAINT_TRANSFORM: u8 = 2;
const CONSTRAINT_PHYSICS: u8 = 3;
const CONSTRAINT_SLIDER: u8 = 4;
const ATTACHMENT_DEFORM: u8 = 0;
const ATTACHMENT_SEQUENCE: u8 = 1;
const PATH_POSITION: u8 = 0;
const PATH_SPACING: u8 = 1;
const PATH_MIX: u8 = 2;
const PHYSICS_RESET: u8 = 8;
const CURVE_STEPPED: u8 = 1;
const CURVE_BEZIER: u8 = 2;

/// Reads skeleton data from the binary export format.
pub struct SkeletonBinary<'a> {
    attachment_loader: &'a dyn AttachmentLoader,
    scale: f32,
    warnings: Vec<String>,
}

impl<'a> SkeletonBinary<'a> {
    pub fn new(attachment_loader: &'a dyn AttachmentLoader) -> Self {
        SkeletonBinary {
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
        let bytes = std::fs::read(path)?;
        let mut data = self.read_skeleton_data(&bytes)?;
        data.name = file_stem(path);
        Ok(data)
    }

    pub fn read_skeleton_data(&mut self, binary: &[u8]) -> Result<SkeletonData, LoadError> {
        if binary.is_empty() {
            return Err(LoadError::new("Unable to read skeleton file: ", ""));
        }
        let mut r = Reader {
            input: Input {
                data: binary,
                pos: 0,
                strings: Vec::new(),
            },
            loader: self.attachment_loader,
            scale: self.scale,
            b: Build::new(),
        };
        let result = r.read();
        self.warnings = std::mem::take(&mut r.b.warnings);
        result.map(|()| r.b.finish())
    }
}

/// A bounds-checked cursor over the binary data.
struct Input<'a> {
    data: &'a [u8],
    pos: usize,
    strings: Vec<SmolStr>,
}

impl Input<'_> {
    fn eof<T>(&self) -> Result<T, LoadError> {
        Err(LoadError(format!(
            "Unexpected end of skeleton data at byte {}",
            self.pos
        )))
    }

    fn remaining(&self) -> usize {
        self.data.len() - self.pos
    }

    fn u8(&mut self) -> Result<u8, LoadError> {
        match self.data.get(self.pos) {
            Some(&b) => {
                self.pos += 1;
                Ok(b)
            }
            None => self.eof(),
        }
    }

    fn i8(&mut self) -> Result<i8, LoadError> {
        Ok(self.u8()? as i8)
    }

    fn bool(&mut self) -> Result<bool, LoadError> {
        Ok(self.i8()? != 0)
    }

    /// A big-endian 32-bit integer.
    fn int(&mut self) -> Result<i32, LoadError> {
        let Some(bytes) = self.data.get(self.pos..self.pos + 4) else {
            return self.eof();
        };
        self.pos += 4;
        Ok(i32::from_be_bytes(bytes.try_into().expect("4 bytes")))
    }

    fn long(&mut self) -> Result<i64, LoadError> {
        let hi = self.int()? as u32 as u64;
        let lo = self.int()? as u32 as u64;
        Ok(((hi << 32) | lo) as i64)
    }

    fn float(&mut self) -> Result<f32, LoadError> {
        Ok(f32::from_bits(self.int()? as u32))
    }

    /// A variable-length integer with positive values optimized (negatives take 5 bytes).
    fn var(&mut self) -> Result<i32, LoadError> {
        let mut b = self.u8()?;
        let mut value = (b & 0x7f) as i32;
        if b & 0x80 != 0 {
            b = self.u8()?;
            value |= ((b & 0x7f) as i32) << 7;
            if b & 0x80 != 0 {
                b = self.u8()?;
                value |= ((b & 0x7f) as i32) << 14;
                if b & 0x80 != 0 {
                    b = self.u8()?;
                    value |= ((b & 0x7f) as i32) << 21;
                    if b & 0x80 != 0 {
                        value |= ((self.u8()? & 0x7f) as i32) << 28;
                    }
                }
            }
        }
        Ok(value)
    }

    /// A zigzag-encoded variable-length integer.
    fn signed_var(&mut self) -> Result<i32, LoadError> {
        let value = self.var()?;
        Ok(((value as u32 >> 1) as i32) ^ -(value & 1))
    }

    /// A variable-length index or count.
    fn index(&mut self) -> Result<usize, LoadError> {
        let v = self.var()?;
        usize::try_from(v).map_err(|_| LoadError(format!("Invalid index {v} at byte {}", self.pos)))
    }

    /// A count of items that each take at least one byte, validated against the remaining data.
    fn count(&mut self) -> Result<usize, LoadError> {
        let n = self.index()?;
        if n > self.remaining() {
            return Err(LoadError(format!("Invalid count {n} at byte {}", self.pos)));
        }
        Ok(n)
    }

    fn string(&mut self) -> Result<Option<SmolStr>, LoadError> {
        let length = self.index()?;
        if length == 0 {
            return Ok(None);
        }
        let end = self.pos + length - 1;
        let Some(bytes) = self.data.get(self.pos..end) else {
            return self.eof();
        };
        self.pos = end;
        Ok(Some(SmolStr::new(String::from_utf8_lossy(bytes))))
    }

    fn string_or_empty(&mut self) -> Result<SmolStr, LoadError> {
        Ok(self.string()?.unwrap_or_default())
    }

    /// A reference into the string table; 0 is no string.
    fn string_ref(&mut self) -> Result<Option<&str>, LoadError> {
        let index = self.index()?;
        if index == 0 {
            return Ok(None);
        }
        match self.strings.get(index - 1) {
            Some(s) => Ok(Some(s.as_str())),
            None => Err(LoadError(format!("Invalid string reference {index}"))),
        }
    }

    fn string_ref_owned(&mut self) -> Result<SmolStr, LoadError> {
        Ok(self
            .string_ref()?
            .map_or_else(SmolStr::default, SmolStr::new))
    }
}

struct Reader<'a, 'b> {
    input: Input<'b>,
    loader: &'a dyn AttachmentLoader,
    scale: f32,
    b: Build,
}

impl Reader<'_, '_> {
    fn read(&mut self) -> Result<(), LoadError> {
        let scale = self.scale;
        let hash = self.input.long()?;
        self.b.data.hash = if hash == 0 {
            SmolStr::default()
        } else {
            SmolStr::new(hash.to_string())
        };
        let version = self.input.string_or_empty()?;
        if !version.starts_with(VERSION) {
            return Err(LoadError(format!(
                "Skeleton version {version} does not match runtime version {VERSION}"
            )));
        }
        self.b.data.version = version;
        self.b.data.x = self.input.float()?;
        self.b.data.y = self.input.float()?;
        self.b.data.width = self.input.float()?;
        self.b.data.height = self.input.float()?;
        self.b.data.reference_scale = self.input.float()? * scale;
        let nonessential = self.input.bool()?;
        if nonessential {
            self.b.data.fps = self.input.float()?;
            self.b.data.images_path = self.input.string_or_empty()?;
            self.b.data.audio_path = self.input.string_or_empty()?;
        }

        let n = self.input.count()?;
        for _ in 0..n {
            let s = self.input.string_or_empty()?;
            self.input.strings.push(s);
        }

        self.bones(nonessential)?;
        self.slots(nonessential)?;
        self.constraints(nonessential)?;

        // Default skin, then the other skins.
        if let Some(skin) = self.skin(true, nonessential)? {
            self.b.default_skin = Some(self.b.skins.len());
            self.b.skins.push(skin);
        }
        let n = self.input.count()?;
        for _ in 0..n {
            let skin = self
                .skin(false, nonessential)?
                .expect("non-default skins are always present");
            self.b.skins.push(skin);
        }
        resolve_linked_meshes(&mut self.b, false)?;

        // Events.
        let n = self.input.count()?;
        for _ in 0..n {
            let mut d = EventData::new(self.input.string_or_empty()?);
            d.int_value = self.input.signed_var()?;
            d.float_value = self.input.float()?;
            d.string_value = self.input.string_or_empty()?;
            d.audio_path = self.input.string_or_empty()?;
            if !d.audio_path.is_empty() {
                d.volume = self.input.float()?;
                d.balance = self.input.float()?;
            }
            self.b.data.events.push(Arc::new(d));
        }

        // Animations.
        let n = self.input.count()?;
        for _ in 0..n {
            let name = self.input.string_or_empty()?;
            let animation = self
                .animation(&name, nonessential)
                .map_err(|e| LoadError(format!("Error reading animation: {name}: {e}")))?;
            self.b.data.animations.push(Arc::new(animation));
        }

        // Slider animations.
        for i in 0..self.b.data.constraints.len() {
            if matches!(self.b.data.constraints[i], ConstraintData::Slider(_)) {
                let index = self.input.index()?;
                let animation = self.b.data.animations.get(index).cloned().ok_or_else(|| {
                    LoadError::new("Invalid slider animation: ", &index.to_string())
                })?;
                if let ConstraintData::Slider(s) = &mut self.b.data.constraints[i] {
                    s.animation = Some(animation);
                }
            }
        }
        Ok(())
    }

    fn bones(&mut self, nonessential: bool) -> Result<(), LoadError> {
        let scale = self.scale;
        let n = self.input.count()?;
        for i in 0..n {
            let name = self.input.string_or_empty()?;
            let parent = if i == 0 {
                None
            } else {
                let p = self.input.index()?;
                if p >= i {
                    return Err(LoadError::new("Invalid parent bone: ", &p.to_string()));
                }
                Some(p)
            };
            let mut data = BoneData::new(i, name, parent);
            let setup = &mut data.setup_pose;
            setup.rotation = self.input.float()?;
            setup.x = self.input.float()? * scale;
            setup.y = self.input.float()? * scale;
            setup.scale_x = self.input.float()?;
            setup.scale_y = self.input.float()?;
            setup.shear_x = self.input.float()?;
            setup.shear_y = self.input.float()?;
            setup.inherit = Inherit::from_index(self.input.i8()? as usize);
            data.length = self.input.float()? * scale;
            data.skin_required = self.input.bool()?;
            if nonessential {
                data.color = Color::from_rgba8888(self.input.int()?);
                data.icon = self.input.string_or_empty()?;
                data.icon_size = self.input.float()?;
                data.icon_rotation = self.input.float()?;
                data.visible = self.input.bool()?;
            }
            self.b.push_bone(data);
        }
        Ok(())
    }

    fn bone_index(&mut self) -> Result<usize, LoadError> {
        let i = self.input.index()?;
        if i < self.b.data.bones.len() {
            Ok(i)
        } else {
            Err(LoadError::new("Invalid bone index: ", &i.to_string()))
        }
    }

    fn slot_index(&mut self) -> Result<usize, LoadError> {
        let i = self.input.index()?;
        if i < self.b.data.slots.len() {
            Ok(i)
        } else {
            Err(LoadError::new("Invalid slot index: ", &i.to_string()))
        }
    }

    fn slots(&mut self, nonessential: bool) -> Result<(), LoadError> {
        let n = self.input.count()?;
        for i in 0..n {
            let name = self.input.string_or_empty()?;
            let bone = self.bone_index()?;
            let mut data = SlotData::new(i, name, crate::bone::BoneId(bone));
            data.setup_pose.color = Color::from_rgba8888(self.input.int()?);
            let dark = self.input.int()?;
            if dark != -1 {
                data.setup_pose.dark_color = Color::from_rgb888(dark);
                data.setup_pose.has_dark_color = true;
            }
            data.attachment_name = self.input.string_ref_owned()?;
            data.blend_mode = BlendMode::from_index(self.input.index()?);
            if nonessential {
                data.visible = self.input.bool()?;
            }
            self.b.push_slot(data);
        }
        Ok(())
    }

    fn bone_indices(&mut self) -> Result<Vec<usize>, LoadError> {
        let n = self.input.count()?;
        (0..n).map(|_| self.bone_index()).collect()
    }

    fn property_from_index(index: u8) -> Result<BoneProperty, LoadError> {
        BoneProperty::from_index(index)
            .ok_or_else(|| LoadError::new("Invalid bone property: ", &index.to_string()))
    }

    fn constraints(&mut self, nonessential: bool) -> Result<(), LoadError> {
        let scale = self.scale;
        let n = self.input.count()?;
        for _ in 0..n {
            let name = self.input.string_or_empty()?;
            match self.input.u8()? {
                CONSTRAINT_IK => {
                    let mut d = IkConstraintData::new(name);
                    d.bones = self.bone_indices()?;
                    d.target = self.bone_index()?;
                    let flags = self.input.u8()?;
                    d.skin_required = flags & 1 != 0;
                    if flags & 2 != 0 {
                        d.scale_y_mode = ScaleYMode::from_index(self.input.u8()?);
                    }
                    let setup = &mut d.setup_pose;
                    setup.bend_direction = if flags & 4 != 0 { -1 } else { 1 };
                    setup.compress = flags & 8 != 0;
                    setup.stretch = flags & 16 != 0;
                    if flags & 32 != 0 {
                        setup.mix = if flags & 64 != 0 {
                            self.input.float()?
                        } else {
                            1.0
                        };
                    }
                    if flags & 128 != 0 {
                        setup.softness = self.input.float()? * scale;
                    }
                    self.b.data.constraints.push(ConstraintData::Ik(d));
                }
                CONSTRAINT_TRANSFORM => {
                    let mut d = TransformConstraintData::new(name);
                    d.bones = self.bone_indices()?;
                    d.source = self.bone_index()?;
                    let flags = self.input.u8()?;
                    d.skin_required = flags & 1 != 0;
                    d.local_source = flags & 2 != 0;
                    d.local_target = flags & 4 != 0;
                    d.additive = flags & 8 != 0;
                    d.clamp = flags & 16 != 0;
                    for _ in 0..(flags >> 5) {
                        let property = Self::property_from_index(self.input.u8()?)?;
                        let from_scale = if property.is_translation() {
                            scale
                        } else {
                            1.0
                        };
                        let mut from = FromProperty::new(property);
                        from.offset = self.input.float()? * from_scale;
                        for _ in 0..self.input.u8()? {
                            let to_property = Self::property_from_index(self.input.u8()?)?;
                            let to_scale = if to_property.is_translation() {
                                scale
                            } else {
                                1.0
                            };
                            let mut to = ToProperty::new(to_property);
                            to.offset = self.input.float()? * to_scale;
                            to.max = self.input.float()? * to_scale;
                            to.scale = self.input.float()? * to_scale / from_scale;
                            from.to.push(to);
                        }
                        d.properties.push(from);
                    }
                    let flags = self.input.u8()?;
                    if flags & 1 != 0 {
                        d.offsets[TransformConstraintData::ROTATION] = self.input.float()?;
                    }
                    if flags & 2 != 0 {
                        d.offsets[TransformConstraintData::X] = self.input.float()? * scale;
                    }
                    if flags & 4 != 0 {
                        d.offsets[TransformConstraintData::Y] = self.input.float()? * scale;
                    }
                    if flags & 8 != 0 {
                        d.offsets[TransformConstraintData::SCALE_X] = self.input.float()?;
                    }
                    if flags & 16 != 0 {
                        d.offsets[TransformConstraintData::SCALE_Y] = self.input.float()?;
                    }
                    if flags & 32 != 0 {
                        d.offsets[TransformConstraintData::SHEAR_Y] = self.input.float()?;
                    }
                    let flags = self.input.u8()?;
                    let setup = &mut d.setup_pose;
                    if flags & 1 != 0 {
                        setup.mix_rotate = self.input.float()?;
                    }
                    if flags & 2 != 0 {
                        setup.mix_x = self.input.float()?;
                    }
                    if flags & 4 != 0 {
                        setup.mix_y = self.input.float()?;
                    }
                    if flags & 8 != 0 {
                        setup.mix_scale_x = self.input.float()?;
                    }
                    if flags & 16 != 0 {
                        setup.mix_scale_y = self.input.float()?;
                    }
                    if flags & 32 != 0 {
                        setup.mix_shear_y = self.input.float()?;
                    }
                    self.b.data.constraints.push(ConstraintData::Transform(d));
                }
                CONSTRAINT_PATH => {
                    let mut d = PathConstraintData::new(name);
                    d.bones = self.bone_indices()?;
                    d.slot = self.slot_index()?;
                    let flags = self.input.u8()?;
                    d.skin_required = flags & 1 != 0;
                    d.position_mode = PositionMode::from_index((flags >> 1) & 1);
                    d.spacing_mode = SpacingMode::from_index((flags >> 2) & 3);
                    d.rotate_mode = RotateMode::from_index((flags >> 4) & 3);
                    if flags & 128 != 0 {
                        d.offset_rotation = self.input.float()?;
                    }
                    let setup = &mut d.setup_pose;
                    setup.position = self.input.float()?;
                    if d.position_mode == PositionMode::Fixed {
                        setup.position *= scale;
                    }
                    setup.spacing = self.input.float()?;
                    if matches!(d.spacing_mode, SpacingMode::Length | SpacingMode::Fixed) {
                        setup.spacing *= scale;
                    }
                    setup.mix_rotate = self.input.float()?;
                    setup.mix_x = self.input.float()?;
                    setup.mix_y = self.input.float()?;
                    self.b.data.constraints.push(ConstraintData::Path(d));
                }
                CONSTRAINT_PHYSICS => {
                    let mut d = PhysicsConstraintData::new(name);
                    d.bone = self.bone_index()?;
                    let flags = self.input.u8()?;
                    d.skin_required = flags & 1 != 0;
                    if flags & 2 != 0 {
                        d.x = self.input.float()?;
                    }
                    if flags & 4 != 0 {
                        d.y = self.input.float()?;
                    }
                    if flags & 8 != 0 {
                        d.rotate = self.input.float()?;
                    }
                    if flags & 16 != 0 {
                        let mut scale_x = self.input.float()?;
                        if scale_x < -2.0 {
                            d.scale_y_mode = ScaleYMode::Volume;
                            scale_x = -2.0 - scale_x;
                        } else if scale_x < 0.0 {
                            d.scale_y_mode = ScaleYMode::Uniform;
                            scale_x = -1.0 - scale_x;
                        }
                        d.scale_x = scale_x;
                    }
                    if flags & 32 != 0 {
                        d.shear_x = self.input.float()?;
                    }
                    d.limit = (if flags & 64 != 0 {
                        self.input.float()?
                    } else {
                        5000.0
                    }) * scale;
                    d.step = 1.0 / self.input.u8()? as f32;
                    let setup = &mut d.setup_pose;
                    setup.inertia = self.input.float()?;
                    setup.strength = self.input.float()?;
                    setup.damping = self.input.float()?;
                    setup.mass_inverse = if flags & 128 != 0 {
                        self.input.float()?
                    } else {
                        1.0
                    };
                    setup.wind = self.input.float()?;
                    setup.gravity = self.input.float()?;
                    let flags = self.input.u8()?;
                    d.inertia_global = flags & 1 != 0;
                    d.strength_global = flags & 2 != 0;
                    d.damping_global = flags & 4 != 0;
                    d.mass_global = flags & 8 != 0;
                    d.wind_global = flags & 16 != 0;
                    d.gravity_global = flags & 32 != 0;
                    d.mix_global = flags & 64 != 0;
                    d.setup_pose.mix = if flags & 128 != 0 {
                        self.input.float()?
                    } else {
                        1.0
                    };
                    self.b.data.constraints.push(ConstraintData::Physics(d));
                }
                CONSTRAINT_SLIDER => {
                    let mut d = SliderData::new(name);
                    let flags = self.input.u8()?;
                    d.skin_required = flags & 1 != 0;
                    d.loop_ = flags & 2 != 0;
                    d.additive = flags & 4 != 0;
                    if flags & 8 != 0 {
                        let value = self.input.float()?;
                        if nonessential && flags & 64 != 0 {
                            d.max = value;
                        } else {
                            d.setup_pose.time = value;
                        }
                    }
                    if flags & 16 != 0 {
                        d.setup_pose.mix = if flags & 32 != 0 {
                            self.input.float()?
                        } else {
                            1.0
                        };
                    }
                    if flags & 64 != 0 {
                        d.local = flags & 128 != 0;
                        d.bone = Some(self.bone_index()?);
                        let offset = self.input.float()?;
                        let property = BoneProperty::from_index(self.input.u8()?);
                        let property_scale = if property.is_some_and(|p| p.is_translation()) {
                            scale
                        } else {
                            1.0
                        };
                        d.property = property.map(|p| {
                            let mut from = FromProperty::new(p);
                            from.offset = offset * property_scale;
                            from
                        });
                        d.offset = self.input.float()?;
                        d.scale = self.input.float()? / property_scale;
                    }
                    self.b.data.constraints.push(ConstraintData::Slider(d));
                }
                other => {
                    return Err(LoadError::new(
                        "Invalid constraint type: ",
                        &other.to_string(),
                    ))
                }
            }
        }
        Ok(())
    }

    fn skin(&mut self, default_skin: bool, nonessential: bool) -> Result<Option<Skin>, LoadError> {
        let (mut skin, slot_count);
        if default_skin {
            slot_count = self.input.count()?;
            if slot_count == 0 {
                return Ok(None);
            }
            skin = Skin::new("default");
        } else {
            skin = Skin::new(self.input.string_or_empty()?);
            if nonessential {
                skin.color = Color::from_rgba8888(self.input.int()?);
            }
            skin.bones = self.bone_indices()?;
            let n = self.input.count()?;
            for _ in 0..n {
                let c = self.input.index()?;
                if c >= self.b.data.constraints.len() {
                    return Err(LoadError::new("Invalid skin constraint: ", &c.to_string()));
                }
                skin.constraints.push(c);
            }
            slot_count = self.input.count()?;
        }
        for _ in 0..slot_count {
            let slot_index = self.slot_index()?;
            let n = self.input.count()?;
            for _ in 0..n {
                let placeholder = self.input.string_ref_owned()?;
                let attachment = self
                    .attachment(&skin, slot_index, &placeholder, nonessential)?
                    .ok_or_else(|| LoadError::new("Error reading attachment: ", &placeholder))?;
                skin.set_attachment(slot_index, &placeholder, Arc::new(attachment));
            }
        }
        Ok(Some(skin))
    }

    fn sequence(&mut self, has_path_suffix: bool) -> Result<Sequence, LoadError> {
        if !has_path_suffix {
            return Ok(Sequence::new(1, false));
        }
        let mut s = Sequence::new(self.input.count()?, true);
        s.start = self.input.var()?;
        s.digits = self.input.var()?;
        s.setup_index = self.input.var()?;
        Ok(s)
    }

    fn float_array(&mut self, n: usize, scale: f32) -> Result<Vec<f32>, LoadError> {
        if n.checked_mul(4)
            .is_none_or(|bytes| bytes > self.input.remaining())
        {
            return self.input.eof();
        }
        (0..n)
            .map(|_| Ok(self.input.float()? * if scale == 1.0 { 1.0 } else { scale }))
            .collect()
    }

    fn u16_array(&mut self, n: usize) -> Result<Vec<u16>, LoadError> {
        if n > self.input.remaining() {
            return self.input.eof();
        }
        (0..n).map(|_| Ok(self.input.var()? as u16)).collect()
    }

    /// Reads vertices into `vertex`, returning the unweighted float count.
    fn vertices(
        &mut self,
        vertex: &mut VertexAttachment,
        weighted: bool,
    ) -> Result<usize, LoadError> {
        let scale = self.scale;
        let vertex_count = self.input.count()?;
        let vertices_length = vertex_count << 1;
        if !weighted {
            vertex.vertices = self.float_array(vertices_length, scale)?;
            vertex.world_vertices_length = vertices_length;
            return Ok(vertices_length);
        }
        let n = self.input.count()?;
        let mut bones = Vec::with_capacity(n);
        let mut vertices = Vec::with_capacity(n.saturating_sub(vertex_count) * 3);
        while bones.len() < n {
            let bone_count = self.input.index()?;
            bones.push(bone_count);
            for _ in 0..bone_count {
                bones.push(self.bone_index()?);
                vertices.push(self.input.float()? * scale);
                vertices.push(self.input.float()? * scale);
                vertices.push(self.input.float()?);
            }
        }
        vertex.bones = bones;
        vertex.vertices = vertices;
        vertex.world_vertices_length = vertices_length;
        Ok(vertices_length)
    }

    fn attachment(
        &mut self,
        skin: &Skin,
        slot_index: usize,
        placeholder: &str,
        nonessential: bool,
    ) -> Result<Option<Attachment>, LoadError> {
        let scale = self.scale;
        let flags = self.input.i8()? as u8;
        let name = if flags & 8 != 0 {
            self.input.string_ref_owned()?
        } else {
            SmolStr::new(placeholder)
        };
        let name = name.as_str();
        Ok(Some(match flags & 7 {
            0 => {
                let path = if flags & 16 != 0 {
                    self.input.string_ref_owned()?
                } else {
                    name.into()
                };
                let color = if flags & 32 != 0 {
                    self.input.int()?
                } else {
                    -1
                };
                let sequence = self.sequence(flags & 64 != 0)?;
                let rotation = if flags & 128 != 0 {
                    self.input.float()?
                } else {
                    0.0
                };
                let (x, y) = (self.input.float()?, self.input.float()?);
                let (scale_x, scale_y) = (self.input.float()?, self.input.float()?);
                let (width, height) = (self.input.float()?, self.input.float()?);
                let Some(mut a) =
                    self.loader
                        .new_region_attachment(skin, placeholder, name, &path, sequence)
                else {
                    return Ok(None);
                };
                a.path = path;
                a.x = x * scale;
                a.y = y * scale;
                a.scale_x = scale_x;
                a.scale_y = scale_y;
                a.rotation = rotation;
                a.width = width * scale;
                a.height = height * scale;
                a.color = Color::from_rgba8888(color);
                a.update_sequence();
                Attachment::Region(a)
            }
            1 => {
                let mut vertex = VertexAttachment::new(name);
                self.vertices(&mut vertex, flags & 16 != 0)?;
                let color = if nonessential { self.input.int()? } else { 0 };
                let Some(mut a) = self
                    .loader
                    .new_bounding_box_attachment(skin, placeholder, name)
                else {
                    return Ok(None);
                };
                keep_vertices(&mut a.vertex, vertex);
                if nonessential {
                    a.color = Color::from_rgba8888(color);
                }
                Attachment::BoundingBox(a)
            }
            2 => {
                let path = if flags & 16 != 0 {
                    self.input.string_ref_owned()?
                } else {
                    name.into()
                };
                let color = if flags & 32 != 0 {
                    self.input.int()?
                } else {
                    -1
                };
                let sequence = self.sequence(flags & 64 != 0)?;
                let hull_length = self.input.index()?;
                let mut vertex = VertexAttachment::new(name);
                let vertices_length = self.vertices(&mut vertex, flags & 128 != 0)?;
                let uvs = self.float_array(vertices_length, 1.0)?;
                let triangle_count = vertices_length
                    .saturating_sub(hull_length)
                    .saturating_sub(2)
                    * 3;
                let triangles = self.u16_array(triangle_count)?;
                let n = self.input.count()?;
                let timeline_slots: Vec<usize> = (0..n)
                    .map(|_| self.input.index())
                    .collect::<Result<_, _>>()?;
                let (mut edges, mut width, mut height) = (Vec::new(), 0.0, 0.0);
                if nonessential {
                    let n = self.input.count()?;
                    edges = self.u16_array(n)?;
                    width = self.input.float()?;
                    height = self.input.float()?;
                }
                let Some(mut mesh) =
                    self.loader
                        .new_mesh_attachment(skin, placeholder, name, &path, sequence)
                else {
                    return Ok(None);
                };
                mesh.path = path;
                mesh.color = Color::from_rgba8888(color);
                mesh.hull_length = hull_length << 1;
                keep_vertices(&mut mesh.vertex, vertex);
                mesh.region_uvs = uvs;
                mesh.triangles = triangles;
                mesh.vertex.base.timeline_slots.extend(timeline_slots);
                if nonessential {
                    mesh.edges = edges;
                    mesh.width = width * scale;
                    mesh.height = height * scale;
                }
                mesh.update_sequence();
                Attachment::Mesh(mesh)
            }
            3 => {
                let path = if flags & 16 != 0 {
                    self.input.string_ref_owned()?
                } else {
                    name.into()
                };
                let color = if flags & 32 != 0 {
                    self.input.int()?
                } else {
                    -1
                };
                let sequence = self.sequence(flags & 64 != 0)?;
                let inherit_timelines = flags & 128 != 0;
                let source_slot = self.input.index()?;
                let skin_index = self.input.index()?;
                let source = self.input.string_ref_owned()?;
                let (mut width, mut height) = (0.0, 0.0);
                if nonessential {
                    width = self.input.float()?;
                    height = self.input.float()?;
                }
                let Some(mut mesh) =
                    self.loader
                        .new_mesh_attachment(skin, placeholder, name, &path, sequence)
                else {
                    return Ok(None);
                };
                mesh.path = path;
                mesh.color = Color::from_rgba8888(color);
                if nonessential {
                    mesh.width = width * scale;
                    mesh.height = height * scale;
                }
                self.b.linked_meshes.push(LinkedMesh {
                    skin: self.b.skins.len(),
                    slot_index,
                    placeholder: placeholder.into(),
                    source_skin: SourceSkin::Index(skin_index),
                    source_slot,
                    source,
                    inherit_timelines,
                });
                Attachment::Mesh(mesh)
            }
            4 => {
                let (closed, constant_speed) = (flags & 16 != 0, flags & 32 != 0);
                let mut vertex = VertexAttachment::new(name);
                let vertices_length = self.vertices(&mut vertex, flags & 64 != 0)?;
                let lengths = self.float_array(vertices_length / 6, scale)?;
                let color = if nonessential { self.input.int()? } else { 0 };
                let Some(mut a) = self.loader.new_path_attachment(skin, placeholder, name) else {
                    return Ok(None);
                };
                a.closed = closed;
                a.constant_speed = constant_speed;
                keep_vertices(&mut a.vertex, vertex);
                a.lengths = lengths;
                if nonessential {
                    a.color = Color::from_rgba8888(color);
                }
                Attachment::Path(a)
            }
            5 => {
                let rotation = self.input.float()?;
                let (x, y) = (self.input.float()?, self.input.float()?);
                let color = if nonessential { self.input.int()? } else { 0 };
                let Some(mut a) = self.loader.new_point_attachment(skin, placeholder, name) else {
                    return Ok(None);
                };
                a.x = x * scale;
                a.y = y * scale;
                a.rotation = rotation;
                if nonessential {
                    a.color = Color::from_rgba8888(color);
                }
                Attachment::Point(a)
            }
            6 => {
                let end_slot = self.slot_index()?;
                let mut vertex = VertexAttachment::new(name);
                self.vertices(&mut vertex, flags & 16 != 0)?;
                let color = if nonessential { self.input.int()? } else { 0 };
                let Some(mut a) = self.loader.new_clipping_attachment(skin, placeholder, name)
                else {
                    return Ok(None);
                };
                a.end_slot = Some(end_slot);
                a.convex = flags & 32 != 0;
                a.inverse = flags & 64 != 0;
                keep_vertices(&mut a.vertex, vertex);
                if nonessential {
                    a.color = Color::from_rgba8888(color);
                }
                Attachment::Clipping(a)
            }
            other => {
                return Err(LoadError::new(
                    "Invalid attachment type: ",
                    &other.to_string(),
                ))
            }
        }))
    }

    /// Reads the keys of a curve timeline in the binary layout: time, values, then (for every
    /// frame but the last) the next key and the curve between them.
    fn curved<T: CurveSink, const N: usize>(
        &mut self,
        t: &mut T,
        frame_count: usize,
        read_values: impl Fn(&mut Input) -> Result<[f32; N], LoadError>,
        scales: [f32; N],
        set_frame: impl Fn(&mut T, usize, f32, [f32; N]),
    ) -> Result<(), LoadError> {
        let mut time = self.input.float()?;
        let mut values = read_values(&mut self.input)?;
        let mut bezier = 0;
        for frame in 0..frame_count {
            set_frame(t, frame, time, values);
            if frame + 1 == frame_count {
                break;
            }
            let time2 = self.input.float()?;
            let values2 = read_values(&mut self.input)?;
            match self.input.i8()? as u8 {
                CURVE_STEPPED => t.set_stepped(frame),
                CURVE_BEZIER => {
                    for k in 0..N {
                        self.bezier(
                            t, bezier, frame, k, time, time2, values[k], values2[k], scales[k],
                        )?;
                        bezier += 1;
                    }
                }
                _ => {}
            }
            time = time2;
            values = values2;
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn bezier(
        &mut self,
        t: &mut impl CurveSink,
        bezier: usize,
        frame: usize,
        value: usize,
        time1: f32,
        time2: f32,
        value1: f32,
        value2: f32,
        scale: f32,
    ) -> Result<(), LoadError> {
        let cx1 = self.input.float()?;
        let cy1 = self.input.float()?;
        let cx2 = self.input.float()?;
        let cy2 = self.input.float()?;
        t.set_bezier(
            bezier,
            frame,
            value,
            time1,
            value1,
            cx1,
            cy1 * scale,
            cx2,
            cy2 * scale,
            time2,
            value2,
        );
        Ok(())
    }

    fn animation(&mut self, name: &str, nonessential: bool) -> Result<Animation, LoadError> {
        let scale = self.scale;
        let mut timelines: Vec<Timeline> = Vec::new();
        let mut bones: Vec<usize> = Vec::new();
        let _timeline_count = self.input.index()?;

        let byte = |input: &mut Input| -> Result<f32, LoadError> { Ok(input.u8()? as f32 / 255.0) };

        // Slot timelines.
        let n = self.input.count()?;
        for _ in 0..n {
            let slot = self.slot_index()?;
            let nn = self.input.count()?;
            for _ in 0..nn {
                let kind = self.input.i8()? as u8;
                let frames = self.input.index()?;
                match kind {
                    SLOT_ATTACHMENT => {
                        let mut t = AttachmentTimeline::new(frames, slot);
                        for frame in 0..frames {
                            let time = self.input.float()?;
                            let attachment_name = self.input.string_ref_owned()?;
                            t.set_frame(frame, time, &attachment_name);
                        }
                        timelines.push(Timeline::Attachment(t));
                    }
                    SLOT_RGBA => {
                        let beziers = self.input.index()?;
                        let mut t = ColorTimeline::new(ColorKind::Rgba, frames, beziers, slot);
                        self.curved(
                            &mut t,
                            frames,
                            |i| Ok([byte(i)?, byte(i)?, byte(i)?, byte(i)?]),
                            [1.0; 4],
                            |t, f, time, v| t.set_frame(f, time, &v),
                        )?;
                        timelines.push(Timeline::Color(t));
                    }
                    SLOT_RGB => {
                        let beziers = self.input.index()?;
                        let mut t = ColorTimeline::new(ColorKind::Rgb, frames, beziers, slot);
                        self.curved(
                            &mut t,
                            frames,
                            |i| Ok([byte(i)?, byte(i)?, byte(i)?]),
                            [1.0; 3],
                            |t, f, time, v| t.set_frame(f, time, &v),
                        )?;
                        timelines.push(Timeline::Color(t));
                    }
                    SLOT_RGBA2 => {
                        let beziers = self.input.index()?;
                        let mut t = ColorTimeline::new(ColorKind::Rgba2, frames, beziers, slot);
                        self.curved(
                            &mut t,
                            frames,
                            |i| {
                                Ok([
                                    byte(i)?,
                                    byte(i)?,
                                    byte(i)?,
                                    byte(i)?,
                                    byte(i)?,
                                    byte(i)?,
                                    byte(i)?,
                                ])
                            },
                            [1.0; 7],
                            |t, f, time, v| t.set_frame(f, time, &v),
                        )?;
                        timelines.push(Timeline::Color(t));
                    }
                    SLOT_RGB2 => {
                        let beziers = self.input.index()?;
                        let mut t = ColorTimeline::new(ColorKind::Rgb2, frames, beziers, slot);
                        self.curved(
                            &mut t,
                            frames,
                            |i| Ok([byte(i)?, byte(i)?, byte(i)?, byte(i)?, byte(i)?, byte(i)?]),
                            [1.0; 6],
                            |t, f, time, v| t.set_frame(f, time, &v),
                        )?;
                        timelines.push(Timeline::Color(t));
                    }
                    SLOT_ALPHA => {
                        let beziers = self.input.index()?;
                        let mut t = AlphaTimeline::new(frames, beziers, slot);
                        self.curved(
                            &mut t,
                            frames,
                            |i| Ok([byte(i)?]),
                            [1.0],
                            |t, f, time, v| t.set_frame(f, time, v[0]),
                        )?;
                        timelines.push(Timeline::Alpha(t));
                    }
                    other => {
                        return Err(LoadError::new(
                            "Invalid slot timeline type: ",
                            &other.to_string(),
                        ))
                    }
                }
            }
        }

        // Bone timelines.
        let n = self.input.count()?;
        for _ in 0..n {
            let bone = self.bone_index()?;
            bones.push(bone);
            let nn = self.input.count()?;
            for _ in 0..nn {
                let kind = self.input.i8()? as u8;
                let frames = self.input.index()?;
                if kind == BONE_INHERIT {
                    let mut t = InheritTimeline::new(frames, bone);
                    for frame in 0..frames {
                        let time = self.input.float()?;
                        let inherit = Inherit::from_index(self.input.i8()? as usize);
                        t.set_frame(frame, time, inherit);
                    }
                    timelines.push(Timeline::Inherit(t));
                    continue;
                }
                let beziers = self.input.index()?;
                let one = |this: &mut Self,
                           kind: Bone1Kind,
                           value_scale: f32,
                           timelines: &mut Vec<Timeline>|
                 -> Result<(), LoadError> {
                    let mut t = BoneTimeline1::new(kind, frames, beziers, bone);
                    this.curved(
                        &mut t,
                        frames,
                        |i| Ok([i.float()? * value_scale]),
                        [value_scale],
                        |t, f, time, v| t.set_frame(f, time, v[0]),
                    )?;
                    timelines.push(Timeline::Bone1(t));
                    Ok(())
                };
                let two = |this: &mut Self,
                           kind: Bone2Kind,
                           value_scale: f32,
                           timelines: &mut Vec<Timeline>|
                 -> Result<(), LoadError> {
                    let mut t = BoneTimeline2::new(kind, frames, beziers, bone);
                    this.curved(
                        &mut t,
                        frames,
                        |i| Ok([i.float()? * value_scale, i.float()? * value_scale]),
                        [value_scale; 2],
                        |t, f, time, v| t.set_frame(f, time, v[0], v[1]),
                    )?;
                    timelines.push(Timeline::Bone2(t));
                    Ok(())
                };
                match kind {
                    BONE_ROTATE => one(self, Bone1Kind::Rotate, 1.0, &mut timelines)?,
                    BONE_TRANSLATE => two(self, Bone2Kind::Translate, scale, &mut timelines)?,
                    BONE_TRANSLATEX => one(self, Bone1Kind::X, scale, &mut timelines)?,
                    BONE_TRANSLATEY => one(self, Bone1Kind::Y, scale, &mut timelines)?,
                    BONE_SCALE => two(self, Bone2Kind::Scale, 1.0, &mut timelines)?,
                    BONE_SCALEX => one(self, Bone1Kind::ScaleX, 1.0, &mut timelines)?,
                    BONE_SCALEY => one(self, Bone1Kind::ScaleY, 1.0, &mut timelines)?,
                    BONE_SHEAR => two(self, Bone2Kind::Shear, 1.0, &mut timelines)?,
                    BONE_SHEARX => one(self, Bone1Kind::ShearX, 1.0, &mut timelines)?,
                    BONE_SHEARY => one(self, Bone1Kind::ShearY, 1.0, &mut timelines)?,
                    other => {
                        return Err(LoadError::new(
                            "Invalid bone timeline type: ",
                            &other.to_string(),
                        ))
                    }
                }
            }
        }

        self.constraint_timelines(&mut timelines)?;

        // Attachment timelines (deform and sequence).
        let n = self.input.count()?;
        for _ in 0..n {
            let skin_index = self.input.index()?;
            if skin_index >= self.b.skins.len() {
                return Err(LoadError::new(
                    "Invalid skin index: ",
                    &skin_index.to_string(),
                ));
            }
            let nn = self.input.count()?;
            for _ in 0..nn {
                let slot = self.slot_index()?;
                let nnn = self.input.count()?;
                for _ in 0..nnn {
                    let attachment_name = self.input.string_ref_owned()?;
                    let attachment = self.b.skins[skin_index]
                        .attachment(slot, &attachment_name)
                        .cloned()
                        .ok_or_else(|| {
                            LoadError::new("Timeline attachment not found: ", &attachment_name)
                        })?;
                    let kind = self.input.i8()? as u8;
                    let frames = self.input.index()?;
                    match kind {
                        ATTACHMENT_DEFORM => {
                            let beziers = self.input.index()?;
                            timelines.push(Timeline::Deform(self.deform_timeline(
                                frames,
                                beziers,
                                slot,
                                &attachment,
                            )?));
                        }
                        ATTACHMENT_SEQUENCE => {
                            let mut t = SequenceTimeline::new(frames, slot, attachment);
                            for frame in 0..frames {
                                let time = self.input.float()?;
                                let mode_and_index = self.input.int()?;
                                let delay = self.input.float()?;
                                t.set_frame(
                                    frame,
                                    time,
                                    SequenceMode::from_index((mode_and_index & 0xf) as u8),
                                    mode_and_index >> 4,
                                    delay,
                                );
                            }
                            timelines.push(Timeline::Sequence(t));
                        }
                        other => {
                            return Err(LoadError::new(
                                "Invalid attachment timeline type: ",
                                &other.to_string(),
                            ))
                        }
                    }
                }
            }
        }

        // Draw order.
        let slot_count = self.b.data.slots.len();
        let n = self.input.count()?;
        if n > 0 {
            let mut t = DrawOrderTimeline::new(n);
            for frame in 0..n {
                let time = self.input.float()?;
                let order = self.draw_order(slot_count)?;
                t.set_frame(
                    frame,
                    time,
                    if order.is_empty() { None } else { Some(&order) },
                );
            }
            timelines.push(Timeline::DrawOrder(t));
        }

        // Draw order folders.
        let folders = self.input.count()?;
        for _ in 0..folders {
            let folder_slot_count = self.input.count()?;
            let folder_slots: Vec<usize> = (0..folder_slot_count)
                .map(|_| self.slot_index())
                .collect::<Result<_, _>>()?;
            let keys = self.input.count()?;
            let mut t = DrawOrderFolderTimeline::new(keys, folder_slots, slot_count);
            for frame in 0..keys {
                let time = self.input.float()?;
                let order = self.draw_order(folder_slot_count)?;
                t.set_frame(
                    frame,
                    time,
                    if order.is_empty() { None } else { Some(&order) },
                );
            }
            timelines.push(Timeline::DrawOrderFolder(t));
        }

        // Events.
        let n = self.input.count()?;
        if n > 0 {
            let mut t = EventTimeline::new(n);
            for frame in 0..n {
                let time = self.input.float()?;
                let index = self.input.index()?;
                let data =
                    self.b.data.events.get(index).cloned().ok_or_else(|| {
                        LoadError::new("Invalid event index: ", &index.to_string())
                    })?;
                let mut e = Event::new(time, data.clone());
                e.int_value = self.input.signed_var()?;
                e.float_value = self.input.float()?;
                e.string_value = self
                    .input
                    .string()?
                    .unwrap_or_else(|| data.string_value.clone());
                if !data.audio_path.is_empty() {
                    e.volume = self.input.float()?;
                    e.balance = self.input.float()?;
                }
                t.set_frame(frame, Arc::new(e));
            }
            timelines.push(Timeline::Event(t));
        }

        let duration = timelines.iter().fold(0.0f32, |d, t| d.max(t.duration()));
        let mut animation = Animation::new_with_bones(name, timelines, duration, bones);
        if nonessential {
            animation.color = Color::from_rgba8888(self.input.int()?);
        }
        Ok(animation)
    }

    fn deform_timeline(
        &mut self,
        frames: usize,
        beziers: usize,
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
        let mut t = DeformTimeline::new(frames, beziers, slot, attachment.clone());
        let mut time = self.input.float()?;
        let mut bezier = 0;
        for frame in 0..frames {
            let end = self.input.index()?;
            let deform = if end == 0 {
                if weighted {
                    vec![0.0; deform_length]
                } else {
                    setup.clone()
                }
            } else {
                let mut deform = vec![0.0f32; deform_length];
                let start = self.input.index()?;
                if start + end > deform_length {
                    return Err(LoadError::new("Invalid deform range", ""));
                }
                for d in &mut deform[start..start + end] {
                    *d = self.input.float()? * scale;
                }
                if !weighted {
                    for (d, s) in deform.iter_mut().zip(setup) {
                        *d += s;
                    }
                }
                deform
            };
            t.set_frame(frame, time, &deform);
            if frame + 1 == frames {
                break;
            }
            let time2 = self.input.float()?;
            match self.input.i8()? as u8 {
                CURVE_STEPPED => t.curve.set_stepped(frame),
                CURVE_BEZIER => {
                    self.bezier(&mut t, bezier, frame, 0, time, time2, 0.0, 1.0, 1.0)?;
                    bezier += 1;
                }
                _ => {}
            }
            time = time2;
        }
        Ok(t)
    }

    fn draw_order(&mut self, slot_count: usize) -> Result<Vec<usize>, LoadError> {
        let n = self.input.count()?;
        let mut changes = Vec::with_capacity(n);
        for _ in 0..n {
            let slot = self.input.index()?;
            let offset = self.input.var()?;
            changes.push((slot, offset));
        }
        draw_order_from_changes(slot_count, &changes)
    }

    /// Reads the IK, transform, path, physics and slider timelines of an animation.
    fn constraint_timelines(&mut self, timelines: &mut Vec<Timeline>) -> Result<(), LoadError> {
        let scale = self.scale;

        // IK.
        let n = self.input.count()?;
        for _ in 0..n {
            let index = self.input.index()?;
            let frames = self.input.index()?;
            let beziers = self.input.index()?;
            let mut t = IkConstraintTimeline::new(frames, beziers, index);
            let mut flags = self.input.u8()?;
            let read_frame = |this: &mut Self, flags: u8| -> Result<(f32, f32, f32), LoadError> {
                let time = this.input.float()?;
                let mix = if flags & 1 != 0 {
                    if flags & 2 != 0 {
                        this.input.float()?
                    } else {
                        1.0
                    }
                } else {
                    0.0
                };
                let softness = if flags & 4 != 0 {
                    this.input.float()? * scale
                } else {
                    0.0
                };
                Ok((time, mix, softness))
            };
            let (mut time, mut mix, mut softness) = read_frame(self, flags)?;
            let mut bezier = 0;
            for frame in 0..frames {
                t.set_frame(
                    frame,
                    time,
                    mix,
                    softness,
                    if flags & 8 != 0 { 1 } else { -1 },
                    flags & 16 != 0,
                    flags & 32 != 0,
                );
                if frame + 1 == frames {
                    break;
                }
                flags = self.input.u8()?;
                let (time2, mix2, softness2) = read_frame(self, flags)?;
                if flags & 64 != 0 {
                    t.curve.set_stepped(frame);
                } else if flags & 128 != 0 {
                    self.bezier(&mut t.curve, bezier, frame, 0, time, time2, mix, mix2, 1.0)?;
                    self.bezier(
                        &mut t.curve,
                        bezier + 1,
                        frame,
                        1,
                        time,
                        time2,
                        softness,
                        softness2,
                        scale,
                    )?;
                    bezier += 2;
                }
                (time, mix, softness) = (time2, mix2, softness2);
            }
            timelines.push(Timeline::Ik(t));
        }

        // Transform.
        let n = self.input.count()?;
        for _ in 0..n {
            let index = self.input.index()?;
            let frames = self.input.index()?;
            let beziers = self.input.index()?;
            let mut t = TransformConstraintTimeline::new(frames, beziers, index);
            self.curved(
                &mut t,
                frames,
                |i| {
                    Ok([
                        i.float()?,
                        i.float()?,
                        i.float()?,
                        i.float()?,
                        i.float()?,
                        i.float()?,
                    ])
                },
                [1.0; 6],
                |t, f, time, v| t.set_frame(f, time, v[0], v[1], v[2], v[3], v[4], v[5]),
            )?;
            timelines.push(Timeline::Transform(t));
        }

        // Path.
        let n = self.input.count()?;
        for _ in 0..n {
            let index = self.input.index()?;
            let (position_scale, spacing_scale) = match self.b.data.constraints.get(index) {
                Some(ConstraintData::Path(d)) => (
                    if d.position_mode == PositionMode::Fixed {
                        scale
                    } else {
                        1.0
                    },
                    if matches!(d.spacing_mode, SpacingMode::Length | SpacingMode::Fixed) {
                        scale
                    } else {
                        1.0
                    },
                ),
                _ => {
                    return Err(LoadError::new(
                        "Invalid path constraint: ",
                        &index.to_string(),
                    ))
                }
            };
            let nn = self.input.count()?;
            for _ in 0..nn {
                let kind = self.input.i8()? as u8;
                let frames = self.input.index()?;
                let beziers = self.input.index()?;
                match kind {
                    PATH_POSITION | PATH_SPACING => {
                        let (k, value_scale) = if kind == PATH_POSITION {
                            (Constraint1Kind::PathPosition, position_scale)
                        } else {
                            (Constraint1Kind::PathSpacing, spacing_scale)
                        };
                        let mut t = ConstraintTimeline1::new(k, frames, beziers, Some(index));
                        self.curved(
                            &mut t,
                            frames,
                            |i| Ok([i.float()? * value_scale]),
                            [value_scale],
                            |t, f, time, v| t.set_frame(f, time, v[0]),
                        )?;
                        timelines.push(Timeline::Constraint1(t));
                    }
                    PATH_MIX => {
                        let mut t = PathConstraintMixTimeline::new(frames, beziers, index);
                        self.curved(
                            &mut t,
                            frames,
                            |i| Ok([i.float()?, i.float()?, i.float()?]),
                            [1.0; 3],
                            |t, f, time, v| t.set_frame(f, time, v[0], v[1], v[2]),
                        )?;
                        timelines.push(Timeline::PathMix(t));
                    }
                    other => {
                        return Err(LoadError::new(
                            "Invalid path constraint timeline type: ",
                            &other.to_string(),
                        ))
                    }
                }
            }
        }

        // Physics.
        let n = self.input.count()?;
        for _ in 0..n {
            let raw = self.input.var()?;
            let index = if raw == 0 {
                None
            } else {
                Some((raw - 1) as usize)
            };
            let nn = self.input.count()?;
            for _ in 0..nn {
                let kind = self.input.i8()? as u8;
                let frames = self.input.index()?;
                if kind == PHYSICS_RESET {
                    let mut t = PhysicsConstraintResetTimeline::new(frames, index);
                    for frame in 0..frames {
                        t.set_frame(frame, self.input.float()?);
                    }
                    timelines.push(Timeline::PhysicsReset(t));
                    continue;
                }
                let beziers = self.input.index()?;
                let k = match kind {
                    0 => Constraint1Kind::PhysicsInertia,
                    1 => Constraint1Kind::PhysicsStrength,
                    2 => Constraint1Kind::PhysicsDamping,
                    4 => Constraint1Kind::PhysicsMass,
                    5 => Constraint1Kind::PhysicsWind,
                    6 => Constraint1Kind::PhysicsGravity,
                    7 => Constraint1Kind::PhysicsMix,
                    other => {
                        return Err(LoadError::new(
                            "Invalid physics constraint timeline type: ",
                            &other.to_string(),
                        ))
                    }
                };
                let mut t = ConstraintTimeline1::new(k, frames, beziers, index);
                self.curved(
                    &mut t,
                    frames,
                    |i| Ok([i.float()?]),
                    [1.0],
                    |t, f, time, v| t.set_frame(f, time, v[0]),
                )?;
                timelines.push(Timeline::Constraint1(t));
            }
        }

        // Slider.
        let n = self.input.count()?;
        for _ in 0..n {
            let index = self.input.index()?;
            let nn = self.input.count()?;
            for _ in 0..nn {
                let kind = self.input.i8()? as u8;
                let frames = self.input.index()?;
                let beziers = self.input.index()?;
                let k = match kind {
                    0 => Constraint1Kind::SliderTime,
                    1 => Constraint1Kind::SliderMix,
                    other => {
                        return Err(LoadError::new(
                            "Invalid slider timeline type: ",
                            &other.to_string(),
                        ))
                    }
                };
                let mut t = ConstraintTimeline1::new(k, frames, beziers, Some(index));
                self.curved(
                    &mut t,
                    frames,
                    |i| Ok([i.float()?]),
                    [1.0],
                    |t, f, time, v| t.set_frame(f, time, v[0]),
                )?;
                timelines.push(Timeline::Constraint1(t));
            }
        }
        Ok(())
    }
}

/// Copies the vertex data read from the file into the attachment the loader created, keeping the
/// loader-assigned identity.
fn keep_vertices(target: &mut VertexAttachment, read: VertexAttachment) {
    target.bones = read.bones;
    target.vertices = read.vertices;
    target.world_vertices_length = read.world_vertices_length;
}
