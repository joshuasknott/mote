//! mote-render: the creature and its animation.
//!
//! Mote is drawn **procedurally** (software raster into an RGBA bitmap) for
//! now: a velvety indigo blob with big expressive eyes, nub ears and stubby
//! feet. The rendering pipeline is deliberately asset-agnostic — [`Animator`]
//! exposes pose parameters and frame timing, so future sprite-sheet art can
//! replace the procedural backend without touching behaviour or overlay code
//! (see `docs/ARCHITECTURE.md`).
//!
//! Output is premultiplied RGBA, ready for `UpdateLayeredWindow`.

pub mod anim;
pub mod creature;

pub use anim::{AnimInput, Animator};
pub use creature::{draw_mote, FEET_BELOW_CENTER, SPRITE_PX};
