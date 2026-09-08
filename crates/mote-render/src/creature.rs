//! Shared pose contract for the twelve illustrated Mote species.
//!
//! Features all 12 vision board species (01–12) with distinct silhouettes,
//! appendages, color palettes, and facial expressions, alongside the three
//! signature spatial behaviors:
//! - Window-edge peeking ("01 peeks")
//! - Vertical border wall-climbing ("05 climbs")
//! - Curled ledge napping ("09 naps")
//!
//! Coordinates are in sprite pixels (256x256 canvas). Output is premultiplied
//! RGBA ready for Win32 `UpdateLayeredWindow`.

#![allow(clippy::too_many_arguments)]

use mote_core::SpeciesId;

pub const SPRITE_PX: usize = 256;
/// Feet position relative to canvas centre at radius 64 (scales linearly).
pub const FEET_BELOW_CENTER: f32 = 78.0;
const BASE_RADIUS: f32 = 64.0;

/// Full pose for one frame. Produced by [`crate::anim::Animator`].
#[derive(Debug, Clone)]
pub struct Pose {
    pub species: SpeciesId,
    /// Body squash/stretch (1 = rest). Landing ~ (1.25, 0.72).
    pub squash_x: f32,
    pub squash_y: f32,
    /// Whole-body tilt, radians (+ = clockwise).
    pub tilt: f32,
    /// Vertical hop offset in px (dance bounce, landing dip, peeking tuck).
    pub hop_px: f32,
    /// Eye look direction, -1..=1 (screen space: +x right, +y down).
    pub look_x: f32,
    pub look_y: f32,
    /// 0 = open, 1 = fully shut (blink / sleep).
    pub eyelid: f32,
    /// Eye widen factor (startle).
    pub eye_wide: f32,
    pub mouth: Mouth,
    /// -1..=1 lean into look direction (curiosity).
    pub lean: f32,
    /// Walk cycle phase, radians; `None` when still.
    pub step_phase: Option<f32>,
    pub step_amp: f32,
    /// Ear/antenna wiggle phase.
    pub ear_phase: f32,
    /// 0..=1 sleep bubbles + slow breathing.
    pub sleep_amount: f32,
    /// 0..=1 blush (petted / excited / dancing).
    pub blush: f32,
    /// 0..=1 flattened "ugh, heavy load" factor.
    pub wilt: f32,
    /// Creature radius in px (size setting).
    pub radius: f32,
    /// Time in seconds (drives breathing, bubbles, glyphs).
    pub time_s: f32,
    /// True when tucked behind a window edge ("01 peeks").
    pub is_peeking: bool,
    /// True when clinging to a vertical border ("05 climbs").
    pub is_climbing: bool,
    /// Climbing crawl phase (radians).
    pub climb_phase: f32,
    /// True when curled in sleeping posture ("09 naps").
    pub is_napping: bool,
    /// Facing direction: -1 left, +1 right.
    pub facing: i8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mouth {
    /// Tiny content line.
    Small,
    Smile,
    /// Startled "o".
    Oh,
    /// Wavy "this CPU load offends me".
    Wavy,
    Hidden,
}

impl Default for Pose {
    fn default() -> Self {
        Self {
            species: SpeciesId::Peeker,
            squash_x: 1.0,
            squash_y: 1.0,
            tilt: 0.0,
            hop_px: 0.0,
            look_x: 0.0,
            look_y: 0.0,
            eyelid: 0.0,
            eye_wide: 0.0,
            mouth: Mouth::Small,
            lean: 0.0,
            step_phase: None,
            step_amp: 0.0,
            ear_phase: 0.0,
            sleep_amount: 0.0,
            blush: 0.0,
            wilt: 0.0,
            radius: BASE_RADIUS,
            time_s: 0.0,
            is_peeking: false,
            is_climbing: false,
            climb_phase: 0.0,
            is_napping: false,
            facing: 1,
        }
    }
}

pub fn draw_mote(p: &Pose) -> Vec<u8> {
    crate::art::render(p)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_12_species_render_successfully() {
        for &species in SpeciesId::ALL.iter() {
            let p = Pose {
                species,
                ..Pose::default()
            };
            let buf = draw_mote(&p);
            assert_eq!(buf.len(), SPRITE_PX * SPRITE_PX * 4);
            let opaque = buf
                .as_chunks::<4>()
                .0
                .iter()
                .filter(|px| px[3] > 128)
                .count();
            assert!(
                opaque > 1000,
                "Species {:?} produced too few opaque pixels: {}",
                species,
                opaque
            );
        }
    }

    #[test]
    fn premultiplied_invariant_for_all_species() {
        for &species in SpeciesId::ALL.iter() {
            let p = Pose {
                species,
                mouth: Mouth::Smile,
                blush: 0.8,
                sleep_amount: 1.0,
                is_napping: true,
                ..Pose::default()
            };
            let buf = draw_mote(&p);
            for px in buf.as_chunks::<4>().0 {
                assert!(px[0] <= px[3], "R > A: {:?} on {:?}", px, species);
                assert!(px[1] <= px[3], "G > A: {:?} on {:?}", px, species);
                assert!(px[2] <= px[3], "B > A: {:?} on {:?}", px, species);
            }
        }
    }

    #[test]
    fn peeking_masks_lower_body() {
        let p = Pose {
            species: SpeciesId::Peeker,
            is_peeking: true,
            hop_px: 40.0,
            ..Pose::default()
        };
        let buf = draw_mote(&p);
        // Pixels near the very bottom (y > 220) should be clipped transparent
        let mut bottom_row_alpha: u32 = 0;
        for y in 220..255 {
            for x in 50..200 {
                bottom_row_alpha += buf[(y * SPRITE_PX + x) * 4 + 3] as u32;
            }
        }
        assert_eq!(
            bottom_row_alpha, 0,
            "Peeking must clip out the lower body behind window"
        );
    }

    #[test]
    fn climbing_renders_without_crash() {
        let p = Pose {
            species: SpeciesId::Climber,
            is_climbing: true,
            climb_phase: 1.57,
            tilt: -0.22,
            ..Pose::default()
        };
        let buf = draw_mote(&p);
        let opaque = buf
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|px| px[3] > 128)
            .count();
        assert!(opaque > 1000);
    }

