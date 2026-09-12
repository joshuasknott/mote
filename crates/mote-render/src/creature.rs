//! Runtime contract and raster renderer for Mote's pet atlases.
//!
//! Each species is supplied as one embedded generated PNG from
//! `assets/pets`. The image is an eight-pose atlas (four columns by two rows),
//! with equal cells in this order: stand, walk contact near, walk passing,
//! walk contact far, sitting alert, curled asleep, crouched anticipation,
//! airborne leap. Invalid embedded art fails loudly at startup rather than
//! silently replacing a real animal with a placeholder.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use mote_core::{BehaviourState, SpeciesId};
use tiny_skia::{FilterQuality, Pixmap, PixmapPaint, Transform};

pub const SPRITE_PX: usize = 256;
pub const FEET_BELOW_CENTER: f32 = 78.0;
const BASE_RADIUS: f32 = 64.0;
const ATLAS_COLUMNS: usize = 4;
const ATLAS_ROWS: usize = 2;

/// Full pose for one frame. Produced by [`crate::anim::Animator`]. Behaviour
/// and physics supply restrained transforms; anatomy remains in the atlas.
#[derive(Debug, Clone)]
pub struct Pose {
    pub species: SpeciesId,
    pub state: BehaviourState,
    /// Seconds spent in the current behaviour, used for pose transitions.
    pub state_age: f32,
    pub squash_x: f32,
    pub squash_y: f32,
    pub tilt: f32,
    pub hop_px: f32,
    pub lean: f32,
    pub step_phase: Option<f32>,
    pub sleep_amount: f32,
    pub radius: f32,
    pub time_s: f32,
    pub is_peeking: bool,
    pub is_climbing: bool,
    pub climb_phase: f32,
    pub is_napping: bool,
    pub facing: i8,
}

impl Default for Pose {
    fn default() -> Self {
        Self {
            species: SpeciesId::default(),
            state: BehaviourState::Idle,
            state_age: 1.0,
            squash_x: 1.0,
            squash_y: 1.0,
            tilt: 0.0,
            hop_px: 0.0,
            lean: 0.0,
            step_phase: None,
            sleep_amount: 0.0,
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

#[derive(Clone)]
struct AtlasFrame {
    pixmap: Pixmap,
    bounds: (u32, u32, u32, u32),
    foot_y: u32,
}
struct Atlas {
    frames: Vec<AtlasFrame>,
    /// Standing/walking reference height. Every frame is scaled from this
    /// common value so a raised paw does not make a character grow or shrink.
    reference_height: f32,
}
type AtlasCache = HashMap<SpeciesId, Arc<Atlas>>;
static ATLASES: OnceLock<Mutex<AtlasCache>> = OnceLock::new();

fn atlas_cache() -> &'static Mutex<AtlasCache> {
    ATLASES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn atlas_bytes(species: SpeciesId) -> &'static [u8] {
    match species {
        SpeciesId::Cat => include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets/pets/cat.png"
        )),
        SpeciesId::Dog => include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets/pets/dog.png"
        )),
        SpeciesId::Rabbit => include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets/pets/rabbit.png"
        )),
        SpeciesId::Fox => include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets/pets/fox.png"
        )),
        SpeciesId::Owl => include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets/pets/owl.png"
        )),
        SpeciesId::Tortoise => include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets/pets/tortoise.png"
        )),
    }
}

