//! Personality model: slow-moving internal drives + fixed traits.
//!
//! Drives influence behaviour weights but never create obligations — Mote
//! never needs to be "kept alive". All updates are dt-based and
//! deterministic.

use serde::{Deserialize, Serialize};

/// Internal drives, all 0..=1.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Drives {
    /// Low energy -> sits, sleeps more. Restored by sleeping/sitting.
    pub energy: f32,
    /// High curiosity -> examines windows, approaches cursor slowly.
    pub curiosity: f32,
    /// High boredom -> seeks novelty (wander, jump, chase).
    pub boredom: f32,
    /// Low comfort (loud load / chaos) -> hides on quiet surfaces.
    pub comfort: f32,
    /// Spikes with music, cursor play, successful jumps.
    pub excitement: f32,
    /// Rises with idle time; high -> sleep.
    pub sleepiness: f32,
    /// Spikes on startle / heavy CPU; decays otherwise.
    pub stress: f32,
}

impl Default for Drives {
    fn default() -> Self {
        Self {
            energy: 0.8,
            curiosity: 0.6,
            boredom: 0.2,
            comfort: 0.8,
            excitement: 0.2,
            sleepiness: 0.0,
            stress: 0.0,
        }
    }
}

impl Drives {
    /// Advance all drives by `dt` seconds. One cohesive update (many
    /// parameters by design — they arrive together every tick).
    #[allow(clippy::too_many_arguments)]
    pub fn update(
        &mut self,
        dt: f32,
        idle_s: f32,
        user_active: bool,
        media_playing: bool,
        cpu_01: f32,
        sleeping: bool,
        p: &Personality,
    ) {
        // Energy: burns while awake, restores while asleep.
        if sleeping {
            self.energy = (self.energy + dt * 0.06).min(1.0);
            self.sleepiness = (self.sleepiness - dt * 0.08).max(0.0);
        } else {
            let burn = 0.006 * (1.0 + (1.0 - p.laziness) * 0.8);
            self.energy = (self.energy - dt * burn).max(0.05);
            // Sleepiness grows with sustained idle, faster at "night".
            self.sleepiness =
                (self.sleepiness + dt * 0.004 * (1.0 + idle_s.min(600.0) / 300.0)).min(1.0);
        }
        // Boredom grows when nothing happens, relieved by activity.
        if user_active || media_playing {
            self.boredom = (self.boredom - dt * 0.02).max(0.0);
        } else {
            self.boredom = (self.boredom + dt * 0.006 * (0.5 + p.playfulness)).min(1.0);
        }
        // Curiosity oscillates slowly; novelty (user activity) feeds it.
        if user_active {
            self.curiosity = (self.curiosity + dt * 0.01).min(1.0);
        } else {
            self.curiosity = (self.curiosity - dt * 0.003).max(0.15);
        }
        // Comfort drops under heavy load, buffered by boldness.
        let target_comfort = 1.0 - cpu_01.clamp(0.0, 1.0) * (0.85 - p.boldness * 0.25);
        self.comfort += (target_comfort - self.comfort) * (dt * 0.5).min(1.0);
        // Excitement follows music + playfulness, decays fast.
        if media_playing {
            self.excitement = (self.excitement + dt * 0.15 * p.music_love).min(1.0);
        } else {
            self.excitement = (self.excitement - dt * 0.05).max(0.0);
        }
        // Stress follows CPU spikes, scaled by skittishness; decays otherwise.
        if cpu_01 > 0.75 {
            self.stress = (self.stress + dt * 0.25 * (0.4 + p.skittishness * 0.9)).min(1.0);
        } else {
            self.stress = (self.stress - dt * 0.06).max(0.0);
        }
    }

    pub fn spike(&mut self, excitement: f32, stress: f32) {
        self.excitement = (self.excitement + excitement).min(1.0);
        self.stress = (self.stress + stress).min(1.0);
    }
}

/// Fixed personality traits, 0..=1. Defaults = curious, cheeky, lazy-ish.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Personality {
    pub boldness: f32,
    pub laziness: f32,
    pub playfulness: f32,
    pub music_love: f32,
    pub skittishness: f32,
}

impl Default for Personality {
    fn default() -> Self {
        Self {
            boldness: 0.55,
            laziness: 0.55,
            playfulness: 0.65,
            music_love: 0.8,
            skittishness: 0.45,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sleep_restores_energy() {
        let mut d = Drives {
            energy: 0.2,
            ..Drives::default()
        };
        let p = Personality::default();
        for _ in 0..600 {
            d.update(0.1, 0.0, false, false, 0.1, true, &p);
        }
        assert!(d.energy > 0.5);
    }

    #[test]
    fn heavy_cpu_reduces_comfort_and_raises_stress() {
        let mut d = Drives::default();
        let p = Personality::default();
        for _ in 0..300 {
            d.update(0.1, 0.0, false, false, 0.95, false, &p);
        }
        assert!(d.comfort < 0.6);
        assert!(d.stress > 0.3);
    }
}
