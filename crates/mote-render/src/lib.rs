//! mote-render: the creature and its animation.
//!
//! Twelve individually authored cubic silhouettes, ink outlines and pigment
//! texture are software-rastered with tiny-skia. [`Animator`] supplies foot-
//! anchored squash, whole-body tilt, gaze, blinks, steps and antenna sway.
//! The pose boundary keeps artwork independent of simulation and Windows.
//!
//! Output is premultiplied RGBA, ready for `UpdateLayeredWindow`.

pub mod anim;
pub mod creature;

pub use anim::{AnimInput, Animator};
pub use creature::{draw_mote, FEET_BELOW_CENTER, SPRITE_PX};
mod art;
