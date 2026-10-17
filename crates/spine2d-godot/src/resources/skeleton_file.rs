//! `SpineSkeletonFileResource`: the bytes of a skeleton file, stored inside the resource.

use godot::classes::{FileAccess, Resource};
use godot::global::Error;
use godot::prelude::*;

/// A Spine skeleton file (`.json` or `.skel`).
#[derive(GodotClass)]
#[class(base=Resource, tool)]
pub struct SpineSkeletonFileResource {
    base: Base<Resource>,
    /// Path of the skeleton file. Setting it (in the editor) reloads the stored bytes.
    #[export(file = "*.json,*.skel,*.spine-json")]
    #[var(set = set_source_file)]
    source_file: GString,
    /// The file contents, stored in the resource.
    #[export]
    bytes: PackedByteArray,
    /// True for the binary `.skel` format.
    #[export]
    is_binary: bool,
}

#[godot_api]
impl IResource for SpineSkeletonFileResource {
    fn init(base: Base<Resource>) -> Self {
        SpineSkeletonFileResource {
            base,
            source_file: GString::new(),
            bytes: PackedByteArray::new(),
            is_binary: false,
        }
    }
}

#[godot_api]
impl SpineSkeletonFileResource {
    #[func]
    pub fn set_source_file(&mut self, path: GString) {
        self.source_file = path.clone();
        if path.is_empty() || !FileAccess::file_exists(&path) {
            return;
        }
        self.bytes = FileAccess::get_file_as_bytes(&path);
        self.is_binary = path.to_string().ends_with(".skel");
        self.base_mut().emit_changed();
    }

    /// Reads the skeleton file at `path`. Returns an error code: `OK`, or `ERR_FILE_NOT_FOUND` if
    /// there is no file.
    #[func]
    fn load_from_file(&mut self, path: GString) -> Error {
        if path.is_empty() || !FileAccess::file_exists(&path) {
            return Error::ERR_FILE_NOT_FOUND;
        }
        self.set_source_file(path);
        Error::OK
    }
}

impl SpineSkeletonFileResource {
    /// The file contents.
    pub fn bytes(&self) -> &PackedByteArray {
        &self.bytes
    }

    /// True for the binary `.skel` format.
    pub fn is_binary(&self) -> bool {
        self.is_binary
    }
}
