//! Individually inked silhouettes. All coordinates are authored in a shared
//! 256 px art space, with feet at y=206. No external assets or runtime I/O.
use crate::creature::{Mouth, Pose, FEET_BELOW_CENTER, SPRITE_PX};
use mote_core::SpeciesId;
use std::sync::OnceLock;
use tiny_skia::*;

const INK: u32 = 0x252320;

struct Design {
    body: &'static str,
    detail: &'static str,
    colour: u32,
    spot: u32,
    eyes: [(f32, f32); 2],
    feet: [f32; 2],
}

// Closed cubic contours give each creature its own anatomy, including the
// continuous transitions from body to horn, ear, curl or tail.
const DESIGNS: [Design; 12] = [
    Design { body: "M 79 190 C 61 183 63 158 72 137 C 79 117 92 102 103 96 L 98 83 C 84 83 85 79 91 73 L 106 58 C 113 53 103  70 106 76 L 114 89 C 130 85 143 86 153 92 C 160 81 157 70 164 60 L 174 49 C 178 44 169 34 171 28 C 177 25 190 47 188 52 C 185 59 176 63 174 74 L 169 101 C 184 120 190 135 187 151 C 196 162 198 176 191 179 C 185 182 183 170 181 166 C 185 192 169 199 143 200 C 118 203 94 199 79 190 Z", detail: "M 100 93 L 108 88 M 157 94 L 168 100 M 83 165 C 91 170 91 181 84 184 M 176 159 C 179 166 178 172 175 176", colour:0x57514f, spot:0x393638, eyes:[(105.,145.),(155.,134.)], feet:[91.,163.] },
    Design { body:"M 95 196 C 76 186 77 161 85 139 C 92 119 100 102 119 96 C 129 91 138 94 139 82 C 141 72 127 68 126 61 C 126 55 135 43 140 35 C 148 24 162 29 162 37 C 162 46 149 47 141 43 C 132 52 135 56 144 62 C 156 69 144 82 145 93 C 157 102 164 121 162 139 C 178 157 183 179 172 191 C 158 204 117 202 95 196 Z", detail:"M 141 43 C 146 48 155 45 159 42 M 154 157 C 154 173 155 189 147 188 C 138 187 141 180 141 177", colour:0xeee0bd,spot:0xb3a58d,eyes:[(111.,126.),(149.,139.)],feet:[99.,157.] },
    Design { body:"M 40 188 C 32 193 37 176 42 169 C 51 157 68 151 81 140 C 106 120 135 120 157 130 L 169 135 C 173 131 162 125 165 115 C 170 99 192 103 197 113 C 206 131 187 139 180 138 L 181 143 C 198 153 219 164 221 177 C 224 192 192 195 167 198 C 122 203 84 198 57 196 L 52 190 C 47 185 45 187 40 188 Z",detail:"M 177 136 C 173 129 181 123 189 126 M 63 190 C 78 198 133 201 166 195",colour:0xc7794f,spot:0x94583d,eyes:[(147.,164.),(193.,168.)],feet:[62.,183.] },
    Design { body:"M 98 194 C 85 178 94 164 91 149 C 85 133 89 116 101 107 C 98 92 92 80 93 64 C 89 79 88 91 79 103 C 72 112 60 101 64 90 L 86  40 C  90 31 116 34 128 44 C 143 55 148 68 150 81 C 174 82 185 97 181 120 C 191 140 191 173 180 189 C 166 206 118 202 98 194 Z",detail:"M 93 64 C 96 54 97 52 100 62 C 104 82 103 93 108 103 M 111 171 C 112 178 119 172 122 179 C 124 188 109 190 107 183 M 165 144 C 158 143 158 132 166 131 C 172 128 181 132 184 138",colour:0x929754,spot:0x6c733d,eyes:[(119.,141.),(156.,122.)],feet:[108.,164.] },
    Design { body:"M 107 198 C 91 187 96 173 91 157 C 86 141 90 124 99 112 C 107 97 100  90 93 80 C 82 61 94 43 112 40 C 131 36 143 42 137 50 C 132 56 128 48 127 48 C 112 46 106 60 111 73 C 115 88 134 94 145 108 C 161 125 170 145 165 167 C 162 187 151 200 137 200 C 123 201 117 202 107 198 Z",detail:"M 115 158 C 111 150 106 146 102 149 C 100 153 105 155 105 160 C 105 167 115 178 121 177 M 98 165 C  90 154 86 143 79 139",colour:0xb9bbc4,spot:0x91939e,eyes:[(113.,126.),(139.,115.)],feet:[109.,145.] },
    Design { body:"M 81 117 C 104  90 143 92 166 109 C 181 122 203 142 225 157 C 241 170 220 185 207 180 C 194 174 185 165 178 155 C 182 181 175 197 153 199 C 126 203 100 201 87 192 C 77 184 77 171 78 157 C  60 181 48 185 37 179 C 22 172 37 158 46 149 Z",detail:"M 78 157 C 81 145 86 130 91 122 M 178 155 C 176 145 171 135 168 129",colour:0x6d8091,spot:0x485969,eyes:[(111.,140.),(147.,153.)],feet:[99.,160.] },
    Design { body:"M 100 197 C 80 190 87 171 93 162 C 83 151 82 136 91 122 C 101 106 115 103 121 101 C 127 86 131 73 145  60 C 142 44 155 35 168 41 C 186 48 184 66 173 73 C 163 78 151 72 149 68 C 139 75 136 88 132 100 C 151 105 161 120 165 137 C 176 154 173 182 161 193 C 149 204 116 202 100 197 Z",detail:"M 152  60 C 153 69 164 72 170 70 M 119 158 C 132 156 134 168 125 172 L 115 172 M 149 158 C 139 164 139 177 154 173",colour:0x9986a2,spot:0x776680,eyes:[(110.,139.),(153.,134.)],feet:[103.,153.] },
    Design { body:"M 84 189 L 83 155 C 72 151 67 143 60 137 C 56 134 55 128 59 129 L 64 131 C 60 121 64 119 68 126 C 66 119 72 120 73 129 L 85 138 L 91 120 C 75 113  70 102 79 87 L 90 68 C 96 59 97 74 105 80 L 117 89 L 114 105 C 135 103 149 105 160 110 C 166 92 177 73 188 76 C 201 78 214 88 215 95 C 217 113 197 119 179 122 L 181 140 L 196 128 C 197 121 202 121 202 127 C 210 121 213 126 207 131 C 215 130 213 135 207 138 C 199 149 189 156 181 158 L 181 189 C 177 201 160 199 145 199 L 111 199 C 94 201 87 200 84 189 Z",detail:"M 89 80 C  80  90 81 106 94 109 L 103 95 Z M 168 112 C 175 100 180 94 185 93",colour:0xede0bf,spot:0x9d8c76,eyes:[(109.,137.),(153.,145.)],feet:[99.,168.] },
    Design { body:"M  90 190 C 86 178 91 167 96 159 C 78 141 86 118 106 110 L 114 106 L 113 94 C 93 89  90 68 103 56 C 117 43 141 48 148 64 C 155  80 143 94 128 96 L 128 106 C 149 111 165 122 165 140 C 165 151 157 159 155 165 C 159 178 164 189 154 197 C 137 203 103 201 90 190 Z M 117 84 C 129 88 139 77 135 67 C 129 55 113 59 109 69 C 106 77 111 83 117 84 Z",detail:"M 111 167 C 119 173 122 185 114 185 C 108 186 104 182 103 179 M 145 168 C 141 175 142 184 136 185 C 130 186 130 180 132 176",colour:0xd8ab55,spot:0xb58a40,eyes:[(108.,137.),(148.,145.)],feet:[99.,156.] },
    Design { body:"M 91 189 C 77 180 78 164 78 151 C 64 157 66 142 72 133 C 76 126  80 122 88 120 C 88 99 107 95 124 95 C 107 85 118 69 130 70 C 148 68 157 85 143 94 C 170 95 190 110 184 131 C 183 142 174 149 164 153 C 174 159 179 171 171 175 C 165 179 162 168 159 166 C 163 179 158 193 148 197 C 129 202 103 199 91 189 Z",detail:"M 104 159 C 113 169 129 177 126 183 C 121 193 106 184 103 179 M 88 121 C 95 117 98 110 98 106",colour:0x94a263,spot:0x6c7d45,eyes:[(127.,135.),(168.,120.)],feet:[102.,149.] },
    Design { body:"M 98 195 C 79 184 84 160 91 145 C 101 125 121 106 128 84 C 137 62 131 47 119 49 C 108 48 109  60 103 57 C 95 52 105 38 115 36 C 137 29 147 49 151 67 C 159 96 163 119 167 146 C 172 168 170 187 155 196 C 141 204 112 201 98 195 Z",detail:"M 119 41 C 135 36 144 55 148 68 M 154 125 L 159 127",colour:0x454650,spot:0x383a44,eyes:[(115.,143.),(153.,143.)],feet:[104.,151.] },
    Design { body:"M 74 183 C  80 162 86 144 99 126 C 111 109 128 101 145 102 C 177 99 190 114 187 137 C 186 150 176 158 175 168 C 174 177 181 183 175 187 C 170 191 167 184 167 184 C 169 200 143 199 123 199 C 104 201 91 194  80 196 C 65 201 48 199  40 194 C 35 187 59 185 74 183 Z",detail:"M 119 161 C 115 173 128 185 137 181 C 147 177 137 169 133 165 M 70 185 C 79 188 84 190 90 191",colour:0xa29486,spot:0x7c6c60,eyes:[(135.,136.),(172.,126.)],feet:[103.,157.] },
];

