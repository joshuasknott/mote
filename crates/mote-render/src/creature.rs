//! Procedural Mote sprite: software rasteriser + 12-creature morphology engine.
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

pub type Rgb = (u8, u8, u8);

/// Render one frame for the default species in the pose.
pub fn draw_mote(p: &Pose) -> Vec<u8> {
    draw_creature(p, p.species)
}

/// Render one frame for a specific species morphology.
pub fn draw_creature(p: &Pose, species: SpeciesId) -> Vec<u8> {
    let mut c = Canvas::new(SPRITE_PX, SPRITE_PX);
    let s = p.radius / BASE_RADIUS; // global scale factor
    let cx = SPRITE_PX as f32 / 2.0 + p.lean * 6.0 * s;
    let cy = SPRITE_PX as f32 / 2.0 + 22.0 * s + p.hop_px * s;

    let pal = species_palette(species);

    // Dynamic morphology sizing
    let wilt_droop = p.wilt * 8.0 * s;
    let (mut base_rx, mut base_ry) = match species {
        SpeciesId::Loaf => (p.radius * 1.28, p.radius * 0.82),
        SpeciesId::Gourd => (p.radius * 0.90, p.radius * 1.24),
        SpeciesId::Climber => (p.radius * 0.88, p.radius * 1.15),
        SpeciesId::Heavy => (p.radius * 1.22, p.radius * 0.96),
        SpeciesId::Peanut => (p.radius * 0.92, p.radius * 1.12),
        SpeciesId::Shadow => (p.radius * 0.92, p.radius * 1.12),
        SpeciesId::Kaiju => (p.radius * 1.22, p.radius * 0.90),
        SpeciesId::RingTail if p.is_napping || p.sleep_amount > 0.5 => {
            (p.radius * 1.26, p.radius * 0.72)
        }
        _ => (p.radius * 1.0, p.radius * 1.04),
    };

    if (p.is_napping || p.sleep_amount > 0.5)
        && species != SpeciesId::Loaf
        && species != SpeciesId::RingTail
    {
        base_rx = p.radius * 1.24;
        base_ry = p.radius * 0.74;
    }

    let rx = base_rx * p.squash_x;
    let ry = base_ry * p.squash_y + wilt_droop * 0.4;

    // 1. Appendages behind the body (ears, horns, antenna, dorsal spikes)
    draw_appendages_back(&mut c, cx, cy, rx, ry, s, p, species, &pal);

    // 2. Feet (behind body, except climbing paws which cling on sides)
    if !p.is_napping || p.sleep_amount < 0.3 {
        draw_feet(&mut c, cx, cy + ry * 0.92, rx, s, p, species, &pal);
    }

    // 3. Body silhouette with smooth 3D gradient, crown light, and belly patch
    draw_body(&mut c, cx, cy, rx, ry, s, p, species, &pal);

    // 4. Species-specific front markings / folded paws / spots
    draw_body_accents(&mut c, cx, cy, rx, ry, s, p, species, &pal);

    // 5. Face: eyes, eyelids, pupils, highlights, mouth
    draw_face(&mut c, cx, cy, rx, ry, s, p, species, &pal);

    // 6. Signature Behavior: Window-Edge Peeking ("01 peeks")
    // If peeking, mask out the lower body below the window ledge line!
    if p.is_peeking {
        let ledge_y = (SPRITE_PX as f32 / 2.0 + FEET_BELOW_CENTER * s).round() as i32;
        c.clip_below(ledge_y);
    }

    // 7. Signature Behavior: Curled Napping ("09 naps") & Sleep FX
    if p.sleep_amount > 0.01 {
        draw_sleep_fx(&mut c, cx + rx * 0.75, cy - ry * 0.65 - 12.0 * s, s, p);
    }

    c.buf
}

// ---------------------------------------------------------------------------
// Canvas: premultiplied RGBA with analytic anti-aliasing.
// ---------------------------------------------------------------------------

pub struct Canvas {
    pub w: usize,
    pub h: usize,
    pub buf: Vec<u8>,
}

impl Canvas {
    pub fn new(w: usize, h: usize) -> Self {
        Self {
            w,
            h,
            buf: vec![0; w * h * 4],
        }
    }

    /// Blend a premultiplied src colour over the pixel.
    pub fn blend(&mut self, x: i32, y: i32, r: u8, g: u8, b: u8, a01: f32) {
        if a01 <= 0.003 {
            return;
        }
        if x < 0 || y < 0 || x >= self.w as i32 || y >= self.h as i32 {
            return;
        }
        let a = a01.clamp(0.0, 1.0);
        let i = (y as usize * self.w + x as usize) * 4;
        let da = self.buf[i + 3] as f32 / 255.0;
        let out_a = a + da * (1.0 - a);
        if out_a <= 0.003 {
            return;
        }
        // src is premultiplied; dst stored premultiplied.
        let sr = r as f32 / 255.0 * a;
        let sg = g as f32 / 255.0 * a;
        let sb = b as f32 / 255.0 * a;
        self.buf[i] = ((sr + self.buf[i] as f32 / 255.0 * (1.0 - a)) * 255.0) as u8;
        self.buf[i + 1] = ((sg + self.buf[i + 1] as f32 / 255.0 * (1.0 - a)) * 255.0) as u8;
        self.buf[i + 2] = ((sb + self.buf[i + 2] as f32 / 255.0 * (1.0 - a)) * 255.0) as u8;
        self.buf[i + 3] = (out_a * 255.0) as u8;
    }