fn primary_component(
    data: &mut [u8],
    width: u32,
    height: u32,
) -> Option<((u32, u32, u32, u32), u32)> {
    let len = (width * height) as usize;
    let mut seen = vec![false; len];
    let mut best: Vec<usize> = Vec::new();
    let mut best_bounds = (0, 0, width, height);
    let mut best_foot = 0;
    for start in 0..len {
        if seen[start] || data[start * 4 + 3] <= 2 {
            continue;
        }
        let mut stack = vec![start];
        let mut component = Vec::new();
        seen[start] = true;
        let mut min_x = width;
        let mut min_y = height;
        let mut max_x = 0;
        let mut max_y = 0;
        while let Some(idx) = stack.pop() {
            component.push(idx);
            let x = idx as u32 % width;
            let y = idx as u32 / width;
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x + 1);
            max_y = max_y.max(y + 1);
            let x0 = x.saturating_sub(1);
            let x1 = (x + 1).min(width - 1);
            let y0 = y.saturating_sub(1);
            let y1 = (y + 1).min(height - 1);
            for ny in y0..=y1 {
                for nx in x0..=x1 {
                    let ni = (ny * width + nx) as usize;
                    if !seen[ni] && data[ni * 4 + 3] > 2 {
                        seen[ni] = true;
                        stack.push(ni);
                    }
                }
            }
        }
        if component.len() > best.len() {
            best = component;
            best_bounds = (min_x, min_y, max_x, max_y);
            best_foot = max_y;
        }
    }
    if best.is_empty() {
        return None;
    }
    let mut keep = vec![false; len];
    for idx in best {
        keep[idx] = true;
    }
    // Keep the anti-aliased fringe around the primary subject, while removing
    // small neighbour-cell fragments and detached generation artefacts.
    for idx in 0..len {
        if keep[idx] {
            continue;
        }
        let x = idx as u32 % width;
        let y = idx as u32 / width;
        let x0 = x.saturating_sub(1);
        let x1 = (x + 1).min(width - 1);
        let y0 = y.saturating_sub(1);
        let y1 = (y + 1).min(height - 1);
        let near = (y0..=y1).any(|ny| (x0..=x1).any(|nx| keep[(ny * width + nx) as usize]));
        if !near {
            data[idx * 4 + 3] = 0;
            data[idx * 4] = 0;
            data[idx * 4 + 1] = 0;
            data[idx * 4 + 2] = 0;
        }
    }
    Some((best_bounds, best_foot))
}

#[cfg(test)]
fn visible_bounds(data: &[u8], width: u32, height: u32) -> Option<(u32, u32, u32, u32)> {
    let mut min_x = width;
    let mut min_y = height;
    let mut max_x = 0;
    let mut max_y = 0;
    for y in 0..height {
        for x in 0..width {
            if data[((y * width + x) * 4 + 3) as usize] > 16 {
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x + 1);
                max_y = max_y.max(y + 1);
            }
        }
    }
    (max_x > min_x).then_some((min_x, min_y, max_x, max_y))
}

fn load_atlas(species: SpeciesId) -> Arc<Atlas> {
    let pixmap = Pixmap::decode_png(atlas_bytes(species)).unwrap_or_else(|err| {
        panic!(
            "embedded Mote pet atlas {} could not be decoded: {}",
            species.asset_slug(),
            err
        )
    });
    let width = pixmap.width();
    let height = pixmap.height();
    if width == 0
        || height == 0
        || !width.is_multiple_of(ATLAS_COLUMNS as u32)
        || !height.is_multiple_of(ATLAS_ROWS as u32)
    {
        panic!(
            "embedded Mote pet atlas {} must be a non-empty 4x2 equal-cell PNG (got {}x{})",
            species.asset_slug(),
            width,
            height
        );
    }
    let cell_w = width / ATLAS_COLUMNS as u32;
    let cell_h = height / ATLAS_ROWS as u32;
    let mut frames = Vec::with_capacity(8);
    for index in 0..8usize {
        let ox = (index % ATLAS_COLUMNS) as u32 * cell_w;
        let oy = (index / ATLAS_COLUMNS) as u32 * cell_h;
        let mut data = vec![0u8; (cell_w * cell_h * 4) as usize];
        for y in 0..cell_h {
            let src_start = ((oy + y) * width + ox) as usize * 4;
            let dst_start = (y * cell_w) as usize * 4;
            data[dst_start..dst_start + cell_w as usize * 4]
                .copy_from_slice(&pixmap.data()[src_start..src_start + cell_w as usize * 4]);
        }
        let (bounds, foot_y) = primary_component(&mut data, cell_w, cell_h)
            .unwrap_or(((0, 0, cell_w, cell_h), cell_h));
        let frame =
            Pixmap::from_vec(data, tiny_skia::IntSize::from_wh(cell_w, cell_h).unwrap()).unwrap();
        frames.push(AtlasFrame {
            pixmap: frame,
            bounds,
            foot_y,
        });
    }
    let reference_height = frames[..4]
        .iter()
        .map(|f| (f.bounds.3 - f.bounds.1) as f32)
        .fold(1.0, f32::max);
    Arc::new(Atlas {
        frames,
        reference_height,
    })
}

