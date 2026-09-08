//! Export the actual runtime renderer for art review, without Windows sensors.
use mote_core::SpeciesId;
use mote_render::creature::{draw_mote, Mouth, Pose};
use tiny_skia::{Color, Pixmap, PixmapPaint, Transform};

fn main() {
    let output = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "target/mote-gallery".into());
    std::fs::create_dir_all(&output).unwrap();
    let mut gallery = Pixmap::new(1024, 768).unwrap();
    gallery.fill(Color::from_rgba8(248, 246, 241, 255));
    let mut states = Pixmap::new(1024, 768).unwrap();
    states.fill(Color::from_rgba8(36, 39, 45, 255));
    for (i, &species) in SpeciesId::all().iter().enumerate() {
        let p = Pose {
            species,
            ..Pose::default()
        };
        let frame = Pixmap::from_vec(
            draw_mote(&p),
            tiny_skia::IntSize::from_wh(256, 256).unwrap(),
        )
        .unwrap();
        frame
            .save_png(format!("{output}/{}.png", species.id_str()))
            .unwrap();
        gallery.draw_pixmap(
            (i % 4 * 256) as i32,
            (i / 4 * 256) as i32,
            frame.as_ref(),
            &PixmapPaint::default(),
            Transform::identity(),
            None,
        );
        let p = match if i == 4 {
            1
        } else if i == 8 {
            2
        } else {
            i % 4
        } {
            0 => Pose {
                is_peeking: true,
                hop_px: 40.,
                ..p
            },
            1 => Pose {
                is_climbing: true,
                climb_phase: 1.,
                tilt: -0.15,
                ..p
            },
            2 => Pose {
                is_napping: true,
                sleep_amount: 1.,
                eyelid: 1.,
                squash_x: 1.15,
                squash_y: 0.74,
                mouth: Mouth::Hidden,
                time_s: 2.,
                ..p
            },
            _ => Pose {
                step_phase: Some(1.),
                step_amp: 1.,
                tilt: 0.1,
                eye_wide: 0.5,
                mouth: Mouth::Smile,
                ..p
            },
        };
        let frame = Pixmap::from_vec(
            draw_mote(&p),
            tiny_skia::IntSize::from_wh(256, 256).unwrap(),
        )
        .unwrap();
        states.draw_pixmap(
            (i % 4 * 256) as i32,
            (i / 4 * 256) as i32,
            frame.as_ref(),
            &PixmapPaint::default(),
            Transform::identity(),
            None,
        );
    }
    gallery.save_png(format!("{output}/gallery.png")).unwrap();
    states.save_png(format!("{output}/states.png")).unwrap();
    let mut sizes = Pixmap::new(768, 3072).unwrap();
    sizes.fill(Color::from_rgba8(248, 246, 241, 255));
    for (row, &species) in SpeciesId::all().iter().enumerate() {
        for (col, radius) in [46., 64., 86.].iter().copied().enumerate() {
            let p = Pose {
                species,
                radius,
                ..Default::default()
            };
            let frame = Pixmap::from_vec(
                draw_mote(&p),
                tiny_skia::IntSize::from_wh(256, 256).unwrap(),
            )
            .unwrap();
            sizes.draw_pixmap(
                (col * 256) as i32,
                (row * 256) as i32,
                frame.as_ref(),
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
            species: SpeciesId::ALL[n % 12],
            ear_phase: n as f32 * 0.05,
            step_phase: Some(n as f32 * 0.1),
            step_amp: 0.5,
            ..Default::default()
        }));
    }
    println!("Warm renderer mean over 240 mixed-species frames: {:.3} ms (use --release for performance evidence)",start.elapsed().as_secs_f64()*1000./240.);
    println!("Wrote runtime artwork to {output}");
}
