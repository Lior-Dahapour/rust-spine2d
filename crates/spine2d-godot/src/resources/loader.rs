//! Lets Godot load Spine files as resources, so an `.atlas`, `.skel` or `.spine-json` file can be
//! dragged straight into an inspector slot that expects a [`SpineAtlasResource`] or a
//! [`SpineSkeletonFileResource`]. Exports include the files and their page textures.

use crate::resources::{SpineAtlasResource, SpineSkeletonFileResource};
use godot::classes::{FileAccess, IResourceFormatLoader, ResourceFormatLoader};
use godot::prelude::*;
use spine2d_core::Atlas;

const ATLAS: &str = "atlas";
const SKELETON_EXTENSIONS: [&str; 2] = ["skel", "spine-json"];

/// Loads `.atlas`, `.skel` and `.spine-json` files as Spine resources.
#[derive(GodotClass)]
#[class(base=ResourceFormatLoader, init, tool)]
pub struct SpineResourceLoader {
    base: Base<ResourceFormatLoader>,
}

/// The extension of `path`, lowercased.
fn extension(path: &str) -> String {
    path.rsplit('.').next().unwrap_or("").to_lowercase()
}

#[godot_api]
impl IResourceFormatLoader for SpineResourceLoader {
    fn get_recognized_extensions(&self) -> PackedStringArray {
        let mut extensions = PackedStringArray::new();
        extensions.push(ATLAS);
        for extension in SKELETON_EXTENSIONS {
            extensions.push(extension);
        }
        extensions
    }

    fn handles_type(&self, type_: StringName) -> bool {
        let type_ = type_.to_string();
        type_ == "Resource" || type_ == "SpineAtlasResource" || type_ == "SpineSkeletonFileResource"
    }

    fn get_resource_type(&self, path: GString) -> GString {
        match extension(&path.to_string()).as_str() {
            ATLAS => GString::from("SpineAtlasResource"),
            e if SKELETON_EXTENSIONS.contains(&e) => GString::from("SpineSkeletonFileResource"),
            _ => GString::new(),
        }
    }

    fn get_dependencies(&self, path: GString, _add_types: bool) -> PackedStringArray {
        let mut dependencies = PackedStringArray::new();
        let path = path.to_string();
        if extension(&path) != ATLAS || !FileAccess::file_exists(&path) {
            return dependencies;
        }
        let text = FileAccess::get_file_as_string(&path).to_string();
        let dir = path.rfind('/').map_or("", |i| &path[..i]);
        for page in Atlas::parse(&text, dir, None).pages() {
            dependencies.push(page.texture_path.as_str());
        }
        dependencies
    }

    fn load(
        &self,
        path: GString,
        _original_path: GString,
        _use_sub_threads: bool,
        _cache_mode: i32,
    ) -> Variant {
        match extension(&path.to_string()).as_str() {
            ATLAS => {
                let mut atlas = SpineAtlasResource::new_gd();
                atlas.bind_mut().set_source_file(path);
                atlas.to_variant()
            }
            e if SKELETON_EXTENSIONS.contains(&e) => {
                let mut file = SpineSkeletonFileResource::new_gd();
                file.bind_mut().set_source_file(path);
                file.to_variant()
            }
            _ => Variant::nil(),
        }
    }
}
