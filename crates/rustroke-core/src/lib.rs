//! Platform- and renderer-independent building blocks: colors, geometry,
//! shapes and their tessellation into triangles. Nothing here performs I/O.

mod atlas;
mod color;
mod display_list;
mod galley;
mod geometry;
pub mod input;
mod shape;
pub mod tessellator;
mod texture;

pub use atlas::{AtlasRegion, TextureAtlas};
pub use color::Color;
pub use display_list::{ClippedShape, DisplayList};
pub use galley::{Galley, GalleyRow, GlyphQuad};
pub use geometry::{PhysicalSize, Point, Rect, Vec2, point, vec2};
pub use input::{Event, ImeEvent, InputState, Key, Modifiers, PointerButton, RawInput};
pub use shape::{Shape, Stroke};
pub use tessellator::{ClippedMesh, Mesh, Tessellator, Vertex};
pub use texture::{ColorImage, TextureId, TexturesDelta};
