//! The real animals available to Mote.
//!
//! Species are deliberately a small, stable data contract. The renderer can
//! use [`SpeciesId::asset_slug`] to find artwork while the simulation uses
//! [`SpeciesId::motion`] to keep each animal's movement believable.

use crate::personality::Personality;
use serde::{Deserialize, Serialize};

/// The six real-animal Mote species.
///
/// The numeric values are the new stable ids. The aliases retain the names
/// used by earlier settings files; serde maps those old archetypes to the
/// closest real animal on load.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[repr(u8)]
pub enum SpeciesId {
    /// A curious, agile climber that alternates between quiet observation and
    /// short bursts of play.
    #[default]
    #[serde(alias = "Peeker", alias = "Shadow")]
    Cat = 1,
    /// A social, expressive runner with brief, playful pursuits.
    #[serde(alias = "Pup", alias = "Gourd")]
    Dog = 2,
    /// A gentle, alert hopper that prefers open surfaces and safe landings.
    #[serde(alias = "Seedling", alias = "Peanut")]
    Rabbit = 3,
    /// A nimble opportunist that can climb low window edges and nap in cover.
    #[serde(alias = "RingTail", alias = "Climber")]
    Fox = 4,
    /// A watchful nocturnal bird whose grounded movement is a quiet hop.
    #[serde(alias = "Heavy")]
    Owl = 5,
    /// A patient, low-profile wanderer. Tortoises do not voluntarily climb,
    /// chase or jump.
    #[serde(alias = "Loaf", alias = "Sprout", alias = "Kaiju")]
    Tortoise = 6,
}

/// How an animal moves around the desktop. This is separate from art so
/// behaviour and rendering can share grounded physical limits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Locomotion {
    /// Four-footed or feline walking, with an optional faster run.
    Walk,
    /// A two-footed hop between stable surfaces.
    Hop,
    /// A low, unhurried crawl.
    Crawl,
}

/// Physical and behavioural movement traits for a species.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MotionTraits {
    pub locomotion: Locomotion,
    pub can_run: bool,
    pub can_jump: bool,
    pub can_climb: bool,
    pub can_chase_cursor: bool,
    /// Multiplier applied to the shared walk speed.
    pub speed_scale: f32,
    /// Multiplier applied to the shared jump impulse.
    pub jump_scale: f32,
    /// Voluntary horizontal reach for a hop or jump, in pixels.
    pub jump_reach_px: f32,
    /// Voluntary upward reach for a hop or jump, in pixels.
    pub jump_height_px: f32,
}

impl SpeciesId {
    /// All six species in the order shown by the picker UI.
    pub const ALL: [SpeciesId; 6] = [
        SpeciesId::Cat,
        SpeciesId::Dog,
        SpeciesId::Rabbit,
        SpeciesId::Fox,
        SpeciesId::Owl,
        SpeciesId::Tortoise,
    ];

