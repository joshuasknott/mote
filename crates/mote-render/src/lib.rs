//! mote-render: the creature and its animation.
//!
//! Artist-authored realistic pet atlases are rasterised with tiny-skia.
//! [`Animator`] supplies foot-anchored squash, tilt, breathing and grounded
//! locomotion while the pose boundary keeps artwork independent of Windows.
//!
//! Output is premultiplied RGBA, ready for `UpdateLayeredWindow`.

pub mod anim;
pub mod creature;

pub use anim::{AnimInput, Animator};
pub use creature::{draw_mote, draw_portrait, FEET_BELOW_CENTER, SPRITE_PX};
