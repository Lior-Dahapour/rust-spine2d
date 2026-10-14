//! `SpineAttachment`: one piece of a skin (an image, a mesh, a bounding box...) that scripts can
//! move between skins and slots.

use crate::materials::register_texture_alpha;
use crate::resources::SpineAtlasResource;
use godot::classes::{RefCounted, Texture2D};
use godot::global::godot_error;
use godot::prelude::*;
use smallvec::{smallvec, SmallVec};
use spine2d_core::attachment::{Attachment, MeshAttachment, RegionAttachment};
use spine2d_core::{Sequence, TextureRegion};
use std::sync::Arc;

/// The textures an attachment, skin or sprite keeps alive. Most hold one or two, so they live
/// inline, without a heap allocation.
pub type Textures = SmallVec<[Gd<Texture2D>; 2]>;

/// Adds the textures of `from` to `into`, skipping ones that are already there.
///
/// The core attachments only hold the number of their texture. Whoever shows an attachment keeps
/// these references, so Godot does not free a texture while it is still drawn.
pub fn merge_textures(into: &mut Textures, from: &[Gd<Texture2D>]) {
    for texture in from {
        if !into
            .iter()
            .any(|kept| kept.instance_id() == texture.instance_id())
        {
            into.push(texture.clone());
        }
    }
}

/// The part of `texture` selected by `rect` (in pixels; an empty rectangle selects all of it), as
/// a region of plain, unpacked texture coordinates.
fn texture_region(texture: &Gd<Texture2D>, rect: Rect2) -> Result<TextureRegion, String> {
    let (width, height) = (texture.get_width() as f32, texture.get_height() as f32);
    if width <= 0.0 || height <= 0.0 {
        return Err("the texture has no size".into());
    }
    let rect = if rect.size == Vector2::ZERO {
        Rect2::new(Vector2::ZERO, Vector2::new(width, height))
    } else {
        rect
    };
    let end = rect.end();
    if rect.size.x <= 0.0
        || rect.size.y <= 0.0
        || rect.position.x < 0.0
        || rect.position.y < 0.0
        || end.x > width
        || end.y > height
    {
        return Err(format!(
            "the rectangle {rect:?} is not inside the {width}x{height} texture"
        ));
    }
    Ok(TextureRegion {
        renderer_object: texture.get_rid().to_u64() as usize,
        u: rect.position.x / width,
        v: rect.position.y / height,
        u2: end.x / width,
        v2: end.y / height,
        region_width: rect.size.x as i32,
        region_height: rect.size.y as i32,
        atlas: None,
    })
}

/// `size` if both sides are positive, otherwise `default`.
fn size_or(size: Vector2, default: Vector2) -> Vector2 {
    if size.x > 0.0 && size.y > 0.0 {
        size
    } else {
        default
    }
}

/// The image for an attachment drawn from `texture`, with the textures to keep alive. Prints an
/// error and returns `None` if the name or rectangle is unusable.
fn image_from_texture(
    name: &GString,
    texture: Gd<Texture2D>,
    rect: Rect2,
    premultiplied: bool,
) -> Option<(Arc<TextureRegion>, Textures)> {
    if name.is_empty() {
        godot_error!("Spine: an attachment needs a name");
        return None;
    }
    match texture_region(&texture, rect) {
        Ok(region) => {
            register_texture_alpha(region.renderer_object, premultiplied);
            Some((Arc::new(region), smallvec![texture]))
        }
        Err(message) => {
            godot_error!("Spine: cannot create attachment '{name}': {message}");
            None
        }
    }
}

/// The image for an attachment drawn from the region `region_name` of `atlas`, with the page
/// texture to keep alive. Prints an error and returns `None` if there is no such region.
fn image_from_atlas(
    name: &GString,
    atlas: &Gd<SpineAtlasResource>,
    region_name: &GString,
) -> Option<(Arc<TextureRegion>, Textures)> {
    if name.is_empty() {
        godot_error!("Spine: an attachment needs a name");
        return None;
    }
    let atlas = atlas.bind();
    let parsed = atlas.parse()?;
    let Some(found) = parsed.find_region(&region_name.to_string()) else {
        godot_error!(
            "Spine: cannot create attachment '{name}': the atlas has no region '{region_name}'"
        );
        return None;
    };
    register_texture_alpha(found.region.renderer_object, parsed.pages()[found.page].pma);
    let textures = atlas
        .textures()
        .get(found.page)
        .cloned()
        .into_iter()
        .collect();
    Some((found.region.clone(), textures))
}