fn get_atlas(species: SpeciesId) -> Arc<Atlas> {
    let mut cache = atlas_cache().lock().unwrap_or_else(|p| p.into_inner());
    if let Some(cached) = cache.get(&species) {
        return cached.clone();
    }
    let atlas = load_atlas(species);
    cache.insert(species, atlas.clone());
    atlas
}

fn smoothstep(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn frame_for(p: &Pose) -> (usize, usize, f32) {
    if p.is_napping || p.state == BehaviourState::Sleep || p.state == BehaviourState::Waking {
        let asleep = p.sleep_amount.clamp(0.0, 1.0);
        return if asleep < 0.995 {
            (0, 5, smoothstep(asleep))
        } else {
            (5, 5, 0.0)
        };
    }
    if p.sleep_amount > 0.02 {
        return (5, 0, smoothstep(1.0 - p.sleep_amount));
    }
    if p.state == BehaviourState::Sit {
        return (4, 4, 0.0);
    }
    if p.state == BehaviourState::Landing {
        return (6, 0, smoothstep((p.state_age / 0.20).clamp(0.0, 1.0)));
    }
    if p.state == BehaviourState::Jumping {
        return (6, 7, smoothstep((p.state_age / 0.14).clamp(0.0, 1.0)));
    }
    if matches!(p.state, BehaviourState::Stretch | BehaviourState::Wobble) {
        return (6, 6, 0.0);
    }
    if p.state == BehaviourState::Falling {
        return (7, 7, 0.0);
    }
    if p.step_phase.is_some() || p.is_climbing {
        let phase = p
            .step_phase
            .unwrap_or(p.climb_phase)
            .rem_euclid(std::f32::consts::TAU);
        let f = phase / std::f32::consts::TAU * 4.0;
        let i = f.floor() as usize;
        let walk = [1usize, 2, 3, 2];
        return (
            walk[i],
            walk[(i + 1) % walk.len()],
            smoothstep(f - f.floor()),
        );
    }
    (0, 0, 0.0)
}

fn draw_frame(
    dst: &mut Pixmap,
    atlas: &Atlas,
    frame: &AtlasFrame,
    p: &Pose,
    opacity: f32,
    portrait: Option<(u32, u32)>,
) {
    let (bx, by, ex, ey) = frame.bounds;
    let bw = (ex - bx).max(1) as f32;
    let bh = (ey - by).max(1) as f32;
    let radius_scale = (p.radius / BASE_RADIUS).clamp(0.35, 2.2);
    // The atlas already contains the silhouette proportions for each pose.
    // Keep one species scale and let its crouch/sleep/leap artwork carry the
    // dimensional change, avoiding a resize snap at state boundaries.
    let pose_scale = 1.0;
    let (cx, baseline, max_w, max_h) = portrait
        .map(|(w, h)| {
            (
                w as f32 * 0.5,
                h as f32 * 0.90,
                w as f32 * 0.9,
                h as f32 * 0.86,
            )
        })
        .unwrap_or((
            SPRITE_PX as f32 * 0.5 + p.lean.clamp(-1.0, 1.0) * p.radius * 0.25,
            SPRITE_PX as f32 * 0.5 + FEET_BELOW_CENTER * radius_scale + p.hop_px,
            SPRITE_PX as f32 - 28.0,
            SPRITE_PX as f32 - 24.0,
        ));
    let target_ref_h = if portrait.is_some() {
        max_h
    } else {
        p.radius * 2.08
    };
    let source_scale = target_ref_h / atlas.reference_height * pose_scale;
    let mut target_h = bh * source_scale * p.squash_y.clamp(0.92, 1.08);
    let mut target_w = bw * source_scale * p.squash_x.clamp(0.92, 1.08);
    let facing = if p.facing < 0 { -1.0 } else { 1.0 };
    let climb_angle = if p.is_climbing {
        -(p.facing.signum() as f32) * std::f32::consts::FRAC_PI_2
    } else {
        0.0
    };
    let angle = (p.tilt + climb_angle).clamp(-std::f32::consts::PI, std::f32::consts::PI);
    let cos = angle.cos();
    let sin = angle.sin();
    // Account for the rotated silhouette before drawing. This matters for a
    // vertical climber: fitting the unrotated animal leaves its paws clipped
    // at the right edge of the layered window.
    let rotated_w = target_w * cos.abs() + target_h * sin.abs();
    let rotated_h = target_w * sin.abs() + target_h * cos.abs();
    let fit = (max_w / rotated_w.max(1.0))
        .min(max_h / rotated_h.max(1.0))
        .min(1.0);
    target_w *= fit;
    target_h *= fit;
    let sx = facing * target_w / bw;
    let sy = target_h / bh;
    let a = sx * cos;
    let b = sy * sin;
    let c = -sx * sin;
    let d = sy * cos;
    let anchor_x = (bx + ex) as f32 * 0.5;
    let anchor_y = frame.foot_y as f32;
    let mut tx = cx - a * anchor_x - c * anchor_y;
    let mut ty = baseline - b * anchor_x - d * anchor_y;
    let corners = [
        (bx as f32, by as f32),
        (ex as f32, by as f32),
        (bx as f32, ey as f32),
        (ex as f32, ey as f32),
    ];
    let mut min_x = f32::INFINITY;
    let mut min_y = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut max_y = f32::NEG_INFINITY;
    for (x, y) in corners {
        let px = a * x + c * y + tx;
        let py = b * x + d * y + ty;
        min_x = min_x.min(px);
        min_y = min_y.min(py);
        max_x = max_x.max(px);
        max_y = max_y.max(py);
    }
    let canvas_w = portrait.map(|(w, _)| w as f32).unwrap_or(SPRITE_PX as f32);
    let canvas_h = portrait.map(|(_, h)| h as f32).unwrap_or(SPRITE_PX as f32);
    let margin = if portrait.is_some() { 3.0 } else { 2.0 };
    if min_x < margin {
        tx += margin - min_x;
    } else if max_x > canvas_w - margin {
        tx -= max_x - (canvas_w - margin);
    }
    if min_y < margin {
        ty += margin - min_y;
    } else if max_y > canvas_h - margin {
        ty -= max_y - (canvas_h - margin);
    }
    let transform = Transform::from_row(a, b, c, d, tx, ty);
    let paint = PixmapPaint {
        opacity: opacity.clamp(0.0, 1.0),
        quality: FilterQuality::Bilinear,
        ..PixmapPaint::default()
    };
    dst.as_mut()
        .draw_pixmap(0, 0, frame.pixmap.as_ref(), &paint, transform, None);
}

/// Render one 256x256 premultiplied RGBA frame for the desktop overlay.
pub fn draw_mote(p: &Pose) -> Vec<u8> {
    let mut dst = Pixmap::new(SPRITE_PX as u32, SPRITE_PX as u32).expect("fixed sprite canvas");
    let atlas = get_atlas(p.species);
    let (first, second, blend) = frame_for(p);
    if second != first && blend > 0.001 {
        // Blend complete premultiplied rasters. Source-over drawing two
        // opacity-scaled sprites would make alpha dip at the midpoint.
        let mut from = Pixmap::new(SPRITE_PX as u32, SPRITE_PX as u32).unwrap();
        let mut to = Pixmap::new(SPRITE_PX as u32, SPRITE_PX as u32).unwrap();
        draw_frame(&mut from, &atlas, &atlas.frames[first], p, 1.0, None);
        draw_frame(&mut to, &atlas, &atlas.frames[second], p, 1.0, None);
        for (out, (a, b)) in dst.data_mut().as_chunks_mut::<4>().0.iter_mut().zip(
            from.data()
                .as_chunks::<4>()
                .0
                .iter()
                .zip(to.data().as_chunks::<4>().0.iter()),
        ) {
            for i in 0..4 {
                out[i] = (a[i] as f32 + (b[i] as f32 - a[i] as f32) * blend).round() as u8;
            }
        }
    } else {
        draw_frame(&mut dst, &atlas, &atlas.frames[first], p, 1.0, None);
    }
    if p.is_peeking {
        for y in 210..SPRITE_PX {
            for x in 0..SPRITE_PX {
                dst.data_mut()[(y * SPRITE_PX + x) * 4..(y * SPRITE_PX + x) * 4 + 4].fill(0);
            }
        }
    }
    dst.data().to_vec()
}

/// Render a picker portrait at the requested size as premultiplied RGBA8.
pub fn draw_portrait(species: SpeciesId, width: u32, height: u32) -> Vec<u8> {
    let width = width.clamp(1, 1024);
    let height = height.clamp(1, 1024);
    let mut dst = Pixmap::new(width, height).expect("bounded portrait canvas");
    let atlas = get_atlas(species);
    let p = Pose {
        species,
        ..Pose::default()
    };
    draw_frame(
        &mut dst,
        &atlas,
        &atlas.frames[0],
        &p,
        1.0,
        Some((width, height)),
    );
    dst.data().to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn render_outputs_have_expected_dimensions() {
        let frame = draw_mote(&Pose::default());
        assert_eq!(frame.len(), SPRITE_PX * SPRITE_PX * 4);
        let portrait = draw_portrait(SpeciesId::default(), 73, 91);
        assert_eq!(portrait.len(), 73 * 91 * 4);
    }
    #[test]
    fn premultiplied_output_invariant() {
        for px in draw_mote(&Pose::default()).as_chunks::<4>().0 {
            assert!(px[0] <= px[3] && px[1] <= px[3] && px[2] <= px[3]);
        }
    }

    #[test]
    fn every_real_species_has_visible_raster_art() {
        for &species in SpeciesId::all() {
            let frame = draw_mote(&Pose {
                species,
                ..Pose::default()
            });
            let visible = frame
                .as_chunks::<4>()
                .0
                .iter()
                .filter(|px| px[3] > 16)
                .count();
            assert!(
                visible > 1_000,
                "{species:?} atlas did not produce a visible sprite ({visible} pixels)"
            );
            let bounds =
                visible_bounds(&frame, SPRITE_PX as u32, SPRITE_PX as u32).expect("visible bounds");
            assert!(
                bounds.0 > 0
                    && bounds.1 > 0
                    && bounds.2 < SPRITE_PX as u32
                    && bounds.3 < SPRITE_PX as u32,
                "{species:?} sprite touches canvas edge: {bounds:?}"
            );
        }
    }
    #[test]
    fn walk_selection_blends_only_walk_frames() {
        let p = Pose {
            state: BehaviourState::Walk,
            step_phase: Some(std::f32::consts::PI * 0.25),
            ..Pose::default()
        };
        let (a, b, blend) = frame_for(&p);
        assert!([1, 2, 3].contains(&a) && [1, 2, 3].contains(&b));
        assert!((0.0..=1.0).contains(&blend));
    }

    #[test]
    fn walk_crossfade_keeps_opaque_subject_alpha() {
        let p = Pose {
            state: BehaviourState::Walk,
            step_phase: Some(std::f32::consts::PI * 0.75),
            ..Pose::default()
        };
        let frame = draw_mote(&p);
        let peak = frame
            .as_chunks::<4>()
            .0
            .iter()
            .map(|px| px[3])
            .max()
            .unwrap_or(0);
        // Soft fur alpha and bilinear sampling can peak just below 255.
        // A source-over
        // opacity crossfade would peak around 190 at the midpoint instead.
        assert!(peak >= 250, "crossfade alpha dipped unexpectedly to {peak}");
    }

    #[test]
    fn climbing_rotation_stays_inside_canvas() {
        for &species in SpeciesId::all() {
            for radius in [46.0, 64.0, 86.0] {
                for facing in [-1, 1] {
                    let frame = draw_mote(&Pose {
                        species,
                        radius,
                        facing,
                        is_climbing: true,
                        climb_phase: 1.0,
                        ..Pose::default()
                    });
                    let bounds = visible_bounds(&frame, SPRITE_PX as u32, SPRITE_PX as u32)
                        .expect("visible climber");
                    assert!(
                        bounds.0 > 0
                            && bounds.1 > 0
                            && bounds.2 < SPRITE_PX as u32
                            && bounds.3 < SPRITE_PX as u32,
                        "{species:?} facing {facing} radius {radius} clipped: {bounds:?}"
                    );
                }
            }
        }
    }
}
