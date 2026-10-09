//! Platform- and renderer-independent building blocks: colors, geometry,
//! shapes and their tessellation into triangles. Nothing here performs I/O.

mod atlas;
mod color;
mod display_list;
mod galley;
mod geometry;
pub mod input;
mod paint;
mod shape;
pub mod tessellator;
mod texture;

pub use atlas::{AtlasRegion, TextureAtlas};
pub use color::Color;
pub use display_list::{ClippedShape, DisplayList};
pub use galley::{Galley, GalleyDecoration, GalleyRow, GlyphQuad};
pub use geometry::{PhysicalSize, Point, Rect, Vec2, point, vec2};
pub use input::{
    Event, ImeEvent, InputState, Key, KeyboardShortcut, Modifiers, POINTS_PER_SCROLL_LINE,
    PointerButton, RawInput, TouchPhase,
};
pub use paint::{
    Gradient, GradientKind, Shadow, Transform, cubic_bezier_points, dashes, quadratic_bezier_points,
};
pub use shape::{PaintCallback, Shape, Stroke};
pub use tessellator::{ClippedMesh, Mesh, Tessellator, Vertex};
pub use texture::{ColorImage, TextureId, TexturesDelta};
