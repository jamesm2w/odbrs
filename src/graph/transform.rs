use eframe::epaint::{pos2, Pos2, Vec2};

use super::AdjacencyList;

#[derive(Default, Debug)]
pub struct Transform {
    pub dragx: f32,
    pub dragy: f32,
    pub zoom: f32,

    pub scale: f32,
    pub left: f32,
    pub right: f32,
    pub top: f32,
    pub bottom: f32,
}

impl Transform {
    pub fn new(adjlist: &AdjacencyList) -> Self {
        let bounding = super::minimal_bounding(adjlist);

        Transform {
            dragx: 0.0,
            dragy: 0.0,
            zoom: 1.0,
            scale: 1.0,
            left: bounding.0 as _,
            right: bounding.1 as _,
            top: bounding.2 as _,
            bottom: bounding.3 as _,
        }
    }

    // Drag the graph around
    pub fn drag(&mut self, drag_delta: Vec2) {
        self.dragx += drag_delta.x;

        // self.dragx = self.dragx.clamp(map_x_screen(&self, self.left), map_x_screen(&self, self.right));

        self.dragy += drag_delta.y;

        // self.dragy = self.dragy.clamp(map_y_screen(&self, self.top), map_y_screen(&self, self.bottom));
    }

    // Increase the zoom level of the graph
    pub fn zoom(&mut self, scroll_delta: f32) {
        self.zoom += scroll_delta / 50.0;

        self.zoom = self.zoom.clamp(1.0, 100.0)
    }

    // Calculate the scale of the graph for the given width
    pub fn scale(&mut self, width: f32) {
        self.scale = width / (self.right - self.left)
    }

    // Convert a map coord in ERSG:27700 to a screen-space position
    pub fn map_to_screen(&self, x: f64, y: f64) -> Pos2 {
        pos2(map_x_screen(&self, x as _), map_y_screen(&self, y as _))
    }

    // Convert a scree-space position to the OS natl grid map
    pub fn screen_to_map(&self, pos: Pos2) -> (f64, f64) {
        let Pos2 { x, y } = pos;

        (
            ((((x / self.zoom) + self.dragx) / self.scale) + self.left) as f64,
            ((((y / self.zoom) + self.dragy) / -self.scale) + self.top) as f64,
        )
    }
}

// Inline single arugment transformations
#[inline]
fn map_x_screen(transform: &Transform, x: f32) -> f32 {
    (((x - transform.left) * transform.scale) - transform.dragx) * transform.zoom
}

#[inline]
fn map_y_screen(transform: &Transform, y: f32) -> f32 {
    (((y - transform.top) * -transform.scale) - transform.dragy) * transform.zoom
}
