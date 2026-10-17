//! The drawing side of a skeleton entity: the renderer, the GPU meshes and the debug overlay.
//!
//! Everything about animating lives in the ECS components of `spine2d-ecs` (`SpineSkeleton`,
//! `SpineAnimation`). This holds what is only needed to draw, and it is drawn from the entity's
//! pose. It holds Godot objects (the render server handles and byte arrays of its meshes), which
//! cannot be shared between threads like ECS components, so the non-send
//! [`RenderStore`](crate::systems::render::RenderStore) owns it.

use crate::resources::LoadedSkeleton;
use crate::systems::render::batch_mesh::{BatchStats, MeshBatch};
use crate::systems::render::culler::Culler;
use crate::systems::render::debug::DebugLayer;
use crate::systems::render::materials::{MaterialKey, Materials, TextureAlpha};
use crate::systems::render::view::SpineView;
use godot::classes::RenderingServer;
use godot::obj::Singleton;
use godot::prelude::*;
use smallvec::SmallVec;
use spine2d_core::skeleton::BorrowedSkeletonRef;
use spine2d_core::SlotId;
use spine2d_render::{SkeletonRenderer, SlotMarker};
use std::sync::Arc;

/// Draw index of the first mesh. It is far below the index of any ordinary child, so the meshes
/// (and the slot nodes between them) sit behind the sprite's other children.
///
/// Every mesh needs its own index: Godot orders a canvas item's children by index and gives no
/// guarantee for equal ones, so meshes that all had index 0 were drawn in a scrambled order once a
/// skeleton had enough of them.
pub const SPLIT_BASE_INDEX: i32 = -(1 << 24);

/// Distance between the draw indices of consecutive meshes while the sprite has slot nodes. The
/// slot nodes between two meshes take the indices in between, in draw order.
pub const SPLIT_INDEX_STRIDE: i32 = 256;

/// The draw index of mesh number `n`, in draw order. Meshes are `SPLIT_INDEX_STRIDE` apart while
/// slot nodes need room between them, and next to each other otherwise.
fn mesh_draw_index(n: usize, split: bool) -> i32 {
    let stride = if split { SPLIT_INDEX_STRIDE } else { 1 };
    SPLIT_BASE_INDEX + n as i32 * stride
}

/// The drawing state of one skeleton.
pub struct SpineRender {
    renderer: SkeletonRenderer,
    batches: Vec<MeshBatch>,
    loaded: Arc<LoadedSkeleton>,
    debug: Option<DebugLayer>,
    /// Slots (by index) the meshes are split after, for the slot nodes; empty when there are none.
    split_slots: Vec<bool>,
    /// Where the split slots fell among the groups of the last draw, in draw order.
    markers: Vec<SlotMarker>,
    /// Tells the render server to report when the sprite is on screen; created on first use.
    culler: Option<Culler>,
    /// What the render server calls when the sprite enters and leaves a viewport.
    on_screen_callables: (Callable, Callable),
    /// The draw index last given to each slot node's canvas item.
    applied: SmallVec<[(Rid, i32); 2]>,
    /// The [`SpineView::order_epoch`] the applied indices belong to.
    seen_epoch: u32,
}

/// The groups of meshes that belong to each slot node with materials, and its materials. The
/// meshes were cut before and after such a slot, so the groups between the cut before it and the
/// cut after it are its own.
fn slot_material_ranges(
    view: &SpineView,
    order: &[SlotId],
    markers: &[SlotMarker],
) -> SmallVec<[(std::ops::Range<usize>, [Rid; 4]); 2]> {
    let groups_before = |slot: SlotId| {
        markers
            .iter()
            .find(|marker| marker.slot == slot)
            .map(|marker| marker.groups_before)
    };
    view.slot_items()
        .iter()
        .filter(|item| item.has_materials())
        .filter_map(|item| {
            let end = groups_before(item.slot)?;
            let place = order.iter().position(|slot| *slot == item.slot)?;
            let start = match place.checked_sub(1) {
                Some(before) => groups_before(order[before])?,
                None => 0,
            };
            Some((start..end, item.materials))
        })
        .collect()
}

