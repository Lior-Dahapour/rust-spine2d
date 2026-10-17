//! Script-facing objects that point at one skeleton in the world. Each holds the skeleton's entity
//! plus an index, so creating one allocates no skeleton data, and a stale handle (a disposed track
//! entry, a freed sprite) simply stops working.

use godot::global::godot_warn;

mod animation;
mod animation_state;
mod bone;
pub(crate) mod constraints;
mod event;
pub(crate) mod skeleton;
mod slot;
mod slot_data;
mod slot_pose;
mod track_entry;

pub use animation::SpineAnimation;
pub use animation_state::SpineAnimationState;
pub use bone::{SpineBone, SpineBoneData};
pub use event::{SpineEvent, SpineEventData};
pub use skeleton::SpineSkeleton;
pub use slot::SpineSlot;
pub use slot_data::SpineSlotData;
pub use track_entry::SpineTrackEntry;

/// Which copy of a pose a pose handle reads and writes: the one of a bone or a constraint.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PoseKind {
    /// The pose animations and scripts set.
    Pose,
    /// The pose after constraints changed it; the one the skeleton is drawn with.
    Applied,
    /// The pose from the skeleton data. It is shared by every sprite, so changing it changes it for
    /// all of them, the next time they apply their setup pose.
    Setup,
}

/// Logs the warning for a method the plugin has not implemented yet, so a script that calls it
/// sees why nothing happened.
pub fn warn_not_implemented(method: &str) {
    godot_warn!("Spine: {method} is not implemented yet");
}