/// An attachment of a skin: what a slot shows. Get one from `SpineSkin.get_attachment()`, or
/// make one with the `create_*` functions.
///
/// An attachment is shared, not copied, when it is put into another skin or slot. Use `copy()` for
/// an independent one.
#[derive(GodotClass)]
#[class(base=RefCounted, no_init, tool)]
pub struct SpineAttachment {
    base: Base<RefCounted>,
    attachment: Arc<Attachment>,
    textures: Textures,
}

impl SpineAttachment {
    /// Wraps a core attachment together with the textures it draws.
    pub fn wrap(attachment: Arc<Attachment>, textures: Textures) -> Gd<Self> {
        Gd::from_init_fn(|base| SpineAttachment {
            base,
            attachment,
            textures,
        })
    }

    /// The wrapped core attachment.
    pub fn attachment(&self) -> &Arc<Attachment> {
        &self.attachment
    }

    /// The textures this attachment draws.
    pub fn textures(&self) -> &[Gd<Texture2D>] {
        &self.textures
    }

    /// A region attachment drawing `region`; `size` defaults to the region's original size.
    fn wrap_region(
        name: &GString,
        region: Arc<TextureRegion>,
        size: Vector2,
        textures: Textures,
    ) -> Gd<SpineAttachment> {
        let pixels = region.atlas.as_ref().map_or(
            Vector2::new(region.region_width as f32, region.region_height as f32),
            |info| Vector2::new(info.original_width as f32, info.original_height as f32),
        );
        let size = size_or(size, pixels);
        let attachment = RegionAttachment::with_region(name.to_string(), region, size.x, size.y);
        Self::wrap(Arc::new(Attachment::Region(attachment)), textures)
    }

    /// A mesh from script arrays, or `None` (with an error printed) if they do not fit together.
    fn wrap_mesh(
        name: &GString,
        region: Arc<TextureRegion>,
        textures: Textures,
        uvs: &PackedFloat32Array,
        triangles: &PackedInt32Array,
        vertices: &PackedFloat32Array,
        hull_vertices: i32,
    ) -> Option<Gd<SpineAttachment>> {
        let mut checked = Vec::with_capacity(triangles.len());
        for (at, &value) in triangles.as_slice().iter().enumerate() {
            let Ok(vertex) = u16::try_from(value) else {
                godot_error!(
                    "Spine: cannot create mesh '{name}': triangle value {value} at position {at} is not a vertex number"
                );
                return None;
            };
            checked.push(vertex);
        }
        let Ok(hull) = usize::try_from(hull_vertices) else {
            godot_error!(
                "Spine: cannot create mesh '{name}': the hull cannot have {hull_vertices} vertices"
            );
            return None;
        };
        match MeshAttachment::from_arrays(
            name.to_string(),
            region,
            uvs.as_slice(),
            &checked,
            vertices.as_slice(),
            hull,
        ) {
            Ok(mesh) => Some(Self::wrap(Arc::new(Attachment::Mesh(mesh)), textures)),
            Err(error) => {
                godot_error!("Spine: cannot create mesh '{name}': {error}");
                None
            }
        }
    }

    /// A linked mesh of `parent` drawing `region`, or `None` (with an error printed) if `parent`
    /// is not a mesh.
    fn wrap_linked_mesh(
        name: &GString,
        parent: &Gd<SpineAttachment>,
        region: Arc<TextureRegion>,
        textures: Textures,
    ) -> Option<Gd<SpineAttachment>> {
        let parent = parent.bind();
        let Some(mut linked) = MeshAttachment::new_linked_mesh(parent.attachment()) else {
            godot_error!("Spine: cannot create '{name}': the parent is not a mesh");
            return None;
        };
        linked.vertex.base.name = name.to_string().into();
        linked.sequence = Sequence::new(1, false);
        linked.sequence.regions[0] = Some(region);
        linked.update_sequence();
        Some(Self::wrap(Arc::new(Attachment::Mesh(linked)), textures))
    }

    /// Changes the region attachment, if this is one that nothing else shares, and recomputes its
    /// vertices. A shared attachment is refused: skins and slots hold it by reference, and the
    /// change would show in some of them and not others.
    fn edit_region(&mut self, change: impl FnOnce(&mut RegionAttachment)) -> bool {
        let Some(Attachment::Region(region)) = Arc::get_mut(&mut self.attachment) else {
            godot_error!(
                "Spine: only a region attachment that is not in a skin or slot can be edited"
            );
            return false;
        };
        change(region);
        region.update_sequence();
        true
    }
}