impl SpineRender {
    pub fn new(loaded: Arc<LoadedSkeleton>, on_screen_callables: (Callable, Callable)) -> Self {
        SpineRender {
            renderer: SkeletonRenderer::new(),
            batches: Vec::new(),
            loaded,
            debug: None,
            split_slots: Vec::new(),
            markers: Vec::new(),
            culler: None,
            on_screen_callables,
            applied: SmallVec::new(),
            seen_epoch: 0,
        }
    }

    /// Chooses the slots the meshes are split after at the next draw; none clears the split.
    fn set_split_slots(&mut self, slots: impl Iterator<Item = SlotId>) {
        self.split_slots.clear();
        for slot in slots {
            if self.split_slots.len() <= slot.0 {
                self.split_slots.resize(slot.0 + 1, false);
            }
            self.split_slots[slot.0] = true;
        }
    }

    /// The draw index for something drawn in the gap before group `groups_before`, as the
    /// `rank`-th item of that gap (0 first). Meaningful after a draw with split slots.
    pub fn gap_draw_index(groups_before: usize, rank: usize) -> i32 {
        SPLIT_BASE_INDEX + (groups_before as i32 - 1) * SPLIT_INDEX_STRIDE + 1 + rank as i32
    }

    /// Draws the pose of `skeleton`, reusing the meshes of earlier frames. Returns the bounds of
    /// what was drawn, if anything.
    pub fn draw(
        &mut self,
        skeleton: &BorrowedSkeletonRef,
        view: &SpineView,
        parent_item: Rid,
        materials: &mut Materials,
        alpha: &TextureAlpha,
    ) -> Option<Rect2> {
        // A slot node that gives its slot a material needs the meshes of that slot alone, so the
        // meshes are cut before the slot as well as after it.
        let order = skeleton.draw_order().applied_pose();
        let before_materials = view
            .slot_items()
            .iter()
            .filter(|item| item.has_materials())
            .filter_map(|item| {
                let place = order.iter().position(|slot| *slot == item.slot)?;
                place.checked_sub(1).map(|before| order[before])
            });
        let cut: SmallVec<[SlotId; 4]> = view
            .slot_items()
            .iter()
            .map(|item| item.slot)
            .chain(before_materials)
            .collect();
        self.set_split_slots(cut.into_iter());
        let SpineRender {
            renderer,
            batches,
            loaded,
            debug,
            split_slots,
            markers,
            ..
        } = self;
        let rendered = renderer.render_batches_split(skeleton, split_slots);
        markers.clear();
        markers.extend_from_slice(rendered.markers);
        while batches.len() < rendered.groups.len() {
            batches.push(MeshBatch::new(parent_item));
        }
        let split = !split_slots.is_empty();
        let slot_materials = slot_material_ranges(view, order, rendered.markers);
        for (n, (batch, group)) in batches.iter_mut().zip(rendered.groups).enumerate() {
            let own_material = slot_materials
                .iter()
                .find(|(groups, _)| groups.contains(&n))
                .and_then(|(_, materials)| {
                    Some(materials[group.blend_mode as usize]).filter(|m| *m != Rid::Invalid)
                })
                .or_else(|| view.material(group.blend_mode));
            batch.set_draw_index(mesh_draw_index(n, split));
            let texture = Rid::new(group.texture as u64);
            let premultiplied = loaded
                .premultiplied
                .get(&group.texture)
                .copied()
                .or_else(|| alpha.get(group.texture))
                .unwrap_or(false);
            let wants_two_color = group.dark_color != 0xff00_0000;
            batch.upload(
                group,
                rendered.group_commands(group),
                wants_two_color,
                texture,
                |two_color| {
                    own_material.unwrap_or_else(|| {
                        materials.get(MaterialKey::new(group.blend_mode, premultiplied, two_color))
                    })
                },
            );
        }
        for batch in batches.iter_mut().skip(rendered.groups.len()) {
            batch.set_visible(false);
        }
        let debug_flags = view.debug();
        if debug_flags.any() {
            debug
                .get_or_insert_with(|| DebugLayer::new(parent_item))
                .draw(debug_flags, skeleton, rendered.commands);
        } else if let Some(layer) = debug {
            layer.clear();
        }
        let bounds = batches
            .iter()
            .filter_map(MeshBatch::drawn_bounds)
            .reduce(Rect2::merge);
        self.place_slot_items(view);
        bounds
    }

