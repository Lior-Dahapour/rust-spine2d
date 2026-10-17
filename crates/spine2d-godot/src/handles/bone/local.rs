//! `SpineBoneLocal`: the local transform values of a bone, held by the script.
//!
//! The official plugin registers this class, but none of its methods returns one, so a script can
//! only make one with `SpineBoneLocal.new()`. It is a plain set of values; the poses of real bones
//! are `SpineBonePose`.

use godot::classes::RefCounted;
use godot::global::godot_warn;
use godot::prelude::*;
use spine2d_core::BoneLocal;

use super::pose::inherit_from;

/// A bone's local position, rotation, scale, shear and inheritance, as plain values.
#[derive(GodotClass)]
#[class(init, base=RefCounted, tool)]
pub struct SpineBoneLocal {
    base: Base<RefCounted>,
    local: BoneLocal,
}

#[godot_api]
impl SpineBoneLocal {
    #[func]
    fn get_x(&self) -> f32 {
        self.local.x
    }

    #[func]
    fn set_x(&mut self, value: f32) {
        self.local.x = value;
    }

    #[func]
    fn get_y(&self) -> f32 {
        self.local.y
    }

    #[func]
    fn set_y(&mut self, value: f32) {
        self.local.y = value;
    }

    /// The rotation in degrees, counter clockwise.
    #[func]
    fn get_rotation(&self) -> f32 {
        self.local.rotation
    }

    #[func]
    fn set_rotation(&mut self, value: f32) {
        self.local.rotation = value;
    }

    #[func]
    fn get_scale_x(&self) -> f32 {
        self.local.scale_x
    }

    #[func]
    fn set_scale_x(&mut self, value: f32) {
        self.local.scale_x = value;
    }

    #[func]
    fn get_scale_y(&self) -> f32 {
        self.local.scale_y
    }

    #[func]
    fn set_scale_y(&mut self, value: f32) {
        self.local.scale_y = value;
    }

    #[func]
    fn get_shear_x(&self) -> f32 {
        self.local.shear_x
    }

    #[func]
    fn set_shear_x(&mut self, value: f32) {
        self.local.shear_x = value;
    }

    #[func]
    fn get_shear_y(&self) -> f32 {
        self.local.shear_y
    }

    #[func]
    fn set_shear_y(&mut self, value: f32) {
        self.local.shear_y = value;
    }

    /// How the bone inherits its parent's transform: one of `SpineConstant.Inherit_*`.
    #[func]
    fn get_inherit(&self) -> i32 {
        self.local.inherit as i32
    }

    #[func]
    fn set_inherit(&mut self, value: i32) {
        let Some(inherit) = inherit_from(value) else {
            godot_warn!("Spine: {value} is not an inherit mode (0 to 4)");
            return;
        };
        self.local.inherit = inherit;
    }

    #[func]
    fn set_position(&mut self, x: f32, y: f32) {
        self.local.x = x;
        self.local.y = y;
    }

    #[func]
    fn set_scale(&mut self, scale_x: f32, scale_y: f32) {
        self.local.scale_x = scale_x;
        self.local.scale_y = scale_y;
    }

    #[func]
    fn set_scale_uniform(&mut self, scale: f32) {
        self.local.scale_x = scale;
        self.local.scale_y = scale;
    }
}