    /// Filled ellipse with 1.2px analytic anti-aliased edge.
    pub fn ellipse(&mut self, cx: f32, cy: f32, rx: f32, ry: f32, col: Rgb, alpha: f32) {
        self.ellipse_fn(cx, cy, rx, ry, &mut |_, _| (col, alpha));
    }

    pub fn ellipse_fn(
        &mut self,
        cx: f32,
        cy: f32,
        rx: f32,
        ry: f32,
        f: &mut dyn FnMut(f32, f32) -> (Rgb, f32),
    ) {
        if rx <= 0.5 || ry <= 0.5 {
            return;
        }
        let aa = 1.2;
        let x0 = (cx - rx - aa - 1.0).floor() as i32;
        let x1 = (cx + rx + aa + 1.0).ceil() as i32;
        let y0 = (cy - ry - aa - 1.0).floor() as i32;
        let y1 = (cy + ry + aa + 1.0).ceil() as i32;
        for y in y0..=y1 {
            for x in x0..=x1 {
                let dx = (x as f32 + 0.5 - cx) / rx;
                let dy = (y as f32 + 0.5 - cy) / ry;
                let d = (dx * dx + dy * dy).sqrt();
                let w = aa / rx.min(ry);
                let cov = ((1.0 - d) / w + 0.5).clamp(0.0, 1.0);
                if cov <= 0.004 {
                    continue;
                }
                let nx = (x as f32 + 0.5 - cx) / rx;
                let ny = (y as f32 + 0.5 - cy) / ry;
                let (col, a) = f(nx, ny);
                self.blend(x, y, col.0, col.1, col.2, a * cov);
            }
        }
    }

    /// Anti-aliased line capsule (line segment with rounded stroke radius).
    pub fn capsule(
        &mut self,
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
        radius: f32,
        col: Rgb,
        alpha: f32,
    ) {
        let min_x = (x0.min(x1) - radius - 2.0).floor() as i32;
        let max_x = (x0.max(x1) + radius + 2.0).ceil() as i32;
        let min_y = (y0.min(y1) - radius - 2.0).floor() as i32;
        let max_y = (y0.max(y1) + radius + 2.0).ceil() as i32;
        let dx = x1 - x0;
        let dy = y1 - y0;
        let l2 = dx * dx + dy * dy;
        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let px = x as f32 + 0.5 - x0;
                let py = y as f32 + 0.5 - y0;
                let t = if l2 > 0.001 {
                    (px * dx + py * dy) / l2
                } else {
                    0.0
                }
                .clamp(0.0, 1.0);
                let qx = x0 + t * dx;
                let qy = y0 + t * dy;
                let dist = ((x as f32 + 0.5 - qx).powi(2) + (y as f32 + 0.5 - qy).powi(2)).sqrt();
                let cov = ((radius + 0.6 - dist) / 1.2).clamp(0.0, 1.0);
                if cov > 0.003 {
                    self.blend(x, y, col.0, col.1, col.2, alpha * cov);
                }
            }
        }
    }

    /// Anti-aliased ring / loop handle (for 09 Ring-tail).
    pub fn fill_ring(
        &mut self,
        cx: f32,
        cy: f32,
        r_outer: f32,
        r_inner: f32,
        col: Rgb,
        alpha: f32,
    ) {
        let x0 = (cx - r_outer - 2.0).floor() as i32;
        let x1 = (cx + r_outer + 2.0).ceil() as i32;
        let y0 = (cy - r_outer - 2.0).floor() as i32;
        let y1 = (cy + r_outer + 2.0).ceil() as i32;
        for y in y0..=y1 {
            for x in x0..=x1 {
                let dx = x as f32 + 0.5 - cx;
                let dy = y as f32 + 0.5 - cy;
                let d = (dx * dx + dy * dy).sqrt();
                let cov_outer = ((r_outer + 0.6 - d) / 1.2).clamp(0.0, 1.0);
                let cov_inner = ((d - (r_inner - 0.6)) / 1.2).clamp(0.0, 1.0);
                let cov = (cov_outer * cov_inner).clamp(0.0, 1.0);
                if cov > 0.003 {
                    self.blend(x, y, col.0, col.1, col.2, alpha * cov);
                }
            }
        }
    }

    /// Anti-aliased filled triangle (for horns and dorsal spikes).
    pub fn triangle(
        &mut self,
        p0: (f32, f32),
        p1: (f32, f32),
        p2: (f32, f32),
        col: Rgb,
        alpha: f32,
    ) {
        let min_x = (p0.0.min(p1.0).min(p2.0) - 2.0).floor() as i32;
        let max_x = (p0.0.max(p1.0).max(p2.0) + 2.0).ceil() as i32;
        let min_y = (p0.1.min(p1.1).min(p2.1) - 2.0).floor() as i32;
        let max_y = (p0.1.max(p1.1).max(p2.1) + 2.0).ceil() as i32;
        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let px = x as f32 + 0.5;
                let py = y as f32 + 0.5;
                let w0 = (p1.0 - p0.0) * (py - p0.1) - (p1.1 - p0.1) * (px - p0.0);
                let w1 = (p2.0 - p1.0) * (py - p1.1) - (p2.1 - p1.1) * (px - p1.0);
                let w2 = (p0.0 - p2.0) * (py - p2.1) - (p0.1 - p2.1) * (px - p2.0);
                if (w0 >= 0.0 && w1 >= 0.0 && w2 >= 0.0) || (w0 <= 0.0 && w1 <= 0.0 && w2 <= 0.0) {
                    self.blend(x, y, col.0, col.1, col.2, alpha);
                }
            }
        }
    }

    /// Draw procedural 'z' glyph for sleeping animation.
    pub fn draw_z_glyph(&mut self, x: f32, y: f32, size: f32, col: Rgb, alpha: f32) {
        let h = size;
        let w = size * 0.75;
        let th = (size * 0.22).max(1.5);
        // Top horizontal bar
        self.capsule(
            x - w * 0.5,
            y - h * 0.5,
            x + w * 0.5,
            y - h * 0.5,
            th * 0.5,
            col,
            alpha,
        );
        // Diagonal bar
        self.capsule(
            x + w * 0.5,
            y - h * 0.5,
            x - w * 0.5,
            y + h * 0.5,
            th * 0.5,
            col,
            alpha,
        );
        // Bottom horizontal bar
        self.capsule(
            x - w * 0.5,
            y + h * 0.5,
            x + w * 0.5,
            y + h * 0.5,
            th * 0.5,
            col,
            alpha,
        );
    }

    /// Clear all pixels strictly below `y_cutoff` (for peeking behind windows).
    pub fn clip_below(&mut self, y_cutoff: i32) {
        let y_start = y_cutoff.clamp(0, self.h as i32) as usize;
        for y in y_start..self.h {
            let offset = y * self.w * 4;
            self.buf[offset..offset + self.w * 4].fill(0);
        }
    }
}