    pub fn all() -> &'static [SpeciesId] {
        &Self::ALL
    }

    /// Stable 1-based id.
    pub fn index(self) -> u8 {
        self as u8
    }

    /// Zero-padded picker id ("01" .. "06").
    pub fn id_str(self) -> &'static str {
        match self {
            SpeciesId::Cat => "01",
            SpeciesId::Dog => "02",
            SpeciesId::Rabbit => "03",
            SpeciesId::Fox => "04",
            SpeciesId::Owl => "05",
            SpeciesId::Tortoise => "06",
        }
    }

    /// Short display name.
    pub fn name(self) -> &'static str {
        match self {
            SpeciesId::Cat => "Cat",
            SpeciesId::Dog => "Dog",
            SpeciesId::Rabbit => "Rabbit",
            SpeciesId::Fox => "Fox",
            SpeciesId::Owl => "Owl",
            SpeciesId::Tortoise => "Tortoise",
        }
    }

    /// Numbered display name used by the picker.
    pub fn full_name(self) -> &'static str {
        match self {
            SpeciesId::Cat => "01 Cat",
            SpeciesId::Dog => "02 Dog",
            SpeciesId::Rabbit => "03 Rabbit",
            SpeciesId::Fox => "04 Fox",
            SpeciesId::Owl => "05 Owl",
            SpeciesId::Tortoise => "06 Tortoise",
        }
    }

    /// Human-facing species summary suitable for a picker card.
    pub fn description(self) -> &'static str {
        match self {
            SpeciesId::Cat => "Curious, light-footed company for a quiet desktop.",
            SpeciesId::Dog => "A sociable little explorer with a confident trot.",
            SpeciesId::Rabbit => "Soft, alert, and happiest travelling in little hops.",
            SpeciesId::Fox => "Nimble and curious, with a fondness for calm naps.",
            SpeciesId::Owl => "Patient and watchful, with small, precise hops.",
            SpeciesId::Tortoise => "A slow little explorer. Always stays grounded.",
        }
    }

    /// Stable filesystem-friendly artwork key.
    pub fn asset_slug(self) -> &'static str {
        match self {
            SpeciesId::Cat => "cat",
            SpeciesId::Dog => "dog",
            SpeciesId::Rabbit => "rabbit",
            SpeciesId::Fox => "fox",
            SpeciesId::Owl => "owl",
            SpeciesId::Tortoise => "tortoise",
        }
    }

    /// Parse a new stable 1-based id (1..=6).
    pub fn from_index(idx: u8) -> Option<Self> {
        match idx {
            1 => Some(SpeciesId::Cat),
            2 => Some(SpeciesId::Dog),
            3 => Some(SpeciesId::Rabbit),
            4 => Some(SpeciesId::Fox),
            5 => Some(SpeciesId::Owl),
            6 => Some(SpeciesId::Tortoise),
            _ => None,
        }
    }

    /// Grounded movement capabilities used by the behaviour brain.
    pub fn motion(self) -> MotionTraits {
        match self {
            SpeciesId::Cat => MotionTraits {
                locomotion: Locomotion::Walk,
                can_run: true,
                can_jump: true,
                can_climb: true,
                can_chase_cursor: true,
                speed_scale: 0.95,
                jump_scale: 1.0,
                jump_reach_px: 300.0,
                jump_height_px: 165.0,
            },
            SpeciesId::Dog => MotionTraits {
                locomotion: Locomotion::Walk,
                can_run: true,
                can_jump: true,
                can_climb: false,
                can_chase_cursor: true,
                speed_scale: 1.05,
                jump_scale: 0.9,
                jump_reach_px: 220.0,
                jump_height_px: 110.0,
            },
            SpeciesId::Rabbit => MotionTraits {
                locomotion: Locomotion::Hop,
                can_run: false,
                can_jump: true,
                can_climb: false,
                can_chase_cursor: false,
                speed_scale: 0.72,
                jump_scale: 0.85,
                jump_reach_px: 155.0,
                jump_height_px: 95.0,
            },
            SpeciesId::Fox => MotionTraits {
                locomotion: Locomotion::Walk,
                can_run: true,
                can_jump: true,
                can_climb: true,
                can_chase_cursor: true,
                speed_scale: 1.0,
                jump_scale: 0.95,
                jump_reach_px: 240.0,
                jump_height_px: 130.0,
            },
            SpeciesId::Owl => MotionTraits {
                locomotion: Locomotion::Hop,
                can_run: false,
                can_jump: true,
                can_climb: false,
                can_chase_cursor: false,
                speed_scale: 0.46,
                jump_scale: 0.55,
                jump_reach_px: 85.0,
                jump_height_px: 45.0,
            },
            SpeciesId::Tortoise => MotionTraits {
                locomotion: Locomotion::Crawl,
                can_run: false,
                can_jump: false,
                can_climb: false,
                can_chase_cursor: false,
                speed_scale: 0.28,
                jump_scale: 0.0,
                jump_reach_px: 0.0,
                jump_height_px: 0.0,
            },
        }
    }

    /// Default baseline personality, tuned to the real animal's temperament.
    pub fn default_personality(self) -> Personality {
        match self {
            SpeciesId::Cat => Personality {
                boldness: 0.58,
                laziness: 0.62,
                playfulness: 0.52,
                music_love: 0.55,
                skittishness: 0.62,
            },
            SpeciesId::Dog => Personality {
                boldness: 0.78,
                laziness: 0.32,
                playfulness: 0.86,
                music_love: 0.70,
                skittishness: 0.38,
            },
            SpeciesId::Rabbit => Personality {
                boldness: 0.38,
                laziness: 0.48,
                playfulness: 0.58,
                music_love: 0.38,
                skittishness: 0.82,
            },
            SpeciesId::Fox => Personality {
                boldness: 0.72,
                laziness: 0.46,
                playfulness: 0.60,
                music_love: 0.42,
                skittishness: 0.48,
            },
            SpeciesId::Owl => Personality {
                boldness: 0.44,
                laziness: 0.72,
                playfulness: 0.24,
                music_love: 0.30,
                skittishness: 0.28,
            },
            SpeciesId::Tortoise => Personality {
                boldness: 0.18,
                laziness: 0.92,
                playfulness: 0.12,
                music_love: 0.18,
                skittishness: 0.12,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn six_real_species_have_stable_metadata() {
        assert_eq!(SpeciesId::ALL.len(), 6);
        for (i, &species) in SpeciesId::ALL.iter().enumerate() {
            assert_eq!(species.index() as usize, i + 1);
            assert_eq!(SpeciesId::from_index((i + 1) as u8), Some(species));
            assert!(!species.name().is_empty());
            assert!(species.full_name().starts_with(species.id_str()));
            assert!(!species.description().is_empty());
            assert!(!species.asset_slug().is_empty());
            let p = species.default_personality();
            assert!((0.0..=1.0).contains(&p.boldness));
            assert!((0.0..=1.0).contains(&p.laziness));
            assert!((0.0..=1.0).contains(&p.playfulness));
            assert!((0.0..=1.0).contains(&p.music_love));
            assert!((0.0..=1.0).contains(&p.skittishness));
        }
    }

    #[test]
    fn old_species_names_deserialise_to_real_animals() {
        let cases = [
            ("Peeker", SpeciesId::Cat),
            ("Shadow", SpeciesId::Cat),
            ("Pup", SpeciesId::Dog),
            ("Gourd", SpeciesId::Dog),
            ("Seedling", SpeciesId::Rabbit),
            ("Peanut", SpeciesId::Rabbit),
            ("RingTail", SpeciesId::Fox),
            ("Climber", SpeciesId::Fox),
            ("Heavy", SpeciesId::Owl),
            ("Loaf", SpeciesId::Tortoise),
            ("Sprout", SpeciesId::Tortoise),
            ("Kaiju", SpeciesId::Tortoise),
        ];
        for (old, expected) in cases {
            let json = format!("\"{old}\"");
            assert_eq!(serde_json::from_str::<SpeciesId>(&json).unwrap(), expected);
        }
    }

    #[test]
    fn tortoise_is_strictly_grounded_and_owl_hops() {
        let tortoise = SpeciesId::Tortoise.motion();
        assert!(!tortoise.can_run && !tortoise.can_jump && !tortoise.can_climb);
        assert!(!tortoise.can_chase_cursor);
        assert_eq!(tortoise.locomotion, Locomotion::Crawl);
        let owl = SpeciesId::Owl.motion();
        assert_eq!(owl.locomotion, Locomotion::Hop);
        assert!(!owl.can_run);
        assert!(owl.jump_reach_px < 100.0 && owl.jump_height_px < 50.0);
    }
}
