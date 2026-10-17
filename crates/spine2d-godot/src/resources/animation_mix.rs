//! `SpineAnimationMix`: the crossfade time between two animations, as a resource.

use godot::classes::Resource;
use godot::prelude::*;

/// How long to crossfade when switching from the animation `from` to the animation `to`. The
/// skeleton data resource lists them in its `animation_mixes`.
#[derive(GodotClass)]
#[class(init, base=Resource, tool)]
pub struct SpineAnimationMix {
    base: Base<Resource>,
    /// The animation switched from.
    #[export]
    from: GString,
    /// The animation switched to.
    #[export]
    to: GString,
    /// The crossfade time in seconds.
    #[export]
    mix: f32,
}

impl SpineAnimationMix {
    /// A resource for the mix from `from` to `to`.
    pub fn create(from: &str, to: &str, mix: f32) -> Gd<Self> {
        Gd::from_init_fn(|base| SpineAnimationMix {
            base,
            from: GString::from(from),
            to: GString::from(to),
            mix,
        })
    }

    /// The names and the time of this mix.
    pub fn values(&self) -> (String, String, f32) {
        (self.from.to_string(), self.to.to_string(), self.mix)
    }
}