    /// Gives each slot node's canvas item the draw index of the gap after its slot, or its place in
    /// the child list when its slot drew nothing. Only calls the render server for indices that
    /// changed.
    fn place_slot_items(&mut self, view: &SpineView) {
        let items = view.slot_items();
        if self.seen_epoch != view.order_epoch() {
            // Godot reset the indices, so the ones remembered are no longer on the server.
            self.applied.clear();
            self.seen_epoch = view.order_epoch();
        }
        self.applied
            .retain(|(item, _)| items.iter().any(|slot| slot.item == *item));
        // Markers come in draw order, so a gap's items are numbered in the order of their slots.
        let (mut gap, mut rank) = (usize::MAX, 0);
        for marker in &self.markers {
            if marker.groups_before != gap {
                (gap, rank) = (marker.groups_before, 0);
            }
            for item in items.iter().filter(|item| item.slot == marker.slot) {
                let index = Self::gap_draw_index(gap, rank);
                Self::apply_index(&mut self.applied, item.item, index);
                rank += 1;
            }
        }
        for item in items
            .iter()
            .filter(|item| !self.markers.iter().any(|marker| marker.slot == item.slot))
        {
            Self::apply_index(&mut self.applied, item.item, item.fallback);
        }
    }

    /// Sets the draw index of `item` on the render server unless it already has it.
    fn apply_index(applied: &mut SmallVec<[(Rid, i32); 2]>, item: Rid, index: i32) {
        match applied.iter_mut().find(|(known, _)| *known == item) {
            Some((_, last)) if *last == index => return,
            Some((_, last)) => *last = index,
            None => applied.push((item, index)),
        }
        RenderingServer::singleton().canvas_item_set_draw_index(item, index);
    }

    /// Keeps the visibility notifier around `bounds`, creating it on first use. Returns whether
    /// the notifier was created by this call.
    pub fn watch(&mut self, parent_item: Rid, bounds: Rect2) -> bool {
        let created = self.culler.is_none();
        let callables = &self.on_screen_callables;
        self.culler
            .get_or_insert_with(|| Culler::new(parent_item))
            .watch(bounds, || callables.clone());
        created
    }

    /// Counters summed over all meshes.
    pub fn stats(&self) -> BatchStats {
        let mut total = BatchStats::default();
        for batch in &self.batches {
            total.add(batch.stats());
        }
        total
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn batch_index(n: i32) -> i32 {
        SPLIT_BASE_INDEX + n * SPLIT_INDEX_STRIDE
    }

    #[test]
    fn every_mesh_gets_its_own_index_in_draw_order_behind_ordinary_children() {
        for split in [false, true] {
            let indices: Vec<i32> = (0..300).map(|n| mesh_draw_index(n, split)).collect();
            assert!(
                indices.windows(2).all(|pair| pair[0] < pair[1]),
                "split: {split}"
            );
            assert!(indices.iter().all(|&index| index < 0), "split: {split}");
        }
    }

    #[test]
    fn slot_nodes_sit_between_the_meshes_around_their_gap() {
        for gap in 1..5usize {
            let first = SpineRender::gap_draw_index(gap, 0);
            let last = SpineRender::gap_draw_index(gap, 10);
            assert!(batch_index(gap as i32 - 1) < first);
            assert!(first < last);
            assert!(last < batch_index(gap as i32));
        }
    }

    #[test]
    fn a_gap_before_the_first_mesh_is_below_every_mesh() {
        assert!(SpineRender::gap_draw_index(0, 0) < batch_index(0));
    }
}
