//! Export the embedded runtime atlases for visual review, without Windows.
use mote_core::{BehaviourState, SpeciesId};
use mote_render::creature::{draw_mote, Pose};
use tiny_skia::{Color, Pixmap, PixmapPaint, Transform};

fn frame(bytes: Vec<u8>) -> tiny_skia::Pixmap {
    tiny_skia::Pixmap::from_vec(bytes, tiny_skia::IntSize::from_wh(256, 256).unwrap()).unwrap()
}

fn main() {
    let output = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "target/mote-gallery".into());
    std::fs::create_dir_all(&output).unwrap();
    let mut gallery = Pixmap::new(768, 512).unwrap();
    gallery.fill(Color::from_rgba8(248, 246, 241, 255));
    for (i, &species) in SpeciesId::all().iter().enumerate() {
        let p = Pose {
            species,
            ..Pose::default()
        };
        let sprite = frame(draw_mote(&p));
        sprite
            .save_png(format!("{output}/{}.png", species.id_str()))
            .unwrap();
        gallery.draw_pixmap(
            (i % 3 * 256) as i32,
            (i / 3 * 256) as i32,
            sprite.as_ref(),
            &PixmapPaint::default(),
            Transform::identity(),
            None,
        );
    }
    gallery.save_png(format!("{output}/gallery.png")).unwrap();

    // A compact 8-state sheet mirrors the atlas contract for every species.
    let states = [
        BehaviourState::Idle,
        BehaviourState::Walk,
        BehaviourState::Sit,
        BehaviourState::Sleep,
        BehaviourState::Stretch,
        BehaviourState::Jumping,
        BehaviourState::Landing,
        BehaviourState::Falling,
    ];
    let mut pose_sheet = Pixmap::new(2048, 1536).unwrap();
    pose_sheet.fill(Color::from_rgba8(36, 39, 45, 255));
    for (row, &species) in SpeciesId::all().iter().enumerate() {
        for (col, &state) in states.iter().enumerate() {
            let p = Pose {
                species,
                state,
                state_age: if state == BehaviourState::Jumping {
                    0.25
                } else {
                    0.0
                },
                sleep_amount: if state == BehaviourState::Sleep {
                    1.0
                } else {
                    0.0
                },
                is_napping: state == BehaviourState::Sleep,
                step_phase: (state == BehaviourState::Walk).then_some(col as f32 * 1.2),
                ..Pose::default()
            };
            let sprite = frame(draw_mote(&p));
            pose_sheet.draw_pixmap(
                (col * 256) as i32,
                (row * 256) as i32,
                sprite.as_ref(),
                &PixmapPaint::default(),
                Transform::identity(),
                None,
            );
        }
    }
    pose_sheet.save_png(format!("{output}/states.png")).unwrap();

    let mut sizes = Pixmap::new(768, 1536).unwrap();
    sizes.fill(Color::from_rgba8(248, 246, 241, 255));
    for (row, &species) in SpeciesId::all().iter().enumerate() {
        for (col, radius) in [46.0, 64.0, 86.0].into_iter().enumerate() {
            let sprite = frame(draw_mote(&Pose {
                species,
                radius,
                ..Pose::default()
            }));
            sizes.draw_pixmap(
                (col * 256) as i32,
                (row * 256) as i32,
                sprite.as_ref(),
                &PixmapPaint::default(),
                Transform::identity(),
                None,
            );
        }
    }
    sizes.save_png(format!("{output}/sizes.png")).unwrap();

    let start = std::time::Instant::now();
    for n in 0..240 {
        std::hint::black_box(draw_mote(&Pose {
            species: SpeciesId::ALL[n % SpeciesId::ALL.len()],
            state: BehaviourState::Walk,
            step_phase: Some(n as f32 * 0.1),
            ..Pose::default()
        }));
    }
    println!("Warm renderer mean over 240 mixed-species frames: {:.3} ms (use --release for performance evidence)", start.elapsed().as_secs_f64() * 1000.0 / 240.0);
    println!("Wrote runtime artwork to {output}");
}
