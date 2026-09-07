//! Species registry: all 12 Mote archetypes from the vision board.
//!
//! Each species defines a distinct visual silhouette and procedural features
//! (rendered in `mote-render`), paired with personality baseline traits
//! (boldness, laziness, playfulness, music_love, skittishness) that drive
//! its behaviour in `mote-core`.

use crate::personality::Personality;
use serde::{Deserialize, Serialize};

/// The 12 canonical Mote species from the vision board.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[repr(u8)]
pub enum SpeciesId {
    /// 01 The Peeker: Charcoal sphere, asymmetric horns, sly side-glance.
    #[default]
    Peeker = 1,
    /// 02 The Seedling: Warm cream teardrop, spiral curly antenna, dot eyes.
    Seedling = 2,
    /// 03 The Loaf: Terracotta slug, bulbous dorsal knob, sleepy half-lids.
    Loaf = 3,
    /// 04 The Gourd: Olive green tall body, floppy ear.
    Gourd = 4,
    /// 05 The Climber: Periwinkle bean, S-antenna, climbing limbs.
    Climber = 5,
    /// 06 The Heavy: Slate blue dome, floor-draping ear-arms.
    Heavy = 6,
    /// 07 The Peanut: Royal purple peanut body, stalk-bulb antenna.
    Peanut = 7,
    /// 08 The Bat / Pup: Cream & mottled brown, alert pointed ears, joyful paws.
    Pup = 8,
    /// 09 The Ring-tail: Amber round body, loop-handle antenna, curled ledge naps.
    RingTail = 9,
    /// 10 The Sprout: Moss green round body, triple-crest mushroom top.
    Sprout = 10,
    /// 11 The Shadow: Inky navy, sweeping scythe crest, wary gaze.
    Shadow = 11,
    /// 12 The Kaiju: Taupe quadruped, dorsal plates/spikes, peaceful eyes.
    Kaiju = 12,
}

impl SpeciesId {
    /// All 12 species in canonical vision board order.
    pub const ALL: [SpeciesId; 12] = [
        SpeciesId::Peeker,
        SpeciesId::Seedling,
        SpeciesId::Loaf,
        SpeciesId::Gourd,
        SpeciesId::Climber,
        SpeciesId::Heavy,
        SpeciesId::Peanut,
        SpeciesId::Pup,
        SpeciesId::RingTail,
        SpeciesId::Sprout,
        SpeciesId::Shadow,
        SpeciesId::Kaiju,
    ];

