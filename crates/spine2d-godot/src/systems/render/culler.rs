//! Reports when a sprite is on screen, so drawing can be skipped while it is not.

use godot::builtin::{Callable, Rect2, Rid};
use godot::classes::RenderingServer;
use godot::obj::Singleton;

/// A canvas item that tells the render server to report when the sprite is on screen, so drawing
/// can be skipped while it is not. It handles cameras, canvas layers and sub-viewports.
pub struct Culler {
    item: Rid,
    area: Option<Rect2>,
}

/// Fraction of the bounds added on every side, so a moving skeleton rarely updates the area.
const CULL_MARGIN: f32 = 0.5;

impl Culler {
    pub fn new(parent_item: Rid) -> Self {
        let mut rs = RenderingServer::singleton();
        let item = rs.canvas_item_create();
        rs.canvas_item_set_parent(item, parent_item);
        Culler { item, area: None }
    }

    /// Makes sure the watched area encloses `bounds`. `callables` creates the enter and exit
    /// callbacks and only runs when the area actually changes.
    pub fn watch(&mut self, bounds: Rect2, callables: impl FnOnce() -> (Callable, Callable)) {
        if self.area.is_some_and(|area| area.encloses(bounds)) {
            return;
        }
        let margin = bounds.size * CULL_MARGIN;
        let area = Rect2::new(bounds.position - margin, bounds.size + margin * 2.0);
        let (enter, exit) = callables();
        RenderingServer::singleton()
            .canvas_item_set_visibility_notifier(self.item, true, area, &enter, &exit);
        self.area = Some(area);
    }
}

impl Drop for Culler {
    fn drop(&mut self) {
        RenderingServer::singleton().free_rid(self.item);
    }
}