#[godot_api]
impl SpineAttachment {
    /// The attachment's own name, which can differ from the name it has in a skin.
    #[func]
    fn get_attachment_name(&self) -> GString {
        GString::from(self.attachment.name())
    }

    /// What kind of attachment this is: `region`, `mesh`, `bounding_box`, `path`, `point` or
    /// `clipping`.
    #[func]
    fn get_type(&self) -> GString {
        GString::from(match &*self.attachment {
            Attachment::Region(_) => "region",
            Attachment::Mesh(_) => "mesh",
            Attachment::BoundingBox(_) => "bounding_box",
            Attachment::Path(_) => "path",
            Attachment::Point(_) => "point",
            Attachment::Clipping(_) => "clipping",
        })
    }

    /// Makes a flat image attachment from a texture: `rect` is the part of the texture to draw in
    /// pixels (leave it empty for the whole texture) and `size` how big it is on the skeleton in
    /// skeleton units (leave it zero to use the pixel size). It is centered on its slot's bone;
    /// move it with `set_position`, `set_rotation` and `set_scale`. Set `premultiplied` if the
    /// texture has premultiplied alpha. Returns `null` and prints an error if the rectangle is not
    /// inside the texture.
    #[func]
    fn create_region(
        name: GString,
        texture: Gd<Texture2D>,
        #[opt(default = Rect2::default())] rect: Rect2,
        #[opt(default = Vector2::ZERO)] size: Vector2,
        #[opt(default = false)] premultiplied: bool,
    ) -> Option<Gd<SpineAttachment>> {
        let (region, textures) = image_from_texture(&name, texture, rect, premultiplied)?;
        Some(Self::wrap_region(&name, region, size, textures))
    }

    /// Like `create_region`, but the image is the region called `region_name` of an atlas, the
    /// way a skeleton's own images are. The size defaults to the region's original size.
    /// Returns `null` and prints an error if the atlas has no such region.
    #[func]
    fn create_region_from_atlas(
        name: GString,
        atlas: Gd<SpineAtlasResource>,
        region_name: GString,
        #[opt(default = Vector2::ZERO)] size: Vector2,
    ) -> Option<Gd<SpineAttachment>> {
        let (region, textures) = image_from_atlas(&name, &atlas, &region_name)?;
        Some(Self::wrap_region(&name, region, size, textures))
    }

    /// Makes a mesh from arrays, which can deform with the skeleton's bones.
    ///
    /// * `uvs`: 2 values per vertex, from 0 to 1 across the image.
    /// * `triangles`: 3 vertex numbers per triangle.
    /// * `vertices`: for an unweighted mesh 2 values per vertex (`x`, `y` in the axes of the
    ///   slot's bone). For a weighted mesh Spine's layout: per vertex the number of bones, then
    ///   for each bone its number (see `SpineSkeletonDataResource.get_bone_index`), `x`, `y` and
    ///   its weight. It is weighted when there are more values than 2 per vertex.
    /// * `hull_vertices`: how many of the first vertices form the outline.
    ///
    /// The image is given as in `create_region`. Returns `null` and prints what is wrong if the
    /// arrays do not fit together. Whether the bone numbers exist is checked when the mesh is
    /// shown on a skeleton.
    // The arguments are the script-facing signature, which GDScript passes one by one.
    #[allow(clippy::too_many_arguments)]
    #[func]
    fn create_mesh(
        name: GString,
        texture: Gd<Texture2D>,
        uvs: PackedFloat32Array,
        triangles: PackedInt32Array,
        vertices: PackedFloat32Array,
        hull_vertices: i32,
        #[opt(default = Rect2::default())] rect: Rect2,
        #[opt(default = false)] premultiplied: bool,
    ) -> Option<Gd<SpineAttachment>> {
        let (region, textures) = image_from_texture(&name, texture, rect, premultiplied)?;
        Self::wrap_mesh(
            &name,
            region,
            textures,
            &uvs,
            &triangles,
            &vertices,
            hull_vertices,
        )
    }

    /// Like `create_mesh`, but the image is the region called `region_name` of an atlas.
    #[func]
    fn create_mesh_from_atlas(
        name: GString,
        atlas: Gd<SpineAtlasResource>,
        region_name: GString,
        uvs: PackedFloat32Array,
        triangles: PackedInt32Array,
        vertices: PackedFloat32Array,
        hull_vertices: i32,
    ) -> Option<Gd<SpineAttachment>> {
        let (region, textures) = image_from_atlas(&name, &atlas, &region_name)?;
        Self::wrap_mesh(
            &name,
            region,
            textures,
            &uvs,
            &triangles,
            &vertices,
            hull_vertices,
        )
    }