    pub fn all() -> &'static [SpeciesId] {
        &Self::ALL
    }

    /// 1-based index (1..=12).
    pub fn index(self) -> u8 {
        self as u8
    }

    /// Zero-padded two-digit string ("01" .. "12").
    pub fn id_str(self) -> &'static str {
        match self {
            SpeciesId::Peeker => "01",
            SpeciesId::Seedling => "02",
            SpeciesId::Loaf => "03",
            SpeciesId::Gourd => "04",
            SpeciesId::Climber => "05",
            SpeciesId::Heavy => "06",
            SpeciesId::Peanut => "07",
            SpeciesId::Pup => "08",
            SpeciesId::RingTail => "09",
            SpeciesId::Sprout => "10",
            SpeciesId::Shadow => "11",
            SpeciesId::Kaiju => "12",
        }
    }

    /// Short title name.
    pub fn name(self) -> &'static str {
        match self {
            SpeciesId::Peeker => "The Peeker",
            SpeciesId::Seedling => "The Seedling",
            SpeciesId::Loaf => "The Loaf",
            SpeciesId::Gourd => "The Gourd",
            SpeciesId::Climber => "The Climber",
            SpeciesId::Heavy => "The Heavy",
            SpeciesId::Peanut => "The Peanut",
            SpeciesId::Pup => "The Pup",
            SpeciesId::RingTail => "The Ring-tail",
            SpeciesId::Sprout => "The Sprout",
            SpeciesId::Shadow => "The Shadow",
            SpeciesId::Kaiju => "The Kaiju",
        }
    }

    /// Full display title with number prefix ("01 The Peeker").
    pub fn full_name(self) -> &'static str {
        match self {
            SpeciesId::Peeker => "01 The Peeker",
            SpeciesId::Seedling => "02 The Seedling",
            SpeciesId::Loaf => "03 The Loaf",
            SpeciesId::Gourd => "04 The Gourd",
            SpeciesId::Climber => "05 The Climber",
            SpeciesId::Heavy => "06 The Heavy",
            SpeciesId::Peanut => "07 The Peanut",
            SpeciesId::Pup => "08 The Pup",
            SpeciesId::RingTail => "09 The Ring-tail",
            SpeciesId::Sprout => "10 The Sprout",
            SpeciesId::Shadow => "11 The Shadow",
            SpeciesId::Kaiju => "12 The Kaiju",
        }
    }

    /// Short descriptive summary of features & personality.
    pub fn description(self) -> &'static str {
        match self {
            SpeciesId::Peeker => {
                "Dark charcoal sphere, asymmetric horns, loves window-edge peeking."
            }
            SpeciesId::Seedling => {
                "Warm cream teardrop, spiral curly antenna, sits and observes gently."
            }
            SpeciesId::Loaf => {
                "Terracotta slug, bulbous dorsal knob, heavy half-lids, slow & sleepy."
            }
            SpeciesId::Gourd => "Olive green tall body, floppy ear, playful bouncy cursor chaser.",
            SpeciesId::Climber => {
                "Periwinkle bean, S-antenna, agile limbs, climbs vertical window borders."
            }
            SpeciesId::Heavy => {
                "Slate blue dome, floor-draping ear-arms, calm rhythmic music bobber."
            }
            SpeciesId::Peanut => {
                "Royal purple peanut, glowing stalk antenna, delicate rhythmic dancer."
            }
            SpeciesId::Pup => {
                "Spotted cream & brown, alert pointed ears, high-energy cursor player."
            }
            SpeciesId::RingTail => {
                "Amber body, loop handle antenna, curls up on ledges for deep naps."
            }
            SpeciesId::Sprout => {
                "Moss green body, triple-bump crest, sturdy wanderer along taskbars."
            }
            SpeciesId::Shadow => {
                "Inky navy teardrop, scythe crest, wary gaze, skittish under heavy load."
            }
            SpeciesId::Kaiju => {
                "Taupe quadruped, dorsal plates, peaceful smile, completely unflappable."
            }
        }
    }

    /// Parse from 1-based index (1..=12).
    pub fn from_index(idx: u8) -> Option<Self> {
        match idx {
            1 => Some(SpeciesId::Peeker),
            2 => Some(SpeciesId::Seedling),
            3 => Some(SpeciesId::Loaf),
            4 => Some(SpeciesId::Gourd),
            5 => Some(SpeciesId::Climber),
            6 => Some(SpeciesId::Heavy),
            7 => Some(SpeciesId::Peanut),
            8 => Some(SpeciesId::Pup),
            9 => Some(SpeciesId::RingTail),
            10 => Some(SpeciesId::Sprout),
            11 => Some(SpeciesId::Shadow),
            12 => Some(SpeciesId::Kaiju),
            _ => None,
        }
    }

    /// Default baseline personality traits tuned for this species.
    pub fn default_personality(self) -> Personality {
        match self {
            // 01 The Peeker: High curiosity & skittishness, loves window-edge peeking.
            SpeciesId::Peeker => Personality {
                boldness: 0.50,
                laziness: 0.35,
                playfulness: 0.60,
                music_love: 0.70,
                skittishness: 0.85,
            },
            // 02 The Seedling: Gentle, high comfort, sits and watches.
            SpeciesId::Seedling => Personality {
                boldness: 0.30,
                laziness: 0.65,
                playfulness: 0.40,
                music_love: 0.75,
                skittishness: 0.30,
            },
            // 03 The Loaf: High laziness, low boredom, slow moving, long naps.
            SpeciesId::Loaf => Personality {
                boldness: 0.25,
                laziness: 0.95,
                playfulness: 0.20,
                music_love: 0.45,
                skittishness: 0.15,
            },
            // 04 The Gourd: High playfulness, clumsy hops, curious cursor chaser.
            SpeciesId::Gourd => Personality {
                boldness: 0.75,
                laziness: 0.30,
                playfulness: 0.90,
                music_love: 0.80,
                skittishness: 0.40,
            },
            // 05 The Climber: High boldness & agility, scrambles up window walls.
            SpeciesId::Climber => Personality {
                boldness: 0.95,
                laziness: 0.20,
                playfulness: 0.75,
                music_love: 0.60,
                skittishness: 0.35,
            },
            // 06 The Heavy: Grounded, low jump capability, calm music bobber.
            SpeciesId::Heavy => Personality {
                boldness: 0.40,
                laziness: 0.80,
                playfulness: 0.30,
                music_love: 0.95,
                skittishness: 0.20,
            },
            // 07 The Peanut: Dainty, observant, high music love (bounces rhythmically).
            SpeciesId::Peanut => Personality {
                boldness: 0.55,
                laziness: 0.45,
                playfulness: 0.70,
                music_love: 0.98,
                skittishness: 0.40,
            },
            // 08 The Pup: High energy, high playfulness, chases cursors eagerly.
            SpeciesId::Pup => Personality {
                boldness: 0.85,
                laziness: 0.15,
                playfulness: 0.95,
                music_love: 0.85,
                skittishness: 0.45,
            },
            // 09 The Ring-tail: High sleepiness, curls up on ledges for deep naps.
            SpeciesId::RingTail => Personality {
                boldness: 0.35,
                laziness: 0.90,
                playfulness: 0.25,
                music_love: 0.55,
                skittishness: 0.25,
            },
            // 10 The Sprout: Sturdy, idle explorer, wanders along taskbars.
            SpeciesId::Sprout => Personality {
                boldness: 0.65,
                laziness: 0.50,
                playfulness: 0.60,
                music_love: 0.70,
                skittishness: 0.30,
            },
            // 11 The Shadow: Skittish, guarded gaze, reacts to heavy load / fast cursor.
            SpeciesId::Shadow => Personality {
                boldness: 0.20,
                laziness: 0.40,
                playfulness: 0.30,
                music_love: 0.50,
                skittishness: 0.95,
            },
            // 12 The Kaiju: Unflappable, low stress response, peaceful.
            SpeciesId::Kaiju => Personality {
                boldness: 0.85,
                laziness: 0.70,
                playfulness: 0.40,
                music_love: 0.60,
                skittishness: 0.05,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_12_species_present_and_unique() {
        assert_eq!(SpeciesId::ALL.len(), 12);
        for (i, &s) in SpeciesId::ALL.iter().enumerate() {
            assert_eq!(s.index() as usize, i + 1);
            assert_eq!(SpeciesId::from_index((i + 1) as u8), Some(s));
            let p = s.default_personality();
            assert!(p.boldness >= 0.0 && p.boldness <= 1.0);
            assert!(p.laziness >= 0.0 && p.laziness <= 1.0);
            assert!(p.playfulness >= 0.0 && p.playfulness <= 1.0);
            assert!(p.music_love >= 0.0 && p.music_love <= 1.0);
            assert!(p.skittishness >= 0.0 && p.skittishness <= 1.0);
        }
    }

    #[test]
    fn personality_presets_match_archetypes() {
        // Loaf should be extremely lazy.
        assert!(SpeciesId::Loaf.default_personality().laziness > 0.9);
        // Pup should be extremely playful and not lazy.
        assert!(SpeciesId::Pup.default_personality().playfulness > 0.9);
        assert!(SpeciesId::Pup.default_personality().laziness < 0.2);
        // Climber should be bold.
        assert!(SpeciesId::Climber.default_personality().boldness > 0.9);
        // Shadow should be skittish.
        assert!(SpeciesId::Shadow.default_personality().skittishness > 0.9);
        // Kaiju should be unflappable (low skittishness).
        assert!(SpeciesId::Kaiju.default_personality().skittishness < 0.1);
    }
}