const RING_NAP: Design = Design {
    body: "M 90 174 C 80 169.2 77 154.8 81 142 C 84 127.6 96 124.4 103 137.2 C 111 150 106 166 98 170.8 C 108 143.6 122 132.4 140 138.8 C 159 142 173 159.6 175 177.2 C 191 177.2 190 193.2 179 193.2 L 174 199.6 C 158 207.6 139 196.4 122 199.6 C 107 207.6 91 204.4 89 193.2 C  80 188.4 83 178.8 90 174 Z M 89 161.2 C 95 167.6 102 153.2 97 146.8 C 91 138.8 85 153.2 89 161.2 Z",
    detail: "M 110 180.4 C 106 194.8 116 201.2 122 193.2 M 159 172.4 C 151 172.4 150 188.4 156 193.2 C 164 198 169 188.4 166 182 M 131 185.2 Q 134 190 137 183.6",
    colour: 0xd8ab55,
    spot: 0xb58a40,
    eyes: [(121., 170.8), (141., 170.8)],
    feet: [104., 163.],
};

fn sway(path: &Path, phase: f32) -> Path {
    let warp = |p: Point| {
        let weight = ((110. - p.y) / 85.).clamp(0., 1.);
        Point::from_xy(p.x + phase.sin() * 3. * weight * weight, p.y)
    };
    let mut b = PathBuilder::new();
    for segment in path.segments() {
        match segment {
            PathSegment::MoveTo(p) => {
                let p = warp(p);
                b.move_to(p.x, p.y);
            }
            PathSegment::LineTo(p) => {
                let p = warp(p);
                b.line_to(p.x, p.y);
            }
            PathSegment::QuadTo(a, p) => {
                let a = warp(a);
                let p = warp(p);
                b.quad_to(a.x, a.y, p.x, p.y);
            }
            PathSegment::CubicTo(a, c, p) => {
                let a = warp(a);
                let c = warp(c);
                let p = warp(p);
                b.cubic_to(a.x, a.y, c.x, c.y, p.x, p.y);
            }
            PathSegment::Close => b.close(),
        }
    }
    b.finish().unwrap()
}