    #[test]
    fn curled_napping_renders_zzz() {
        let p = Pose {
            species: SpeciesId::RingTail,
            is_napping: true,
            sleep_amount: 1.0,
            time_s: 2.0,
            ..Pose::default()
        };
        let buf = draw_mote(&p);
        assert_eq!(buf.len(), SPRITE_PX * SPRITE_PX * 4);
    }

    #[test]
    fn climbing_paws_orient_with_facing() {
        let mut p_right = Pose {
            species: SpeciesId::Climber,
            is_climbing: true,
            facing: 1,
            ..Pose::default()
        };
        let buf_right = draw_mote(&p_right);

        p_right.facing = -1;
        let buf_left = draw_mote(&p_right);

        // When facing right (+1), paws are on the right side (x > 175)
        // When facing left (-1), paws are on the left side (x < 80)
        let right_side_alpha_right: u32 = (100..200)
            .map(|y| buf_right[(y * SPRITE_PX + 180) * 4 + 3] as u32)
            .sum();
        let right_side_alpha_left: u32 = (100..200)
            .map(|y| buf_left[(y * SPRITE_PX + 180) * 4 + 3] as u32)
            .sum();
        assert!(
            right_side_alpha_right > right_side_alpha_left,
            "Paws must be placed on the right side when facing right"
        );
    }

    #[test]
    fn deterministic() {
        let p = Pose {
            species: SpeciesId::Pup,
            time_s: 1.234,
            step_phase: Some(0.7),
            ..Pose::default()
        };
        assert_eq!(draw_mote(&p), draw_mote(&p));
    }

    #[test]
    fn supported_sizes_and_stretches_keep_art_inside_canvas() {
        for &species in SpeciesId::all() {
            for radius in [46.0, 64.0, 86.0] {
                for (squash_x, squash_y, tilt) in
                    [(1.0, 1.0, 0.0), (0.9, 1.2, 0.12), (1.3, 0.75, -0.12)]
                {
                    let p = Pose {
                        species,
                        radius,
                        squash_x,
                        squash_y,
                        tilt,
                        ..Default::default()
                    };
                    let frame = draw_mote(&p);
                    for n in 0..SPRITE_PX {
                        for (x, y) in [(n, 0), (n, SPRITE_PX - 1), (0, n), (SPRITE_PX - 1, n)] {
                            assert_eq!(frame[(y*SPRITE_PX+x)*4+3],0,"{species:?} radius={radius} pose={squash_x},{squash_y},{tilt} clips at {x},{y}");
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn ring_hole_is_transparent_and_feet_stay_at_contact_point() {
        let frame = draw_mote(&Pose {
            species: SpeciesId::RingTail,
            ..Default::default()
        });
        assert_eq!(
            frame[(73 * SPRITE_PX + 123) * 4 + 3],
            0,
            "ring must remain an actual hole"
        );
        for squash_y in [0.74, 1.0, 1.2] {
            let frame = draw_mote(&Pose {
                squash_y,
                ..Default::default()
            });
            let lowest = frame
                .as_chunks::<4>()
                .0
                .iter()
                .enumerate()
                .filter(|(_, px)| px[3] > 128)
                .map(|(i, _)| i / SPRITE_PX)
                .max()
                .unwrap();
            assert!(
                (204..=208).contains(&lowest),
                "feet drifted to {lowest} for squash {squash_y}"
            );
        }
    }
}
