//! V75: a bird trigger's box test against the original's segment-against-box leaf, run on the boot
//! ELF: random frames and boxes, segments crossing, grazing and missing the box, points (segments of
//! no length), and points on the faces exactly, where touching counts. Without the disc the test
//! says so and passes.

use game_render::creature::segment_touches_box;
use ps2_float::Vf;
use ps2_oracle::{Ee, addr, skip};

/// Where the made-up arguments sit, 16-byte aligned.
const CENTRE: u32 = 0x0180_0000;
const HALF: u32 = 0x0180_0010;
const A: u32 = 0x0180_0020;
const B: u32 = 0x0180_0030;
const ROWS: u32 = 0x0180_0040;

struct Xs(u32);

impl Xs {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        self.0
    }
    /// In -1..1.
    fn signed(&mut self) -> f32 {
        (self.next() >> 8) as f32 / (1u32 << 23) as f32 - 1.0
    }
    fn unit(&mut self) -> f32 {
        (self.next() >> 8) as f32 / (1u32 << 24) as f32
    }
}

fn put(ee: &mut Ee, at: u32, v: Vf) {
    for k in 0..4 {
        ee.set_f32(at + 4 * k as u32, [v.x(), v.y(), v.z(), v.w()][k]);
    }
}

/// A rotation off a random unit quaternion, its rows right, up, look, then `origin`.
fn rows(xs: &mut Xs, origin: Vf) -> [Vf; 4] {
    let q = [xs.signed(), xs.signed(), xs.signed(), xs.signed()];
    let n = q.iter().map(|c| c * c).sum::<f32>().sqrt().max(1e-6);
    let [w, x, y, z] = q.map(|c| c / n);
    [
        Vf::new(1.0 - 2.0 * (y * y + z * z), 2.0 * (x * y + w * z), 2.0 * (x * z - w * y), 0.0),
        Vf::new(2.0 * (x * y - w * z), 1.0 - 2.0 * (x * x + z * z), 2.0 * (y * z + w * x), 0.0),
        Vf::new(2.0 * (x * z + w * y), 2.0 * (y * z - w * x), 1.0 - 2.0 * (x * x + y * y), 0.0),
        origin,
    ]
}

const IDENTITY: [Vf; 4] = [
    Vf::new(1.0, 0.0, 0.0, 0.0),
    Vf::new(0.0, 1.0, 0.0, 0.0),
    Vf::new(0.0, 0.0, 1.0, 0.0),
    Vf::new(0.0, 0.0, 0.0, 1.0),
];

#[test]
fn the_box_test_is_the_originals() {
    let mut ee = match Ee::game() {
        Ok(ee) => ee,
        Err(why) => return skip(&why),
    };
    let Ok(leaf) = addr("segment_box_touch") else {
        return skip("segment_box_touch is not in research/oracle.txt");
    };
    let mut xs = Xs(0x0b12_d575);
    let (mut hits, mut kinds) = (0usize, [0usize; 4]);
    let cases = 40_000;
    for case in 0..cases {
        let kind = (xs.next() % 4) as usize;
        kinds[kind] += 1;
        let half = Vf::new(xs.unit() * 40.0, xs.unit() * 40.0, xs.unit() * 40.0, 1.0);
        let centre = Vf::new(xs.signed() * 50.0, xs.signed() * 50.0, xs.signed() * 50.0, 1.0);
        // A point about the box, in its own units: up to 1.6 halves out on each axis.
        let near = |xs: &mut Xs| {
            Vf::new(
                centre.x() + xs.signed() * 1.6 * half.x(),
                centre.y() + xs.signed() * 1.6 * half.y(),
                centre.z() + xs.signed() * 1.6 * half.z(),
                1.0,
            )
        };
        let (m, a, b) = match kind {
            // A segment through rotated rows, both ends about the box.
            0 => {
                let origin = Vf::new(xs.signed() * 10.0, xs.signed() * 10.0, xs.signed() * 10.0, 1.0);
                let m = rows(&mut xs, origin);
                let (a, b) = (near(&mut xs), near(&mut xs));
                // The rows move the ends; the box stays. Put the ends where the rows bring them back
                // near it: through the transpose, about the box less the origin.
                let back = |p: Vf| {
                    let d = [p.x() - m[3].x(), p.y() - m[3].y(), p.z() - m[3].z()];
                    let r = |i: usize| m[i].x() * d[0] + m[i].y() * d[1] + m[i].z() * d[2];
                    Vf::new(r(0), r(1), r(2), 1.0)
                };
                (m, back(a), back(b))
            }
            // Long segments with both ends well out, often across the box.
            1 => {
                let (p, q) = (near(&mut xs), near(&mut xs));
                let s = 2.0 + xs.unit() * 6.0;
                let lerp = |t: f32| Vf::new(p.x() + (q.x() - p.x()) * t, p.y() + (q.y() - p.y()) * t, p.z() + (q.z() - p.z()) * t, 1.0);
                (IDENTITY, lerp(-s), lerp(1.0 + s))
            }
            // A point.
            2 => {
                let p = near(&mut xs);
                (IDENTITY, p, p)
            }
            // A point on a face, an edge or a corner exactly, or a ulp either side.
            _ => {
                let mut p = { let v = near(&mut xs); [v.x(), v.y(), v.z()] };
                let (c, h) = ([centre.x(), centre.y(), centre.z()], [half.x(), half.y(), half.z()]);
                for i in 0..3 {
                    if xs.next() % 2 == 0 {
                        let side = if xs.next() % 2 == 0 { 1.0 } else { -1.0 };
                        let on = c[i] + side * h[i];
                        p[i] = match xs.next() % 3 {
                            0 => on,
                            1 => f32::from_bits(on.to_bits() + 1),
                            _ => f32::from_bits(on.to_bits().saturating_sub(1)),
                        };
                    }
                }
                let p = Vf::new(p[0], p[1], p[2], 1.0);
                (IDENTITY, p, p)
            }
        };
        put(&mut ee, CENTRE, centre);
        put(&mut ee, HALF, half);
        put(&mut ee, A, a);
        put(&mut ee, B, b);
        for (k, r) in m.iter().enumerate() {
            put(&mut ee, ROWS + 16 * k as u32, *r);
        }
        ee.call(leaf, &[CENTRE, HALF, A, B, ROWS].map(u64::from), &[], 10_000);
        let theirs = ee.v0() as u32 != 0;
        let ours = segment_touches_box(centre, half, a, b, &m);
        assert_eq!(ours, theirs, "case {case} (kind {kind}): centre {centre:?} half {half:?} a {a:?} b {b:?} rows {m:?}");
        hits += usize::from(theirs);
    }
    println!("{hits} of {cases} touching; kinds {kinds:?}");
    assert!(hits > cases / 5 && hits < cases * 4 / 5, "both answers well tried: {hits}");
}
