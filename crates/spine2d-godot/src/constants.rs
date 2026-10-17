//! `SpineConstant`: the names of the numbers that scripts pass to and get from the plugin, as in
//! the official runtime. The class is never instantiated; scripts write `SpineConstant.Inherit_Normal`
//! instead of `0`. The names and values are the official ones, and the Rust code matches on these
//! constants, so a number is written down once.
//!
//! The official `Property` group (the timeline property kinds) is left out until the animation
//! classes exist.

use godot::classes::Object;
use godot::prelude::*;

/// Named integer constants for the enums of the official API.
#[derive(GodotClass)]
#[class(base=Object, no_init, tool)]
pub struct SpineConstant {
    base: Base<Object>,
}

// The official names have an underscore between the enum and the value, which Rust would write in
// capitals; scripts must find them under the official names.
#[allow(non_upper_case_globals)]
#[godot_api]
impl SpineConstant {
    /// How a bone inherits its parent's transform.
    #[constant]
    pub const Inherit_Normal: i64 = 0;
    #[constant]
    pub const Inherit_OnlyTranslation: i64 = 1;
    #[constant]
    pub const Inherit_NoRotationOrReflection: i64 = 2;
    #[constant]
    pub const Inherit_NoScale: i64 = 3;
    #[constant]
    pub const Inherit_NoScaleOrReflection: i64 = 4;

    /// How a path constraint measures the position along the path.
    #[constant]
    pub const PositionMode_Fixed: i64 = 0;
    #[constant]
    pub const PositionMode_Percent: i64 = 1;

    /// How a path constraint measures the spacing between bones. The official runtime stops at
    /// `Percent`; `Proportional` is the fourth mode of Spine 4.3.
    #[constant]
    pub const SpacingMode_Length: i64 = 0;
    #[constant]
    pub const SpacingMode_Fixed: i64 = 1;
    #[constant]
    pub const SpacingMode_Percent: i64 = 2;
    #[constant]
    pub const SpacingMode_Proportional: i64 = 3;

    /// How a path constraint rotates the bones along the path.
    #[constant]
    pub const RotateMode_Tangent: i64 = 0;
    #[constant]
    pub const RotateMode_Chain: i64 = 1;
    #[constant]
    pub const RotateMode_ChainScale: i64 = 2;

    /// How a slot blends with what is drawn below it.
    #[constant]
    pub const BlendMode_Normal: i64 = 0;
    #[constant]
    pub const BlendMode_Additive: i64 = 1;
    #[constant]
    pub const BlendMode_Multiply: i64 = 2;
    #[constant]
    pub const BlendMode_Screen: i64 = 3;

    /// How a constraint scales the Y axis of a bone when it stretches it.
    #[constant]
    pub const ScaleYMode_None: i64 = 0;
    #[constant]
    pub const ScaleYMode_Uniform: i64 = 1;
    #[constant]
    pub const ScaleYMode_Volume: i64 = 2;

    /// When a sprite updates its skeleton.
    #[constant]
    pub const UpdateMode_Process: i64 = 0;
    #[constant]
    pub const UpdateMode_Physics: i64 = 1;
    #[constant]
    pub const UpdateMode_Manual: i64 = 2;

    /// Whether a bone node follows its bone or drives it.
    #[constant]
    pub const BoneMode_Follow: i64 = 0;
    #[constant]
    pub const BoneMode_Drive: i64 = 1;

    /// What a world transform update does with physics constraints.
    #[constant]
    pub const Physics_None: i64 = 0;
    #[constant]
    pub const Physics_Reset: i64 = 1;
    #[constant]
    pub const Physics_Update: i64 = 2;
    #[constant]
    pub const Physics_Pose: i64 = 3;

    /// How a track entry eases between animations.
    #[constant]
    pub const MixInterpolation_Linear: i64 = 0;
    #[constant]
    pub const MixInterpolation_Smooth: i64 = 1;
    #[constant]
    pub const MixInterpolation_SlowFast: i64 = 2;
    #[constant]
    pub const MixInterpolation_FastSlow: i64 = 3;
    #[constant]
    pub const MixInterpolation_Circle: i64 = 4;
}

#[cfg(test)]
mod tests {
    use spine2d_core::{
        BlendMode, Inherit, Physics, PositionMode, RotateMode, ScaleYMode, SpacingMode,
    };

    use super::SpineConstant;
    use crate::nodes::sprite::UpdateMode;

    // The Rust code turns these enums into script numbers with `as`, so their order has to stay the
    // one the constants name.
    #[test]
    fn core_enums_are_numbered_like_the_constants() {
        assert_eq!(Inherit::Normal as i64, SpineConstant::Inherit_Normal);
        assert_eq!(
            Inherit::OnlyTranslation as i64,
            SpineConstant::Inherit_OnlyTranslation
        );
        assert_eq!(
            Inherit::NoRotationOrReflection as i64,
            SpineConstant::Inherit_NoRotationOrReflection
        );
        assert_eq!(Inherit::NoScale as i64, SpineConstant::Inherit_NoScale);
        assert_eq!(
            Inherit::NoScaleOrReflection as i64,
            SpineConstant::Inherit_NoScaleOrReflection
        );
        assert_eq!(
            PositionMode::Fixed as i64,
            SpineConstant::PositionMode_Fixed
        );
        assert_eq!(
            PositionMode::Percent as i64,
            SpineConstant::PositionMode_Percent
        );
        assert_eq!(
            SpacingMode::Length as i64,
            SpineConstant::SpacingMode_Length
        );
        assert_eq!(SpacingMode::Fixed as i64, SpineConstant::SpacingMode_Fixed);
        assert_eq!(
            SpacingMode::Percent as i64,
            SpineConstant::SpacingMode_Percent
        );
        assert_eq!(
            SpacingMode::Proportional as i64,
            SpineConstant::SpacingMode_Proportional
        );
        assert_eq!(
            RotateMode::Tangent as i64,
            SpineConstant::RotateMode_Tangent
        );
        assert_eq!(RotateMode::Chain as i64, SpineConstant::RotateMode_Chain);
        assert_eq!(
            RotateMode::ChainScale as i64,
            SpineConstant::RotateMode_ChainScale
        );
        assert_eq!(ScaleYMode::None as i64, SpineConstant::ScaleYMode_None);
        assert_eq!(
            ScaleYMode::Uniform as i64,
            SpineConstant::ScaleYMode_Uniform
        );
        assert_eq!(ScaleYMode::Volume as i64, SpineConstant::ScaleYMode_Volume);
        assert_eq!(BlendMode::Normal as i64, SpineConstant::BlendMode_Normal);
        assert_eq!(
            BlendMode::Additive as i64,
            SpineConstant::BlendMode_Additive
        );
        assert_eq!(
            BlendMode::Multiply as i64,
            SpineConstant::BlendMode_Multiply
        );
        assert_eq!(BlendMode::Screen as i64, SpineConstant::BlendMode_Screen);
        assert_eq!(Physics::None as i64, SpineConstant::Physics_None);
        assert_eq!(Physics::Reset as i64, SpineConstant::Physics_Reset);
        assert_eq!(Physics::Update as i64, SpineConstant::Physics_Update);
        assert_eq!(Physics::Pose as i64, SpineConstant::Physics_Pose);
        assert_eq!(
            UpdateMode::Process as i64,
            SpineConstant::UpdateMode_Process
        );
        assert_eq!(
            UpdateMode::Physics as i64,
            SpineConstant::UpdateMode_Physics
        );
        assert_eq!(UpdateMode::Manual as i64, SpineConstant::UpdateMode_Manual);
    }
}