// Use only M/L/C/Q/Z in the authored paths. Parse once, never every frame.
fn path(data: &str) -> Path {
    let mut b = PathBuilder::new();
    for seg in svgtypes::PathParser::from(data) {
        match seg.expect("valid authored path") {
            svgtypes::PathSegment::MoveTo { abs: true, x, y } => b.move_to(x as f32, y as f32),
            svgtypes::PathSegment::LineTo { abs: true, x, y } => b.line_to(x as f32, y as f32),
            svgtypes::PathSegment::CurveTo {
                abs: true,
                x1,
                y1,
                x2,
                y2,
                x,
                y,
            } => b.cubic_to(
                x1 as f32, y1 as f32, x2 as f32, y2 as f32, x as f32, y as f32,
            ),
            svgtypes::PathSegment::Quadratic {
                abs: true,
                x1,
                y1,
                x,
                y,
            } => b.quad_to(x1 as f32, y1 as f32, x as f32, y as f32),
            svgtypes::PathSegment::ClosePath { .. } => b.close(),
            _ => panic!("unsupported authored path command"),
        }
    }
    b.finish().expect("nonempty authored path")
}

fn colour(rgb: u32, alpha: u8) -> Color {
    Color::from_rgba8((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8, alpha)
}
fn paint(rgb: u32, alpha: u8) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color(colour(rgb, alpha));
    p
}
fn stroke(pm: &mut Pixmap, p: &Path, t: Transform, rgb: u32, width: f32) {
    pm.stroke_path(
        p,
        &paint(rgb, 255),
        &Stroke {
            width,
            line_cap: LineCap::Round,
            line_join: LineJoin::Round,
            ..Stroke::default()
        },
        t,
        None,
    );
}
fn oval(x: f32, y: f32, rx: f32, ry: f32) -> Path {
    PathBuilder::from_oval(Rect::from_xywh(x - rx, y - ry, rx * 2., ry * 2.).unwrap()).unwrap()
}
fn fill(pm: &mut Pixmap, p: &Path, t: Transform, rgb: u32) {
    pm.fill_path(p, &paint(rgb, 255), FillRule::EvenOdd, t, None);
}
fn inked(pm: &mut Pixmap, p: &Path, t: Transform, rgb: u32) {
    fill(pm, p, t, rgb);
    stroke(pm, p, t, INK, 1.85);
}

