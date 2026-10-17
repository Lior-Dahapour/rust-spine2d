//! Drawing: the system that draws the skeletons that changed, and what it draws with: the meshes,
//! the debug overlay and the materials.

pub mod batch_mesh;
pub mod culler;
pub mod debug;
pub mod materials;
mod spine_render;
pub mod view;

use godot::builtin::Rid;
use godot::classes::RenderingServer;
use godot::obj::Singleton;
use spine2d_ecs::bevy_ecs::entity::{Entity, EntityHashMap};
use spine2d_ecs::bevy_ecs::query::{Changed, Or, QueryData};
use spine2d_ecs::bevy_ecs::system::{NonSendMut, Query, Res, ResMut};
use spine2d_ecs::{SpineData, SpineSkeleton};

use crate::runtime::store::{SkeletonDataId, SkeletonStore};
use materials::{Materials, TextureAlpha};
use view::SpineView;

pub use spine_render::SpineRender;

/// A non-send resource of the world: the drawing state of every skeleton, which holds Godot
/// byte arrays and so cannot be a component.
pub struct RenderStore {
    entries: EntityHashMap<SpineRender>,
    /// A canvas item that is only touched to tell the render server something changed.
    ping_item: Rid,
    ping_state: bool,
}

impl Default for RenderStore {
    fn default() -> Self {
        RenderStore {
            entries: EntityHashMap::default(),
            ping_item: Rid::Invalid,
            ping_state: false,
        }
    }
}

impl Drop for RenderStore {
    fn drop(&mut self) {
        if self.ping_item != Rid::Invalid {
            RenderingServer::singleton().free_rid(self.ping_item);
        }
    }
}

impl RenderStore {
    /// Gives the skeleton `entity` its drawing state.
    pub fn insert(&mut self, entity: Entity, render: SpineRender) {
        self.entries.insert(entity, render);
    }

    /// Takes the drawing state of the skeleton `entity` out of the store. Dropping it frees its
    /// meshes on the render server.
    pub fn remove(&mut self, entity: Entity) -> Option<SpineRender> {
        self.entries.remove(&entity)
    }

    /// Tells the render server something changed, so it draws the frame even in low-processor
    /// mode, where it otherwise draws only after a change. Drawing itself happens just before the
    /// frame, in [`render_skeletons`], which is too late to ask: the server decides before that.
    pub fn ping(&mut self) {
        let mut server = RenderingServer::singleton();
        if self.ping_item == Rid::Invalid {
            self.ping_item = server.canvas_item_create();
        }
        self.ping_state = !self.ping_state;
        server.canvas_item_set_visible(self.ping_item, self.ping_state);
    }

    /// The drawing state of the skeleton `entity`.
    pub fn get_mut(&mut self, entity: Entity) -> Option<&mut SpineRender> {
        self.entries.get_mut(&entity)
    }
}

/// Draws every skeleton that changed since the last run: its pose, or what the sprite says about
/// itself. A skeleton that did not change is still on the render server as it was left, so it
/// costs nothing here. Skeletons that are hidden, or off screen with culling on, are skipped and
/// drawn when they come back, because that changes their view.
pub fn render_skeletons(
    mut store: NonSendMut<RenderStore>,
    mut materials: ResMut<Materials>,
    alpha: Res<TextureAlpha>,
    skeleton_store: Res<SkeletonStore>,
    mut skeletons: Query<Drawable, Moved>,
) {
    for mut drawable in &mut skeletons {
        let view = &drawable.view;
        if !view.is_drawn() {
            continue;
        }
        let Some(render) = store.get_mut(drawable.entity) else {
            continue;
        };
        let Some(stored) = skeleton_store.get(drawable.data.0) else {
            continue;
        };
        let canvas_item = view.canvas_item();
        let skeleton = drawable.skeleton.as_borrow_ref(&stored.data);
        let Some(bounds) = render.draw(&skeleton, view, canvas_item, &mut materials, &alpha) else {
            continue;
        };
        // Written only when the notifier is created, so a drawn skeleton does not mark its own
        // view changed every frame.
        if drawable.view.cull && render.watch(canvas_item, bounds) {
            // The server only reports transitions, so a sprite that starts off screen would never
            // say so. Assume off screen until it reports entering; it was just drawn once.
            drawable.view.on_screen = false;
        }
    }
}

/// What `render_skeletons` needs of a skeleton.
#[derive(QueryData)]
#[query_data(mutable)]
pub struct Drawable {
    entity: Entity,
    skeleton: &'static SpineSkeleton,
    data: &'static SpineData<SkeletonDataId>,
    view: &'static mut SpineView,
}

/// A skeleton whose pose changed, or whose sprite said something new about itself.
type Moved = Or<(Changed<SpineSkeleton>, Changed<SpineView>)>;