// ---------------------------------------------------------------------------
// Species Palettes
// ---------------------------------------------------------------------------

pub struct SpeciesPalette {
    pub top: Rgb,
    pub mid: Rgb,
    pub bot: Rgb,
    pub belly: Rgb,
    pub ear: Rgb,
    pub accent: Rgb,
    pub blush: Rgb,
    pub eye_white: Rgb,
    pub pupil: Rgb,
}

pub fn species_palette(species: SpeciesId) -> SpeciesPalette {
    match species {
        // 01 The Peeker: Charcoal sphere with silver-gray horns
        SpeciesId::Peeker => SpeciesPalette {
            top: (72, 74, 86),
            mid: (42, 44, 54),
            bot: (22, 24, 32),
            belly: (68, 72, 88),
            ear: (50, 52, 64),
            accent: (130, 136, 155),
            blush: (210, 120, 140),
            eye_white: (245, 245, 250),
            pupil: (22, 22, 30),
        },
        // 02 The Seedling: Warm cream teardrop with pale sprout antenna
        SpeciesId::Seedling => SpeciesPalette {
            top: (252, 246, 232),
            mid: (238, 226, 204),
            bot: (205, 190, 165),
            belly: (255, 252, 246),
            ear: (180, 198, 148),
            accent: (150, 178, 120),
            blush: (248, 165, 168),
            eye_white: (255, 255, 255),
            pupil: (45, 38, 35),
        },
        // 03 The Loaf: Terracotta slug with clay-red dorsal knob
        SpeciesId::Loaf => SpeciesPalette {
            top: (224, 125, 95),
            mid: (190, 95, 68),
            bot: (145, 65, 45),
            belly: (238, 155, 125),
            ear: (165, 75, 50),
            accent: (175, 80, 55),
            blush: (250, 130, 130),
            eye_white: (248, 240, 235),
            pupil: (55, 28, 22),
        },
        // 04 The Gourd: Olive green tall body with floppy ear
        SpeciesId::Gourd => SpeciesPalette {
            top: (160, 178, 92),
            mid: (126, 144, 66),
            bot: (88, 104, 42),
            belly: (182, 202, 118),
            ear: (112, 128, 55),
            accent: (145, 165, 75),
            blush: (235, 150, 135),
            eye_white: (246, 248, 242),
            pupil: (32, 40, 20),
        },
        // 05 The Climber: Silver-periwinkle slender bean
        SpeciesId::Climber => SpeciesPalette {
            top: (195, 202, 235),
            mid: (158, 166, 210),
            bot: (122, 130, 180),
            belly: (218, 224, 248),
            ear: (138, 145, 195),
            accent: (110, 118, 170),
            blush: (235, 145, 175),
            eye_white: (248, 248, 255),
            pupil: (40, 44, 75),
        },
        // 06 The Heavy: Slate blue dome with floor-draping ear-arms
        SpeciesId::Heavy => SpeciesPalette {
            top: (118, 138, 172),
            mid: (84, 105, 140),
            bot: (56, 74, 108),
            belly: (145, 166, 200),
            ear: (68, 88, 122),
            accent: (95, 115, 150),
            blush: (215, 135, 160),
            eye_white: (242, 245, 250),
            pupil: (28, 36, 52),
        },
        // 07 The Peanut: Royal purple figure-eight with luminous bulb
        SpeciesId::Peanut => SpeciesPalette {
            top: (148, 80, 195),
            mid: (110, 50, 158),
            bot: (78, 32, 122),
            belly: (180, 120, 220),
            ear: (90, 40, 135),
            accent: (255, 245, 160),
            blush: (240, 130, 170),
            eye_white: (248, 242, 252),
            pupil: (35, 18, 55),
        },
        // 08 The Pup: Cream & mottled brown with alert pointed bat ears
        SpeciesId::Pup => SpeciesPalette {
            top: (242, 225, 200),
            mid: (212, 185, 155),
            bot: (170, 135, 105),
            belly: (255, 245, 230),
            ear: (115, 78, 55),
            accent: (115, 78, 55),
            blush: (245, 135, 145),
            eye_white: (252, 250, 246),
            pupil: (42, 28, 20),
        },
        // 09 The Ring-tail: Amber round body with loop-handle ring antenna
        SpeciesId::RingTail => SpeciesPalette {
            top: (245, 185, 70),
            mid: (215, 148, 40),
            bot: (175, 110, 25),
            belly: (255, 215, 125),
            ear: (190, 125, 30),
            accent: (200, 135, 35),
            blush: (250, 140, 130),
            eye_white: (255, 250, 240),
            pupil: (50, 32, 12),
        },
        // 10 The Sprout: Moss green round body with triple-crest mushroom top
        SpeciesId::Sprout => SpeciesPalette {
            top: (125, 172, 105),
            mid: (92, 138, 74),
            bot: (65, 104, 52),
            belly: (160, 202, 140),
            ear: (75, 118, 58),
            accent: (85, 130, 68),
            blush: (240, 150, 150),
            eye_white: (245, 250, 242),
            pupil: (25, 42, 20),
        },
        // 11 The Shadow: Inky navy teardrop with sweeping scythe crest
        SpeciesId::Shadow => SpeciesPalette {
            top: (48, 54, 92),
            mid: (32, 38, 70),
            bot: (18, 22, 48),
            belly: (62, 68, 112),
            ear: (28, 32, 60),
            accent: (115, 135, 195),
            blush: (195, 105, 145),
            eye_white: (210, 230, 245),
            pupil: (15, 18, 38),
        },
        // 12 The Kaiju: Taupe quadruped with dorsal plates
        SpeciesId::Kaiju => SpeciesPalette {
            top: (172, 162, 152),
            mid: (140, 130, 120),
            bot: (110, 100, 90),
            belly: (202, 192, 182),
            ear: (115, 105, 95),
            accent: (120, 110, 100),
            blush: (225, 145, 145),
            eye_white: (245, 242, 238),
            pupil: (40, 35, 30),
        },
    }
}

