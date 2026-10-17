//! `SpineAtlasResource`: a Spine atlas, the `.atlas` text with its page textures.
//!
//! The resource stores its source inside the saved resource, so exported games do not need the
//! original files. In the editor, setting the source file path reads the file and refreshes the
//! stored copy.

use godot::classes::{FileAccess, Image, ImageTexture, Resource, Texture2D};
use godot::global::{godot_error, Error};
use godot::prelude::*;
use godot::tools::try_load;
use spine2d_core::{Atlas, AtlasPage, TextureLoader};

/// Gives atlas pages the textures stored in a [`SpineAtlasResource`], by page order.
struct StoredTextures<'a> {
    textures: &'a [Gd<Texture2D>],
}

impl TextureLoader for StoredTextures<'_> {
    fn load(&mut self, page: &mut AtlasPage, _path: &str) {
        if let Some(texture) = self.textures.get(page.index) {
            page.width = texture.get_width();
            page.height = texture.get_height();
            page.texture = texture.get_rid().to_u64() as usize;
        }
    }

    fn unload(&mut self, _texture: usize) {}
}

/// The directory part of a Godot path, without the trailing slash.
fn parent_dir(path: &str) -> &str {
    path.rfind('/').map_or("", |i| &path[..i])
}

/// The texture at `path`: an imported texture resource, or else an image file read as it is, for
/// pages that were not imported (for example files next to an exported game).
fn load_page_texture(path: &str) -> Option<Gd<Texture2D>> {
    if let Ok(texture) = try_load::<Texture2D>(path) {
        return Some(texture);
    }
    let image = Image::load_from_file(path)?;
    ImageTexture::create_from_image(&image).map(|texture| texture.upcast())
}

/// A Spine atlas: the `.atlas` text and the page textures it names.
#[derive(GodotClass)]
#[class(base=Resource, tool)]
pub struct SpineAtlasResource {
    base: Base<Resource>,
    /// Path of the `.atlas` file. Setting it (in the editor) reloads the text and page textures.
    #[export(file = "*.atlas")]
    #[var(set = set_source_file)]
    source_file: GString,
    /// The atlas text, stored in the resource.
    #[export]
    atlas_text: GString,
    /// One texture per atlas page, in page order.
    #[export]
    textures: Array<Gd<Texture2D>>,
}

#[godot_api]
impl IResource for SpineAtlasResource {
    fn init(base: Base<Resource>) -> Self {
        SpineAtlasResource {
            base,
            source_file: GString::new(),
            atlas_text: GString::new(),
            textures: Array::new(),
        }
    }
}

#[godot_api]
impl SpineAtlasResource {
    #[func]
    pub fn set_source_file(&mut self, path: GString) {
        self.source_file = path.clone();
        if path.is_empty() || !FileAccess::file_exists(&path) {
            return;
        }
        let text = FileAccess::get_file_as_string(&path);
        let dir = parent_dir(&path.to_string()).to_string();
        let atlas = Atlas::parse(&text.to_string(), &dir, None);
        let mut textures = Array::new();
        for page in atlas.pages() {
            match load_page_texture(page.texture_path.as_str()) {
                Some(texture) => textures.push(&texture),
                None => godot_error!("Spine atlas: cannot load texture '{}'", page.texture_path),
            }
        }
        self.atlas_text = text;
        self.textures = textures;
        self.base_mut().emit_changed();
    }

    /// Number of atlas pages.
    #[func]
    fn get_page_count(&self) -> i32 {
        self.textures.len() as i32
    }

    /// Reads the atlas at `path` and the textures of its pages, which are looked up next to it.
    /// Returns an error code: `OK`, or `ERR_FILE_NOT_FOUND` if there is no file.
    #[func]
    fn load_from_atlas_file(&mut self, path: GString) -> Error {
        if path.is_empty() || !FileAccess::file_exists(&path) {
            return Error::ERR_FILE_NOT_FOUND;
        }
        self.set_source_file(path);
        Error::OK
    }

    /// The path of the `.atlas` file this atlas was read from.
    #[func]
    fn get_source_path(&self) -> GString {
        self.source_file.clone()
    }

    /// Normal maps of the pages. This plugin does not load them, so the list is empty.
    #[func]
    fn get_normal_maps(&self) -> VarArray {
        VarArray::new()
    }

    /// Specular maps of the pages. This plugin does not load them, so the list is empty.
    #[func]
    fn get_specular_maps(&self) -> VarArray {
        VarArray::new()
    }
}

impl SpineAtlasResource {
    /// The page textures, in page order.
    pub fn textures(&self) -> Vec<Gd<Texture2D>> {
        self.textures.iter_shared().collect()
    }

    /// Parses the stored atlas text and attaches the stored textures to its pages.
    pub fn parse(&self) -> Option<Atlas> {
        let text = self.atlas_text.to_string();
        if text.is_empty() {
            return None;
        }
        let textures: Vec<Gd<Texture2D>> = self.textures.iter_shared().collect();
        Some(Atlas::parse(
            &text,
            "",
            Some(&mut StoredTextures {
                textures: &textures,
            }),
        ))
    }
}
