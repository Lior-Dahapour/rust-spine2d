//! Resources that hold the files of a Spine skeleton: the atlas, the skeleton file, and the
//! combined skeleton data a sprite plays.
//!
//! Each resource stores its source (atlas text, skeleton bytes, textures) inside the saved
//! resource, so exported games do not need the original files. In the editor, setting the source
//! file path reads the file and refreshes the stored copy.

use crate::skin::SpineSkin;
use godot::classes::{FileAccess, Resource, Texture2D};
use godot::global::godot_error;
use godot::prelude::*;
use godot::tools::try_load;
use spine2d_core::{
    AnimationStateData, Atlas, AtlasAttachmentLoader, AtlasPage, SkeletonBinary, SkeletonData,
    SkeletonJson, TextureLoader,
};
use std::collections::HashMap;
use std::sync::Arc;

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
            match try_load::<Texture2D>(page.texture_path.as_str()) {
                Ok(texture) => textures.push(&texture),
                Err(error) => godot_error!(
                    "Spine atlas: cannot load texture '{}': {error}",
                    page.texture_path
                ),
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
}

/// An atlas and a skeleton file combined, plus the mix times between animations.
#[derive(GodotClass)]
#[class(base=Resource, tool)]
pub struct SpineSkeletonDataResource {
    base: Base<Resource>,
    #[export]
    #[var(set = set_atlas)]
    atlas: Option<Gd<SpineAtlasResource>>,
    #[export]
    #[var(set = set_skeleton_file)]
    skeleton_file: Option<Gd<SpineSkeletonFileResource>>,
    /// Scale applied to the skeleton while loading.
    #[export]
    #[var(set = set_scale)]
    scale: f32,
    /// Crossfade time used between animations that have no specific mix.
    #[export]
    default_mix: f32,
    mixes: HashMap<(String, String), f32>,
    loaded: Option<Arc<LoadedSkeleton>>,
}

/// A parsed skeleton and the textures its atlas pages use.
pub struct LoadedSkeleton {
    pub data: Arc<SkeletonData>,
    /// For each atlas page texture handle, whether the page is premultiplied alpha.
    pub premultiplied: HashMap<usize, bool>,
}

#[godot_api]
impl IResource for SpineSkeletonDataResource {
    fn init(base: Base<Resource>) -> Self {
        SpineSkeletonDataResource {
            base,
            atlas: None,
            skeleton_file: None,
            scale: 1.0,
            default_mix: 0.0,
            mixes: HashMap::new(),
            loaded: None,
        }
    }
}

#[godot_api]
impl SpineSkeletonDataResource {
    #[func]
    fn set_atlas(&mut self, atlas: Option<Gd<SpineAtlasResource>>) {
        self.atlas = atlas;
        self.invalidate();
    }

    #[func]
    fn set_skeleton_file(&mut self, file: Option<Gd<SpineSkeletonFileResource>>) {
        self.skeleton_file = file;
        self.invalidate();
    }

    #[func]
    fn set_scale(&mut self, scale: f32) {
        self.scale = scale;
        self.invalidate();
    }

    /// Sets the crossfade time when switching from animation `from` to animation `to`.
    #[func]
    fn set_mix(&mut self, from: GString, to: GString, duration: f32) {
        self.mixes
            .insert((from.to_string(), to.to_string()), duration);
    }

    /// True once the atlas and skeleton file load into a skeleton.
    #[func]
    fn is_skeleton_data_loaded(&mut self) -> bool {
        self.loaded().is_some()
    }

    /// Names of all animations.
    #[func]
    pub fn get_animation_names(&mut self) -> PackedStringArray {
        self.loaded().map_or_else(PackedStringArray::new, |l| {
            l.data
                .animations
                .iter()
                .map(|a| GString::from(a.name()))
                .collect()
        })
    }

    /// The number of the slot called `name` (what skins use to refer to slots), or -1.
    #[func]
    fn get_slot_index(&mut self, name: GString) -> i32 {
        self.loaded()
            .and_then(|l| l.data.find_slot(&name.to_string()).map(|s| s.index as i32))
            .unwrap_or(-1)
    }