// ---------------------------------------------------------------------------
// Body Morphology Drawing
// ---------------------------------------------------------------------------

fn draw_body(
    c: &mut Canvas,
    cx: f32,
    cy: f32,
    rx: f32,
    ry: f32,
    _s: f32,
    p: &Pose,
    species: SpeciesId,
    pal: &SpeciesPalette,
) {
    let top = pal.top;
    let mid = pal.mid;
    let bot = pal.bot;

    // Body base with vertical lighting + crown specular glow
    let draw_shaded_ellipse = |c: &mut Canvas, ecx: f32, ecy: f32, erx: f32, ery: f32| {
        c.ellipse_fn(ecx, ecy, erx, ery, &mut |nx, ny| {
            let t = ((ny + 1.0) * 0.5).clamp(0.0, 1.0);
            let (mut r, mut g, mut b) = if t < 0.5 {
                let k = t * 2.0;
                (
                    top.0 as f32 + (mid.0 as f32 - top.0 as f32) * k,
                    top.1 as f32 + (mid.1 as f32 - top.1 as f32) * k,
                    top.2 as f32 + (mid.2 as f32 - top.2 as f32) * k,
                )
            } else {
                let k = (t - 0.5) * 2.0;
                (
                    mid.0 as f32 + (bot.0 as f32 - mid.0 as f32) * k,
                    mid.1 as f32 + (bot.1 as f32 - mid.1 as f32) * k,
                    mid.2 as f32 + (bot.2 as f32 - mid.2 as f32) * k,
                )
            };
            // Specular crown light
            let lx = (nx + 0.40) * 0.8;
            let ly = (ny + 0.50) * 0.9;
            let glow = (1.0 - (lx * lx + ly * ly)).max(0.0).powi(2) * 32.0;
            r += glow;
            g += glow * 0.96;
            b += glow * 1.05;
            let shade = 1.0 - p.tilt.abs() * 0.30 - p.wilt * 0.12;
            (
                (
                    (r * shade).clamp(0.0, 255.0) as u8,
                    (g * shade).clamp(0.0, 255.0) as u8,
                    (b * shade).clamp(0.0, 255.0) as u8,
                ),
                1.0,
            )
        });
    };

    match species {
        // 07 The Peanut: two connected overlapping lobes
        SpeciesId::Peanut => {
            let top_ry = ry * 0.58;
            let bot_ry = ry * 0.65;
            let top_cy = cy - ry * 0.35;
            let bot_cy = cy + ry * 0.35;
            draw_shaded_ellipse(c, cx, bot_cy, rx * 0.95, bot_ry);
            draw_shaded_ellipse(c, cx, top_cy, rx * 0.82, top_ry);
        }
        // 04 The Gourd: pear silhouette
        SpeciesId::Gourd => {
            let bot_ry = ry * 0.68;
            let top_ry = ry * 0.52;
            draw_shaded_ellipse(c, cx, cy + ry * 0.30, rx * 1.02, bot_ry);
            draw_shaded_ellipse(c, cx, cy - ry * 0.36, rx * 0.76, top_ry);
        }
        _ => {
            draw_shaded_ellipse(c, cx, cy, rx, ry);
        }
    }

    // Belly patch: lighter tone lower center
    let brx = rx * 0.50;
    let bry = ry * 0.42;
    let bcy = cy + ry * 0.40;
    c.ellipse_fn(cx, bcy, brx, bry, &mut |_, _| (pal.belly, 0.55));

    // Blush when petted / excited / dancing
    if p.blush > 0.02 {
        let a = p.blush * 0.55;
        let bl_rx = rx * 0.16;
        let bl_ry = ry * 0.10;
        let bl_y = cy + ry * 0.18;
        c.ellipse(cx - rx * 0.50, bl_y, bl_rx, bl_ry, pal.blush, a);
        c.ellipse(cx + rx * 0.50, bl_y, bl_rx, bl_ry, pal.blush, a);
    }
}

