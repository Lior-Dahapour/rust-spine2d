//! Files and data the user loads (the atlas, the skeleton file, the combined skeleton data, skins and
//! attachments, and the loader that lets Godot read Spine files as resources), and the resources
//! the world keeps.

pub mod animation_mix;
pub mod atlas;
pub mod attachment;
pub mod callbacks;
pub mod godot_nodes;
pub mod loader;
pub mod skeleton_data;
pub mod skeleton_file;
pub mod skin;
pub mod steps;

pub use atlas::SpineAtlasResource;
pub use skeleton_data::{LoadedSkeleton, SpineSkeletonDataResource};
pub use skeleton_file::SpineSkeletonFileResource;