    /// Makes a copy of the mesh `parent` with a different image: same shape, same bone weights, and
    /// it follows the parent's deform animations. The image is given as in `create_region`, and
    /// should have the layout of the parent's image, because the mesh keeps its UVs. Returns
    /// `null` and prints an error if `parent` is not a mesh.
    #[func]
    fn create_linked_mesh(
        name: GString,
        parent: Gd<SpineAttachment>,
        texture: Gd<Texture2D>,
        #[opt(default = Rect2::default())] rect: Rect2,
        #[opt(default = false)] premultiplied: bool,
    ) -> Option<Gd<SpineAttachment>> {
        let (region, textures) = image_from_texture(&name, texture, rect, premultiplied)?;
        Self::wrap_linked_mesh(&name, &parent, region, textures)
    }

    /// Like `create_linked_mesh`, but the image is the region called `region_name` of an atlas.
    #[func]
    fn create_linked_mesh_from_atlas(
        name: GString,
        parent: Gd<SpineAttachment>,
        atlas: Gd<SpineAtlasResource>,
        region_name: GString,
    ) -> Option<Gd<SpineAttachment>> {
        let (region, textures) = image_from_atlas(&name, &atlas, &region_name)?;
        Self::wrap_linked_mesh(&name, &parent, region, textures)
    }

    /// Moves a region attachment on its slot's bone, in skeleton units. Returns false for other
    /// kinds of attachment, or one that is already in a skin or slot (use `copy()` first).
    #[func]
    fn set_position(&mut self, position: Vector2) -> bool {
        self.edit_region(|region| {
            region.x = position.x;
            region.y = position.y;
        })
    }

    /// Rotates a region attachment, in degrees. See `set_position` for when it can fail.
    #[func]
    fn set_rotation(&mut self, degrees: f32) -> bool {
        self.edit_region(|region| region.rotation = degrees)
    }

    /// Scales a region attachment. See `set_position` for when it can fail.
    #[func]
    fn set_scale(&mut self, scale: Vector2) -> bool {
        self.edit_region(|region| {
            region.scale_x = scale.x;
            region.scale_y = scale.y;
        })
    }

    /// Sets the size of a region attachment in skeleton units. See `set_position` for when it can
    /// fail.
    #[func]
    fn set_size(&mut self, size: Vector2) -> bool {
        self.edit_region(|region| {
            region.width = size.x;
            region.height = size.y;
        })
    }

    /// Tints a region attachment. See `set_position` for when it can fail.
    #[func]
    fn set_color(&mut self, color: Color) -> bool {
        self.edit_region(|region| {
            region.color = spine2d_core::Color::new(color.r, color.g, color.b, color.a);
        })
    }

    /// An independent copy. A mesh is copied as a linked mesh of the same shape, so it still
    /// follows the original's deform animations.
    #[func]
    fn copy(&self) -> Gd<SpineAttachment> {
        let copy = Attachment::copy(&self.attachment);
        Self::wrap(Arc::new(copy), self.textures.clone())
    }
}

/// One entry of a skin: an attachment, the slot it belongs to and the name the slot uses for it.
#[derive(GodotClass)]
#[class(base=RefCounted, no_init, tool)]
pub struct SpineSkinEntry {
    base: Base<RefCounted>,
    slot_index: i32,
    name: GString,
    attachment: Gd<SpineAttachment>,
}

impl SpineSkinEntry {
    /// Describes one skin entry for scripts.
    pub fn create(slot_index: usize, name: &str, attachment: Gd<SpineAttachment>) -> Gd<Self> {
        Gd::from_init_fn(|base| SpineSkinEntry {
            base,
            slot_index: slot_index as i32,
            name: GString::from(name),
            attachment,
        })
    }
}

#[godot_api]
impl SpineSkinEntry {
    #[func]
    fn get_slot_index(&self) -> i32 {
        self.slot_index
    }

    /// The name the slot uses for this attachment (the key in the skin).
    #[func]
    fn get_name(&self) -> GString {
        self.name.clone()
    }

    #[func]
    fn get_attachment(&self) -> Gd<SpineAttachment> {
        self.attachment.clone()
    }
}