pub fn render(p: &Pose) -> Vec<u8> {
    static PATHS: OnceLock<Vec<(Path, Path)>> = OnceLock::new();
    let paths = PATHS.get_or_init(|| {
        DESIGNS
            .iter()
            .chain(std::iter::once(&RING_NAP))
            .map(|d| (path(d.body), path(d.detail)))
            .collect()
    });
    let napping = p.species == SpeciesId::RingTail && p.is_napping && p.sleep_amount > 0.5;
    let i = if napping {
        12
    } else {
        p.species.index() as usize - 1
    };
    let d = if napping { &RING_NAP } else { &DESIGNS[i] };
    let (body, detail) = &paths[i];
    let body = sway(body, p.ear_phase);
    let detail = sway(detail, p.ear_phase);
    let (body, detail) = (&body, &detail);
    let mut pm = Pixmap::new(SPRITE_PX as u32, SPRITE_PX as u32).unwrap();
    let s = p.radius / 64.;
    // Deform around the contact point rather than the belly, so squash and
    // sleep do not sink the feet through a ledge. Tilt rotates the whole art.
    let sx = s * p.squash_x;
    let sy = s * p.squash_y;
    let angle = p.tilt;
    let (sn, cs) = angle.sin_cos();
    let anchor_x = 128. + p.lean * 4. * s;
    let anchor_y = 128. + FEET_BELOW_CENTER * s + p.hop_px * s;
    let transform = |fit: f32| {
        Transform::from_row(
            sx * cs * fit,
            sx * sn * fit,
            -sy * sn * fit,
            sy * cs * fit,
            anchor_x + (-128. * sx * cs + 206. * sy * sn) * fit,
            anchor_y + (-128. * sx * sn - 206. * sy * cs) * fit,
        )
    };
    let bounds = body.clone().transform(transform(1.)).unwrap().bounds();
    // Reserve antialiasing margins on the fixed native overlay. Keep the
    // contact point fixed; only unusually wide/tall poses need fitting.
    let mut fit = 1.0_f32;
    if bounds.left() < 5. {
        fit = fit.min((anchor_x - 5.) / (anchor_x - bounds.left()));
    }
    if bounds.right() > 251. {
        fit = fit.min((251. - anchor_x) / (bounds.right() - anchor_x));
    }
    if bounds.top() < 5. {
        fit = fit.min((anchor_y - 5.) / (anchor_y - bounds.top()));
    }
    let t = transform(fit);
    // Rust-coloured dorsal plates, behind the Kaiju's continuous tail.
    if p.species == SpeciesId::Kaiju {
        for (x, y) in [
            (80., 164.),
            (88., 147.),
            (99., 131.),
            (114., 116.),
            (137., 106.),
        ] {
            let mut b = PathBuilder::new();
            b.move_to(x - 3., y + 10.);
            b.cubic_to(x - 27., y - 3., x - 19., y - 10., x + 9., y - 1.);
            b.close();
            inked(&mut pm, &b.finish().unwrap(), t, 0x9c6859);
        }
    }
    // Feet articulate independently; their toe tips share the world baseline.
    for (j, x) in d.feet.iter().enumerate() {
        let phase = p.step_phase.unwrap_or(0.) + j as f32 * std::f32::consts::PI;
        let lift = phase.sin().max(0.) * 5. * p.step_amp;
        let mut b = PathBuilder::new();
        b.move_to(x - 7., 192. - lift);
        b.cubic_to(
            x - 9.,
            199. - lift,
            x - 15.,
            202. - lift,
            x - 10.,
            205. - lift,
        );
        b.cubic_to(
            x - 6.,
            207. - lift,
            x + 9.,
            205. - lift,
            x + 10.,
            203. - lift,
        );
        b.quad_to(x + 7., 198. - lift, x + 6., 192. - lift);
        b.close();
        inked(&mut pm, &b.finish().unwrap(), t, d.colour);
    }
    let mut body_paint = paint(d.colour, 255);
    body_paint.shader = LinearGradient::new(
        Point::from_xy(80., 60.),
        Point::from_xy(150., 220.),
        vec![
            GradientStop::new(0., colour(d.colour, 255)),
            GradientStop::new(0.65, colour(d.colour, 255)),
            GradientStop::new(1., colour(d.spot, 255)),
        ],
        SpreadMode::Pad,
        Transform::identity(),
    )
    .unwrap();
    pm.fill_path(body, &body_paint, FillRule::EvenOdd, t, None);
    let mut mask = Mask::new(SPRITE_PX as u32, SPRITE_PX as u32).unwrap();
    mask.fill_path(body, FillRule::EvenOdd, true, t);
    // Uneven pigment islands follow the flank, leaving the face quiet.
    for (j, (dx, y, rx, ry)) in [
        (0., 178., 3., 6.),
        (9., 189., 2.8, 3.8),
        (13., 170., 3.2, 5.),
        (-4., 161., 2., 3.),
        (15., 185., 4., 6.),
        (6., 153., 2.5, 3.5),
    ]
    .iter()
    .copied()
    .enumerate()
    {
        let flank = if i.is_multiple_of(2) { 165. } else { 90. };
        let x = flank + if i.is_multiple_of(2) { dx } else { -dx };
        let mark = oval(x, y, rx, ry);
        let mt = Transform::from_rotate_at(-24. + j as f32 * 13., x, y).post_concat(t);
        pm.fill_path(
            &mark,
            &paint(d.spot, 145),
            FillRule::Winding,
            mt,
            Some(&mask),
        );
    }
    // Broad translucent washes, not a glossy central highlight.
    let wash = oval(95., 114., 25., 59.);
    pm.fill_path(
        &wash,
        &paint(0xffedcc, 16),
        FillRule::Winding,
        t,
        Some(&mask),
    );
    // Fine deterministic pigment, attached to art coordinates so it doesn't
    // shimmer or crawl during movement. The same cached tile serves all pets.
    static GRAIN: OnceLock<Pixmap> = OnceLock::new();
    let grain = GRAIN.get_or_init(|| {
        let mut tile = Pixmap::new(256, 256).unwrap();
        let mut seed = 0x73ad1234_u32;
        for px in tile.pixels_mut() {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            let a = (seed >> 27) as u8;
            let rgb = if seed & 0x1000000 == 0 {
                0xfff2d9
            } else {
                0x40382e
            };
            *px = colour(rgb, a / 3).premultiply().to_color_u8();
        }
        tile
    });
    pm.draw_pixmap(
        0,
        0,
        grain.as_ref(),
        &PixmapPaint::default(),
        t,
        Some(&mask),
    );
    if p.species == SpeciesId::Peeker {
        for data in [
            "M 91 73 L 106 58 L 100 79 L 89 79 Z",
            "M 172 29 C 181 35 189 47 186 51 L 176 54 L 177 44 Z",
        ] {
            pm.fill_path(
                &path(data),
                &paint(0xa46c5e, 255),
                FillRule::Winding,
                t,
                Some(&mask),
            );
        }
    }
    if p.species == SpeciesId::Pup {
        pm.fill_path(
            &oval(194., 92., 25., 25.),
            &paint(0x8d7c66, 255),
            FillRule::Winding,
            t,
            Some(&mask),
        );
        pm.fill_path(
            &oval(91., 95., 10., 18.),
            &paint(0xada08a, 255),
            FillRule::Winding,
            t,
            Some(&mask),
        );
    }
    stroke(&mut pm, body, t, INK, 1.9);
    stroke(&mut pm, detail, t, INK, 1.55);
    face(&mut pm, p, d, t);
    if p.is_climbing {
        let dir = if p.facing < 0 { -1. } else { 1. };
        for j in 0..2 {
            let y = 132. + j as f32 * 43. + p.climb_phase.sin() * 9. * (1. - j as f32 * 2.);
            let mut b = PathBuilder::new();
            b.move_to(128. + dir * 19., y + 17.);
            b.quad_to(128. + dir * 42., y + 20., 128. + dir * 55., y);
            let arm = b.finish().unwrap();
            stroke(&mut pm, &arm, t, INK, 9.);
            stroke(&mut pm, &arm, t, d.colour, 5.8);
            inked(&mut pm, &oval(128. + dir * 55., y, 4., 6.), t, d.colour);
        }
    }
    if p.is_peeking {
        let ledge = (128. + FEET_BELOW_CENTER * s).round().clamp(0., 256.) as usize;
        pm.data_mut()[ledge * SPRITE_PX * 4..].fill(0);
        // Fingers sit on top of the ledge after masking the hidden body.
        for x in [103., 153.] {
            inked(
                &mut pm,
                &oval(128. + (x - 128.) * s, ledge as f32 - 3. * s, 7. * s, 5. * s),
                Transform::identity(),
                d.colour,
            );
        }
        pm.data_mut()[ledge * SPRITE_PX * 4..].fill(0);
    }
    if p.sleep_amount > 0.1 {
        for j in 0..3 {
            let ph = (p.time_s * 0.3 + j as f32 / 3.).fract();
            let x = 177. + ph * 18.;
            let y = 133. - ph * 38.;
            let mut b = PathBuilder::new();
            b.move_to(x, y);
            b.line_to(x + 5., y);
            b.line_to(x, y + 6.);
            b.line_to(x + 5., y + 6.);
            let mut pen = paint(0x89919c, ((1. - ph) * p.sleep_amount * 220.) as u8);
            pen.anti_alias = true;
            pm.stroke_path(
                &b.finish().unwrap(),
                &pen,
                &Stroke {
                    width: 1.5,
                    line_cap: LineCap::Round,
                    ..Stroke::default()
                },
                t,
                None,
            );
        }
    }
    pm.take()
}