    /// The number of the bone called `name` (what weighted meshes use to refer to bones), or -1.
    #[func]
    fn get_bone_index(&mut self, name: GString) -> i32 {
        self.loaded()
            .and_then(|l| l.data.find_bone(&name.to_string()).map(|b| b.index as i32))
            .unwrap_or(-1)
    }

    /// The number of the constraint called `name` (what skins use to refer to constraints), or -1.
    #[func]
    fn get_constraint_index(&mut self, name: GString) -> i32 {
        let name = name.to_string();
        self.loaded()
            .and_then(|l| l.data.constraints.iter().position(|c| c.name() == name))
            .map_or(-1, |index| index as i32)
    }

    /// Names of all slots, in setup order.
    #[func]
    pub fn get_slot_names(&mut self) -> PackedStringArray {
        self.loaded().map_or_else(PackedStringArray::new, |l| {
            l.data
                .slots
                .iter()
                .map(|s| GString::from(s.name.as_str()))
                .collect()
        })
    }

    /// The skin called `name`, or `null` if there is none. The skin is a copy-on-write handle:
    /// changing it does not change this skeleton data.
    #[func]
    fn find_skin(&mut self, name: GString) -> Option<Gd<SpineSkin>> {
        let loaded = self.loaded()?;
        let skin = loaded.data.find_skin(&name.to_string())?;
        Some(SpineSkin::wrap(skin.clone()))
    }

    /// Names of all skins.
    #[func]
    pub fn get_skin_names(&mut self) -> PackedStringArray {
        self.loaded().map_or_else(PackedStringArray::new, |l| {
            l.data
                .skins
                .iter()
                .map(|s| GString::from(s.name()))
                .collect()
        })
    }

    /// Length in seconds of the animation called `name`, or 0 if there is none.
    #[func]
    fn get_animation_duration(&mut self, name: GString) -> f32 {
        self.loaded()
            .and_then(|l| l.data.find_animation(&name.to_string()).map(|a| a.duration))
            .unwrap_or(0.0)
    }

    /// Skeleton width as exported (in the skeleton's units).
    #[func]
    fn get_width(&mut self) -> f32 {
        self.loaded().map_or(0.0, |l| l.data.width)
    }

    /// Skeleton height as exported (in the skeleton's units).
    #[func]
    fn get_height(&mut self) -> f32 {
        self.loaded().map_or(0.0, |l| l.data.height)
    }
}

impl SpineSkeletonDataResource {
    fn invalidate(&mut self) {
        self.loaded = None;
        self.base_mut().emit_changed();
    }

    /// The parsed skeleton, loading it on first use. Errors are printed once per load attempt.
    pub fn loaded(&mut self) -> Option<Arc<LoadedSkeleton>> {
        if self.loaded.is_none() {
            self.loaded = self.load().map(Arc::new);
        }
        self.loaded.clone()
    }

    fn load(&self) -> Option<LoadedSkeleton> {
        let atlas = self.atlas.as_ref()?.bind().parse()?;
        let file = self.skeleton_file.as_ref()?.bind();
        let bytes = file.bytes.to_vec();
        if bytes.is_empty() {
            return None;
        }
        let attachments = AtlasAttachmentLoader::new(&atlas);
        let loaded = if file.is_binary {
            let mut loader = SkeletonBinary::new(&attachments);
            loader.set_scale(self.scale);
            loader.read_skeleton_data(&bytes)
        } else {
            let mut loader = SkeletonJson::new(&attachments);
            loader.set_scale(self.scale);
            loader.read_skeleton_data(&String::from_utf8_lossy(&bytes))
        };
        match loaded {
            Ok(data) => {
                let premultiplied = atlas
                    .pages()
                    .iter()
                    .map(|page| (page.texture, page.pma))
                    .collect();
                Some(LoadedSkeleton {
                    data: Arc::new(data),
                    premultiplied,
                })
            }
            Err(error) => {
                godot_error!("Spine skeleton: {error}");
                None
            }
        }
    }

    /// Animation state data with this resource's mixes, for one sprite.
    pub fn state_data(&self, data: &Arc<SkeletonData>) -> AnimationStateData {
        let mut state_data = AnimationStateData::new(data.clone());
        state_data.default_mix = self.default_mix;
        for ((from, to), duration) in &self.mixes {
            state_data.set_mix(from, to, *duration);
        }
        state_data
    }
}
