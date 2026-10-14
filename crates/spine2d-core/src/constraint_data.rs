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

//! Setup data for constraints. The constraint runtime that consumes it is ported separately.
//!
//! Pose defaults here are the reference constructor defaults (mostly zero); the loaders set the
//! real setup values.

use crate::animation::Animation;
use smol_str::SmolStr;
use std::sync::Arc;

/// How a constraint scales a bone's Y axis relative to X when stretching.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ScaleYMode {
    #[default]
    None,
    Uniform,
    Volume,
}

impl ScaleYMode {
    pub fn from_name(name: &str) -> ScaleYMode {
        match name {
            "uniform" => ScaleYMode::Uniform,
            "volume" => ScaleYMode::Volume,
            _ => ScaleYMode::None,
        }
    }

    pub fn from_index(index: u8) -> ScaleYMode {
        match index {
            1 => ScaleYMode::Uniform,
            2 => ScaleYMode::Volume,
            _ => ScaleYMode::None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum PositionMode {
    #[default]
    Fixed,
    Percent,
}

impl PositionMode {
    /// Unknown names are `Percent`, like the reference.
    pub fn from_name(name: &str) -> PositionMode {
        match name {
            "fixed" => PositionMode::Fixed,
            _ => PositionMode::Percent,
        }
    }

    pub fn from_index(index: u8) -> PositionMode {
        if index & 1 == 0 {
            PositionMode::Fixed
        } else {
            PositionMode::Percent
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SpacingMode {
    #[default]
    Length,
    Fixed,
    Percent,
    Proportional,
}

impl SpacingMode {
    pub fn from_name(name: &str) -> SpacingMode {
        match name {
            "fixed" => SpacingMode::Fixed,
            "percent" => SpacingMode::Percent,
            "proportional" => SpacingMode::Proportional,
            _ => SpacingMode::Length,
        }
    }

    pub fn from_index(index: u8) -> SpacingMode {
        match index & 3 {
            1 => SpacingMode::Fixed,
            2 => SpacingMode::Percent,
            3 => SpacingMode::Proportional,
            _ => SpacingMode::Length,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum RotateMode {
    #[default]
    Tangent,
    Chain,
    ChainScale,
}

impl RotateMode {
    pub fn from_name(name: &str) -> RotateMode {
        match name {
            "chain" => RotateMode::Chain,
            "chainScale" => RotateMode::ChainScale,
            _ => RotateMode::Tangent,
        }
    }

    pub fn from_index(index: u8) -> RotateMode {
        match index & 3 {
            1 => RotateMode::Chain,
            2 => RotateMode::ChainScale,
            _ => RotateMode::Tangent,
        }
    }
}

/// A bone property a transform constraint or slider reads (`from`) or writes (`to`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoneProperty {
    Rotate,
    X,
    Y,
    ScaleX,
    ScaleY,
    ShearY,
}

impl BoneProperty {
    pub fn from_name(name: &str) -> Option<BoneProperty> {
        Some(match name {
            "rotate" => BoneProperty::Rotate,
            "x" => BoneProperty::X,
            "y" => BoneProperty::Y,
            "scaleX" => BoneProperty::ScaleX,
            "scaleY" => BoneProperty::ScaleY,
            "shearY" => BoneProperty::ShearY,
            _ => return None,
        })
    }

    pub fn from_index(index: u8) -> Option<BoneProperty> {
        Some(match index {
            0 => BoneProperty::Rotate,
            1 => BoneProperty::X,
            2 => BoneProperty::Y,
            3 => BoneProperty::ScaleX,
            4 => BoneProperty::ScaleY,
            5 => BoneProperty::ShearY,
            _ => return None,
        })
    }

    /// True for the translation properties, which scale with the skeleton scale.
    pub fn is_translation(self) -> bool {
        matches!(self, BoneProperty::X | BoneProperty::Y)
    }
}

/// The source side of a transform constraint property mapping.
#[derive(Clone, Debug, PartialEq)]
pub struct FromProperty {
    pub property: BoneProperty,
    pub offset: f32,
    pub to: Vec<ToProperty>,
}

impl FromProperty {
    pub fn new(property: BoneProperty) -> Self {
        FromProperty {
            property,
            offset: 0.0,
            to: Vec::new(),
        }
    }
}

/// The target side of a transform constraint property mapping.
#[derive(Clone, Debug, PartialEq)]
pub struct ToProperty {
    pub property: BoneProperty,
    pub offset: f32,
    pub max: f32,
    pub scale: f32,
}

impl ToProperty {
    pub fn new(property: BoneProperty) -> Self {
        ToProperty {
            property,
            offset: 0.0,
            max: 0.0,
            scale: 1.0,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct IkConstraintPose {
    pub mix: f32,
    pub softness: f32,
    pub bend_direction: i32,
    pub compress: bool,
    pub stretch: bool,
}

#[derive(Clone, Debug)]
pub struct IkConstraintData {
    pub name: SmolStr,
    pub skin_required: bool,
    /// Indices of the constrained bones: one or two.
    pub bones: Vec<usize>,
    pub target: usize,
    pub scale_y_mode: ScaleYMode,
    pub setup_pose: IkConstraintPose,
}

impl IkConstraintData {
    pub fn new(name: impl Into<SmolStr>) -> Self {
        IkConstraintData {
            name: name.into(),
            skin_required: false,
            bones: Vec::new(),
            target: 0,
            scale_y_mode: ScaleYMode::None,
            setup_pose: IkConstraintPose::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TransformConstraintPose {
    pub mix_rotate: f32,
    pub mix_x: f32,
    pub mix_y: f32,
    pub mix_scale_x: f32,
    pub mix_scale_y: f32,
    pub mix_shear_y: f32,
}

#[derive(Clone, Debug)]
pub struct TransformConstraintData {
    pub name: SmolStr,
    pub skin_required: bool,
    pub bones: Vec<usize>,
    pub source: usize,
    /// Offsets added to the constrained bone: rotation, x, y, scale x, scale y, shear y.
    pub offsets: [f32; 6],
    pub local_source: bool,
    pub local_target: bool,
    pub additive: bool,
    pub clamp: bool,
    pub properties: Vec<FromProperty>,
    pub setup_pose: TransformConstraintPose,
}

impl TransformConstraintData {
    pub const ROTATION: usize = 0;
    pub const X: usize = 1;
    pub const Y: usize = 2;
    pub const SCALE_X: usize = 3;
    pub const SCALE_Y: usize = 4;
    pub const SHEAR_Y: usize = 5;

    pub fn new(name: impl Into<SmolStr>) -> Self {
        TransformConstraintData {
            name: name.into(),
            skin_required: false,
            bones: Vec::new(),
            source: 0,
            offsets: [0.0; 6],
            local_source: false,
            local_target: false,
            additive: false,
            clamp: false,
            properties: Vec::new(),
            setup_pose: TransformConstraintPose::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PathConstraintPose {
    pub position: f32,
    pub spacing: f32,
    pub mix_rotate: f32,
    pub mix_x: f32,
    pub mix_y: f32,
}

#[derive(Clone, Debug)]
pub struct PathConstraintData {
    pub name: SmolStr,
    pub skin_required: bool,
    pub bones: Vec<usize>,
    /// Index of the slot holding the path attachment.
    pub slot: usize,
    pub position_mode: PositionMode,
    pub spacing_mode: SpacingMode,
    pub rotate_mode: RotateMode,
    pub offset_rotation: f32,
    pub setup_pose: PathConstraintPose,
}

impl PathConstraintData {
    pub fn new(name: impl Into<SmolStr>) -> Self {
        PathConstraintData {
            name: name.into(),
            skin_required: false,
            bones: Vec::new(),
            slot: 0,
            position_mode: PositionMode::Fixed,
            spacing_mode: SpacingMode::Length,
            rotate_mode: RotateMode::Tangent,
            offset_rotation: 0.0,
            setup_pose: PathConstraintPose::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PhysicsConstraintPose {
    pub inertia: f32,
    pub strength: f32,
    pub damping: f32,
    pub mass_inverse: f32,
    pub wind: f32,
    pub gravity: f32,
    pub mix: f32,
}

#[derive(Clone, Debug)]
pub struct PhysicsConstraintData {
    pub name: SmolStr,
    pub skin_required: bool,
    pub bone: usize,
    pub x: f32,
    pub y: f32,
    pub rotate: f32,
    pub scale_x: f32,
    pub shear_x: f32,
    pub limit: f32,
    /// Fixed timestep in seconds.
    pub step: f32,
    pub scale_y_mode: ScaleYMode,
    pub inertia_global: bool,
    pub strength_global: bool,
    pub damping_global: bool,
    pub mass_global: bool,
    pub wind_global: bool,
    pub gravity_global: bool,
    pub mix_global: bool,
    pub setup_pose: PhysicsConstraintPose,
}

impl PhysicsConstraintData {
    pub fn new(name: impl Into<SmolStr>) -> Self {
        PhysicsConstraintData {
            name: name.into(),
            skin_required: false,
            bone: 0,
            x: 0.0,
            y: 0.0,
            rotate: 0.0,
            scale_x: 0.0,
            shear_x: 0.0,
            limit: 0.0,
            step: 0.0,
            scale_y_mode: ScaleYMode::None,
            inertia_global: false,
            strength_global: false,
            damping_global: false,
            mass_global: false,
            wind_global: false,
            gravity_global: false,
            mix_global: false,
            setup_pose: PhysicsConstraintPose::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SliderPose {
    pub time: f32,
    pub mix: f32,
}

#[derive(Clone, Debug)]
pub struct SliderData {
    pub name: SmolStr,
    pub skin_required: bool,
    /// The animation the slider drives.
    pub animation: Option<Arc<Animation>>,
    pub additive: bool,
    pub loop_: bool,
    /// Index of the bone whose `property` drives the slider, if any.
    pub bone: Option<usize>,
    pub property: Option<FromProperty>,
    pub offset: f32,
    pub scale: f32,
    pub max: f32,
    pub local: bool,
    pub setup_pose: SliderPose,
}

impl SliderData {
    pub fn new(name: impl Into<SmolStr>) -> Self {
        SliderData {
            name: name.into(),
            skin_required: false,
            animation: None,
            additive: false,
            loop_: false,
            bone: None,
            property: None,
            offset: 0.0,
            scale: 0.0,
            max: 0.0,
            local: false,
            setup_pose: SliderPose::default(),
        }
    }
}

/// Setup data for any constraint, in the order constraints are applied.
#[derive(Clone, Debug)]
pub enum ConstraintData {
    Ik(IkConstraintData),
    Transform(TransformConstraintData),
    Path(PathConstraintData),
    Physics(PhysicsConstraintData),
    Slider(SliderData),
}

impl ConstraintData {
    pub fn name(&self) -> &str {
        match self {
            ConstraintData::Ik(d) => &d.name,
            ConstraintData::Transform(d) => &d.name,
            ConstraintData::Path(d) => &d.name,
            ConstraintData::Physics(d) => &d.name,
            ConstraintData::Slider(d) => &d.name,
        }
    }

    pub fn skin_required(&self) -> bool {
        match self {
            ConstraintData::Ik(d) => d.skin_required,
            ConstraintData::Transform(d) => d.skin_required,
            ConstraintData::Path(d) => d.skin_required,
            ConstraintData::Physics(d) => d.skin_required,
            ConstraintData::Slider(d) => d.skin_required,
        }
    }
}