fn face(pm: &mut Pixmap, p: &Pose, d: &Design, t: Transform) {
    let sleepy = p.eyelid.max(p.sleep_amount).clamp(0., 1.);
    for (j, (x, y)) in d.eyes.iter().copied().enumerate() {
        if sleepy > 0.85 || (p.species == SpeciesId::Kaiju && p.eye_wide < 0.4) {
            let mut b = PathBuilder::new();
            b.move_to(x - 6., y);
            b.cubic_to(x - 3., y + 5., x + 4., y + 5., x + 7., y - 1.);
            stroke(pm, &b.finish().unwrap(), t, INK, 2.);
            continue;
        }
        let relaxed = matches!(
            p.species,
            SpeciesId::Peeker | SpeciesId::Loaf | SpeciesId::Shadow
        );
        let half = if relaxed {
            0.60 * (1. - p.eye_wide)
        } else {
            0.
        };
        let rx = if p.species == SpeciesId::Seedling {
            4.
        } else {
            9.
        };
        let ry = (7. + p.eye_wide * 3.) * (1. - sleepy * 0.85);
        let eye = if half > 0.05 {
            let mut b = PathBuilder::new();
            let top = y - ry + half * ry * 1.65;
            b.move_to(x - rx, top + if j == 0 { 1.5 } else { 0. });
            b.line_to(x + rx, top);
            b.cubic_to(
                x + rx,
                y + ry + 1.,
                x - rx + 1.,
                y + ry + 2.,
                x - rx,
                top + if j == 0 { 1.5 } else { 0. },
            );
            b.close();
            b.finish().unwrap()
        } else {
            oval(x, y, rx, ry)
        };
        if p.species == SpeciesId::Seedling {
            fill(pm, &eye, t, INK);
        } else {
            inked(pm, &eye, t, 0xf5edcf);
            let mut clip = Mask::new(SPRITE_PX as u32, SPRITE_PX as u32).unwrap();
            clip.fill_path(&eye, FillRule::Winding, true, t);
            let gaze = if relaxed { 2.5 } else { -1. };
            pm.fill_path(
                &oval(x + gaze + p.look_x * 4., y - 2. + p.look_y * 2., 4.5, 7.),
                &paint(0x242321, 255),
                FillRule::Winding,
                t,
                Some(&clip),
            );
        }
        let mut brow = PathBuilder::new();
        brow.move_to(x - 3., y - 17.);
        brow.quad_to(x, y - 19. - p.eye_wide * 3., x + 3., y - 17.);
        stroke(pm, &brow.finish().unwrap(), t, INK, 1.65);
        if p.blush > 0.02 {
            pm.fill_path(
                &oval(x, y + 12., 7., 3.),
                &paint(0xcf8c7c, (p.blush * 80.) as u8),
                FillRule::Winding,
                t,
                None,
            );
        }
    }
    let x = (d.eyes[0].0 + d.eyes[1].0) * 0.5 + 2.;
    let y = (d.eyes[0].1 + d.eyes[1].1) * 0.5 + 15.;
    let mut b = PathBuilder::new();
    match p.mouth {
        Mouth::Hidden => return,
        Mouth::Oh => {
            inked(pm, &oval(x, y, 3., 4.5), t, 0x51413d);
            return;
        }
        Mouth::Wavy => {
            b.move_to(x - 5., y);
            b.cubic_to(x - 2., y - 5., x + 1., y + 5., x + 5., y);
        }
        Mouth::Small | Mouth::Smile => {
            let w = if p.mouth == Mouth::Smile { 5. } else { 3.5 };
            b.move_to(x - w, y);
            b.cubic_to(x - 1., y + 3., x + 3., y + 3., x + w, y - 1.);
        }
    }
    stroke(pm, &b.finish().unwrap(), t, INK, 1.65);
}