// ---------------------------------------------------------------------------
// Appendages (Horns, Ears, Antennae, Spikes)
// ---------------------------------------------------------------------------

fn draw_appendages_back(
    c: &mut Canvas,
    cx: f32,
    cy: f32,
    rx: f32,
    ry: f32,
    s: f32,
    p: &Pose,
    species: SpeciesId,
    pal: &SpeciesPalette,
) {
    let top_y = cy - ry * 0.88;
    let wig = (p.ear_phase.sin() * 3.0 + p.tilt * 14.0) * s;

    match species {
        // 01 The Peeker: Asymmetric horns (one bent elbow, one straight pointed)
        SpeciesId::Peeker => {
            let base_w = 7.0 * s;
            // Left horn: bent elbow
            let lx0 = cx - rx * 0.42;
            let ly0 = top_y + 4.0 * s;
            let lx1 = cx - rx * 0.62;
            let ly1 = top_y - 20.0 * s + wig * 0.4;
            let lx2 = cx - rx * 0.48;
            let ly2 = top_y - 38.0 * s + wig * 0.4;
            c.capsule(lx0, ly0, lx1, ly1, base_w, pal.ear, 1.0);
            c.capsule(lx1, ly1, lx2, ly2, base_w * 0.75, pal.accent, 1.0);

            // Right horn: straight pointed horn
            let rx0 = cx + rx * 0.42;
            let ry0 = top_y + 4.0 * s;
            let rx1 = cx + rx * 0.52;
            let ry1 = top_y - 42.0 * s - wig * 0.4;
            c.capsule(rx0, ry0, rx1, ry1, base_w * 0.9, pal.ear, 1.0);
            c.triangle(
                (rx1 - 5.0 * s, ry1 + 6.0 * s),
                (rx1 + 5.0 * s, ry1 + 6.0 * s),
                (rx1, ry1 - 10.0 * s),
                pal.accent,
                1.0,
            );
        }
        // 02 The Seedling: Curly spiral antenna
        SpeciesId::Seedling => {
            let ax0 = cx;
            let ay0 = top_y + 2.0 * s;
            let ax1 = cx + wig * 0.5;
            let ay1 = top_y - 24.0 * s;
            let ax2 = cx + 12.0 * s + wig * 0.8;
            let ay2 = top_y - 32.0 * s;
            let ax3 = cx + 8.0 * s + wig * 0.8;
            let ay3 = top_y - 40.0 * s;
            c.capsule(ax0, ay0, ax1, ay1, 3.5 * s, pal.ear, 1.0);
            c.capsule(ax1, ay1, ax2, ay2, 3.0 * s, pal.accent, 1.0);
            c.capsule(ax2, ay2, ax3, ay3, 2.5 * s, pal.accent, 1.0);
            c.ellipse(ax3, ay3, 5.0 * s, 5.0 * s, pal.accent, 1.0);
        }
        // 03 The Loaf: Bulbous dorsal knob
        SpeciesId::Loaf => {
            let kx = cx - rx * 0.28;
            let ky = cy - ry * 0.76;
            c.ellipse(kx, ky, 18.0 * s, 16.0 * s, pal.accent, 1.0);
            c.ellipse(kx - 3.0 * s, ky - 3.0 * s, 8.0 * s, 7.0 * s, pal.top, 0.7);
        }
        // 04 The Gourd: Single floppy ear
        SpeciesId::Gourd => {
            let ex0 = cx + rx * 0.55;
            let ey0 = top_y + 12.0 * s;
            let ex1 = cx + rx * 0.85;
            let ey1 = top_y + 28.0 * s + wig * 0.5;
            c.capsule(ex0, ey0, ex1, ey1, 11.0 * s, pal.ear, 1.0);
            c.ellipse(ex1, ey1, 12.0 * s, 16.0 * s, pal.ear, 1.0);
            c.ellipse(ex1, ey1 + 2.0 * s, 6.0 * s, 9.0 * s, pal.accent, 0.8);
        }
        // 05 The Climber: S-curved antenna
        SpeciesId::Climber => {
            let ax0 = cx;
            let ay0 = top_y + 2.0 * s;
            let ax1 = cx - 10.0 * s + wig * 0.3;
            let ay1 = top_y - 18.0 * s;
            let ax2 = cx + 12.0 * s + wig * 0.6;
            let ay2 = top_y - 34.0 * s;
            c.capsule(ax0, ay0, ax1, ay1, 4.0 * s, pal.ear, 1.0);
            c.capsule(ax1, ay1, ax2, ay2, 3.2 * s, pal.accent, 1.0);
            c.ellipse(ax2, ay2, 6.0 * s, 6.0 * s, pal.accent, 1.0);
        }
        // 06 The Heavy: Sweeping floor-draping ear-arms
        SpeciesId::Heavy => {
            let arm_w = 14.0 * s;
            for side in [-1.0f32, 1.0] {
                let ax0 = cx + side * rx * 0.72;
                let ay0 = cy - ry * 0.10;
                let ax1 = cx + side * (rx * 0.88 + 8.0 * s);
                let ay1 = cy + ry * 0.82;
                c.capsule(ax0, ay0, ax1, ay1, arm_w, pal.ear, 1.0);
                c.ellipse(ax1, ay1, arm_w * 1.1, arm_w * 0.9, pal.bot, 1.0);
            }
        }
        // 07 The Peanut: Stalk antenna with luminous bulb tip
        SpeciesId::Peanut => {
            let sx0 = cx;
            let sy0 = top_y + 2.0 * s;
            let sx1 = cx + wig * 0.4;
            let sy1 = top_y - 32.0 * s;
            c.capsule(sx0, sy0, sx1, sy1, 3.2 * s, pal.ear, 1.0);
            // Glowing bulb tip
            c.ellipse(sx1, sy1, 9.0 * s, 9.0 * s, pal.accent, 1.0);
            c.ellipse(sx1, sy1, 14.0 * s, 14.0 * s, pal.accent, 0.35); // outer glow
        }
        // 08 The Pup: Alert pointed bat/pup ears
        SpeciesId::Pup => {
            let ew = 18.0 * s;
            let eh = 30.0 * s;
            for (side, sign) in [(-1.0f32, -1.0f32), (1.0, 1.0)] {
                let ex = cx + side * (rx * 0.60);
                let ey = top_y + wig * sign * 0.3;
                c.triangle(
                    (ex - ew * 0.7, ey + 4.0 * s),
                    (ex + ew * 0.7, ey + 4.0 * s),
                    (ex + side * 4.0 * s, ey - eh),
                    pal.ear,
                    1.0,
                );
                c.triangle(
                    (ex - ew * 0.4, ey + 2.0 * s),
                    (ex + ew * 0.4, ey + 2.0 * s),
                    (ex + side * 2.0 * s, ey - eh * 0.75),
                    (230, 160, 160),
                    0.85,
                );
            }
        }
        // 09 The Ring-tail: Circular ring antenna / loop handle (droops curled when napping)
        SpeciesId::RingTail => {
            let rcx = cx;
            let rcy = if p.is_napping || p.sleep_amount > 0.5 {
                top_y - 8.0 * s
            } else {
                top_y - 20.0 * s + wig * 0.3
            };
            c.capsule(
                cx,
                top_y + 2.0 * s,
                rcx,
                rcy + 10.0 * s,
                4.0 * s,
                pal.ear,
                1.0,
            );
            c.fill_ring(rcx, rcy, 22.0 * s, 14.0 * s, pal.accent, 1.0);
        }
        // 10 The Sprout: Triple-crest mushroom top
        SpeciesId::Sprout => {
            let cy_crest = top_y - 6.0 * s;
            c.ellipse(cx, cy_crest - 10.0 * s, 16.0 * s, 12.0 * s, pal.accent, 1.0);
            c.ellipse(
                cx - 18.0 * s,
                cy_crest - 4.0 * s,
                12.0 * s,
                9.0 * s,
                pal.ear,
                1.0,
            );
            c.ellipse(
                cx + 18.0 * s,
                cy_crest - 4.0 * s,
                12.0 * s,
                9.0 * s,
                pal.ear,
                1.0,
            );
        }
        // 11 The Shadow: Sweeping scythe horn crest
        SpeciesId::Shadow => {
            let hx0 = cx;
            let hy0 = top_y + 4.0 * s;
            let hx1 = cx - 22.0 * s;
            let hy1 = top_y - 24.0 * s;
            let hx2 = cx - 38.0 * s;
            let hy2 = top_y - 12.0 * s;
            c.capsule(hx0, hy0, hx1, hy1, 7.0 * s, pal.ear, 1.0);
            c.capsule(hx1, hy1, hx2, hy2, 4.5 * s, pal.accent, 1.0);
        }
        // 12 The Kaiju: Dorsal plates / spikes
        SpeciesId::Kaiju => {
            let num_spikes = 4;
            for i in 0..num_spikes {
                let frac = i as f32 / (num_spikes - 1) as f32;
                let sx = cx - rx * 0.70 + frac * rx * 1.40;
                let sy = top_y + (frac * 2.0 - 1.0).powi(2) * 8.0 * s;
                let h = 18.0 * s * (1.0 - (frac - 0.5).abs() * 0.8);
                c.triangle(
                    (sx - 7.0 * s, sy + 3.0 * s),
                    (sx + 7.0 * s, sy + 3.0 * s),
                    (sx, sy - h),
                    pal.accent,
                    1.0,
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Body Accents (Spots, Paws, Mottling)
// ---------------------------------------------------------------------------

fn draw_body_accents(
    c: &mut Canvas,
    cx: f32,
    cy: f32,
    rx: f32,
    ry: f32,
    s: f32,
    p: &Pose,
    species: SpeciesId,
    pal: &SpeciesPalette,
) {
    match species {
        // 08 The Pup: mottled brown spots on body & cheeks
        SpeciesId::Pup => {
            c.ellipse(
                cx - rx * 0.42,
                cy - ry * 0.20,
                10.0 * s,
                7.0 * s,
                pal.accent,
                0.85,
            );
            c.ellipse(
                cx + rx * 0.35,
                cy + ry * 0.15,
                12.0 * s,
                9.0 * s,
                pal.accent,
                0.85,
            );
            c.ellipse(
                cx + rx * 0.48,
                cy - ry * 0.35,
                7.0 * s,
                6.0 * s,
                pal.accent,
                0.85,
            );
        }
        // 07 The Peanut: folded front paws resting on chest
        SpeciesId::Peanut => {
            let paw_y = cy + ry * 0.10;
            c.ellipse(cx - 9.0 * s, paw_y, 7.0 * s, 5.0 * s, pal.ear, 0.95);
            c.ellipse(cx + 9.0 * s, paw_y, 7.0 * s, 5.0 * s, pal.ear, 0.95);
        }
        // 09 The Ring-tail: folded sleeping paws tucked cozy against chest when napping
        SpeciesId::RingTail if p.is_napping || p.sleep_amount > 0.4 => {
            let paw_y = cy + ry * 0.30;
            c.ellipse(cx - 12.0 * s, paw_y, 8.5 * s, 6.0 * s, pal.ear, 0.95);
            c.ellipse(cx + 12.0 * s, paw_y, 8.5 * s, 6.0 * s, pal.ear, 0.95);
            c.ellipse(cx - 12.0 * s, paw_y, 5.5 * s, 3.5 * s, pal.accent, 0.85);
            c.ellipse(cx + 12.0 * s, paw_y, 5.5 * s, 3.5 * s, pal.accent, 0.85);
        }
        // Climbing paws: grip window border on the side the creature faces
        _ if p.is_climbing => {
            let crawl = (p.climb_phase.sin() * 12.0) * s;
            let paw_dir = if p.facing != 0 { p.facing as f32 } else { 1.0 };
            let paw_x = cx + paw_dir * rx * 0.80;
            let col = if species == SpeciesId::Climber {
                pal.accent
            } else {
                pal.ear
            };
            c.ellipse(paw_x, cy - ry * 0.30 + crawl, 8.0 * s, 6.0 * s, col, 1.0);
            c.ellipse(paw_x, cy + ry * 0.40 - crawl, 8.0 * s, 6.0 * s, col, 1.0);
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// Feet Locomotion
// ---------------------------------------------------------------------------

fn draw_feet(
    c: &mut Canvas,
    cx: f32,
    feet_y: f32,
    rx: f32,
    s: f32,
    p: &Pose,
    species: SpeciesId,
    pal: &SpeciesPalette,
) {
    let (mut lo, mut ro) = (0.0, 0.0);
    if let Some(ph) = p.step_phase {
        lo = (ph.sin() * 5.0 * p.step_amp) * s;
        ro = (-ph.sin() * 5.0 * p.step_amp) * s;
    }

    match species {
        // 12 Kaiju: 4 sturdy quadruped feet
        SpeciesId::Kaiju => {
            let fx_front = rx * 0.65;
            let fx_rear = rx * 0.30;
            let fw = 13.0 * s;
            let fh = 9.0 * s;
            c.ellipse(cx - fx_front, feet_y + lo, fw, fh, pal.bot, 1.0);
            c.ellipse(cx - fx_rear, feet_y + ro * 0.5, fw, fh, pal.mid, 1.0);
            c.ellipse(cx + fx_rear, feet_y + lo * 0.5, fw, fh, pal.mid, 1.0);
            c.ellipse(cx + fx_front, feet_y + ro, fw, fh, pal.bot, 1.0);
        }
        _ => {
            let fx = (rx * 0.42).max(18.0 * s);
            let fw = 14.0 * s;
            let fh = 8.5 * s;
            c.ellipse(cx - fx, feet_y + lo, fw, fh, pal.bot, 1.0);
            c.ellipse(cx + fx, feet_y + ro, fw, fh, pal.bot, 1.0);
        }
    }
}

// ---------------------------------------------------------------------------
// Face: Eyes, Pupils, Mouth
// ---------------------------------------------------------------------------

fn draw_face(
    c: &mut Canvas,
    cx: f32,
    cy: f32,
    rx: f32,
    ry: f32,
    s: f32,
    p: &Pose,
    species: SpeciesId,
    pal: &SpeciesPalette,
) {
    let wide = 1.0 + p.eye_wide * 0.35;
    let ex = rx * 0.34;
    let ey = cy - ry * 0.12 + p.wilt * 4.0 * s;
    let erx = 20.0 * s * p.squash_x * wide;
    let mut ery = 25.0 * s * wide;

    // Eyelid droop baseline
    let mut eyelid = p.eyelid;
    if species == SpeciesId::Loaf {
        eyelid = (eyelid + 0.40).min(1.0); // 03 Loaf has natural heavy half-lids
    }
    ery *= (1.0 - eyelid * 0.92).max(0.08);

    for side in [-1.0f32, 1.0] {
        let exx = cx + side * ex + p.lean * 4.0 * s;

        // 12 Kaiju: serene closed crescent smile eyes (never opens wide)
        if species == SpeciesId::Kaiju {
            c.ellipse(exx, ey + 2.0 * s, erx * 0.8, 3.5 * s, pal.pupil, 0.95);
            continue;
        }

        // 02 Seedling: gentle tiny dot eyes
        if species == SpeciesId::Seedling {
            let dot_r = 4.5 * s;
            if eyelid > 0.75 {
                c.ellipse(exx, ey + 2.0 * s, dot_r * 1.2, 2.5 * s, pal.pupil, 0.95);
            } else {
                c.ellipse(
                    exx + p.look_x * 2.0 * s,
                    ey + p.look_y * 2.0 * s,
                    dot_r,
                    dot_r,
                    pal.pupil,
                    1.0,
                );
            }
            continue;
        }

        if eyelid > 0.75 {
            // Shut eye: soft dark curved arc
            c.ellipse(exx, ey + 4.0 * s, erx * 0.9, 4.2 * s, pal.pupil, 0.95);
        } else {
            // Sclera
            c.ellipse(exx, ey, erx, ery.max(3.0 * s), pal.eye_white, 1.0);

            // Pupil
            let px = exx + p.look_x * erx * 0.42;
            let py = ey + p.look_y * ery * 0.40;
            let pr = 10.0 * s * (1.0 + p.eye_wide * 0.25);
            c.ellipse_fn(px, py, pr, pr, &mut |_, _| (pal.pupil, 1.0));

            // Iris highlight / shine
            c.ellipse(
                px - pr * 0.32,
                py - pr * 0.36,
                pr * 0.32,
                pr * 0.32,
                (255, 255, 255),
                0.95,
            );
        }
    }

    // Mouth
    let my = cy + ry * 0.38;
    match p.mouth {
        Mouth::Hidden => {}
        Mouth::Small => {
            c.ellipse(cx, my, 5.0 * s, 2.6 * s, pal.pupil, 0.9);
        }
        Mouth::Smile => {
            c.ellipse(cx, my - 1.0 * s, 9.0 * s, 5.0 * s, pal.pupil, 0.9);
            c.ellipse(cx, my - 3.4 * s, 9.0 * s, 4.2 * s, pal.belly, 1.0);
        }
        Mouth::Oh => {
            c.ellipse(cx, my, 6.5 * s, 8.0 * s, pal.pupil, 0.95);
        }
        Mouth::Wavy => {
            c.ellipse(cx - 6.0 * s, my, 5.0 * s, 2.4 * s, pal.pupil, 0.9);
            c.ellipse(cx + 6.0 * s, my - 2.0 * s, 5.0 * s, 2.4 * s, pal.pupil, 0.9);
        }
    }
}

// ---------------------------------------------------------------------------
// Sleep FX & Procedural "z z z" Glyphs ("09 naps")
// ---------------------------------------------------------------------------

fn draw_sleep_fx(c: &mut Canvas, x: f32, y: f32, s: f32, p: &Pose) {
    // 1. Floating sleep bubbles
    for i in 0..3 {
        let ph = (p.time_s * 0.45 + i as f32 * 0.33) % 1.0;
        let bx = x + (ph * 24.0 - 8.0 * i as f32) * s;
        let by = y - ph * 40.0 * s;
        let r = (4.5 + i as f32 * 2.2 + ph * 3.0) * s;
        let a = p.sleep_amount * (1.0 - ph) * 0.50;
        c.ellipse(bx, by, r, r, (210, 205, 240), a);
    }

    // 2. Procedural "z z z" glyphs rising and swaying with breath
    for i in 0..3 {
        let ph = (p.time_s * 0.35 + i as f32 * 0.33) % 1.0;
        let zx = x + 10.0 * s + (ph.sin() * 8.0 + ph * 20.0) * s;
        let zy = y - ph * 50.0 * s;
        let size = (7.0 + i as f32 * 3.0) * s;
        let a = p.sleep_amount * (1.0 - ph) * 0.70;
        c.draw_z_glyph(zx, zy, size, (220, 215, 250), a);
    }
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
            let opaque = buf.chunks_exact(4).filter(|px| px[3] > 128).count();
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
            for px in buf.chunks_exact(4) {
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
        let opaque = buf.chunks_exact(4).filter(|px| px[3] > 128).count();
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
}
