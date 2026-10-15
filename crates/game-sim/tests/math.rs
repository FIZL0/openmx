//! F5 — the matrix and quaternion helpers, proved against the original.
//!
//! Nine proofs, one per exported function, each over 10,000 generated inputs run through the real
//! instructions in `ps2-oracle`'s interpreter and compared bit for bit with the port. No emulator and
//! no capture: every one of these is a pure function of the quadwords it is handed, which is the
//! cheapest thing in the project to prove and the thing it is least forgivable to leave unproved —
//! every bone and every camera in the game is built out of them, so a single wrong last bit here is a
//! drift that shows up three subsystems away.
//!
//! The harness is the same shape throughout: write the inputs into a scratch region of the
//! interpreter's RAM as raw 32-bit patterns, call the original, read the output quadwords back as raw
//! patterns, and compare against the port given *the same patterns*, unaltered — see [`vf`] for why
//! unaltered and not read through `Vf::from_bits`. Comparing patterns rather than values is what makes
//! `−0` and `0` a failure, which they should be: they are not the same quadword.
//!
//! The generator leans on the awkward classes on purpose — exact zeros and minus zeros, the largest
//! magnitudes, the smallest normals, denormals, and the Inf/NaN patterns the EE does not have — because
//! the interesting disagreements between a chopping multiplier and a host `f32` are all at the ends.

use game_sim::air::AcosTable;
use game_sim::math::{self, Mat4};
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, addr, game_or_skip};

/// How many inputs each proof runs. The project's floor for an oracle proof.
const N: usize = 10_000;

/// Where the harness puts its inputs and outputs: past the executable and anything it allocates, and
/// well away from the stack a call runs on.
const SCRATCH: u32 = 0x0120_0000;
/// Where the arc cosine table goes when the interpolation proof asks the game to build one.
const TABLE: u32 = 0x0100_0000;

// --- the harness ------------------------------------------------------------------------------------

/// Four lanes into RAM as the raw patterns they are.
fn put(ee: &mut Ee, at: u32, lanes: [u32; 4]) {
    for (i, w) in lanes.iter().enumerate() {
        ee.set_u32(at + 4 * i as u32, *w);
    }
}

/// Four lanes back out, still raw: a value never becomes an `f32` on the way to a comparison.
fn get(ee: &Ee, at: u32) -> [u32; 4] {
    core::array::from_fn(|i| ee.u32_at(at + 4 * i as u32))
}

/// Sixteen lanes of a matrix into RAM.
fn put_mat(ee: &mut Ee, at: u32, m: [[u32; 4]; 4]) {
    for (r, row) in m.iter().enumerate() {
        put(ee, at + 16 * r as u32, *row);
    }
}

fn get_mat(ee: &Ee, at: u32) -> [[u32; 4]; 4] {
    core::array::from_fn(|r| get(ee, at + 16 * r as u32))
}

/// Raw patterns as the port's vector, with nothing done to them on the way.
///
/// Deliberately *not* `Vf::from_bits`: that reads a lane the way VU0 reads an operand, flushing the
/// denormals and clamping the exponents the EE has not got, and a load is not an operand. Several of
/// these functions copy a lane straight through — the position row of a matrix, the quaternion the
/// interpolation gives back when there is nothing to interpolate — and a copy keeps the pattern it was
/// given, infinity and all. The lanes that do reach an arithmetic unit are filtered there, by the same
/// `operand` on both sides, so handing over the raw pattern is the strictly more faithful thing to do.
fn vf(lanes: [u32; 4]) -> Vf {
    Vf(lanes.map(f32::from_bits))
}

fn mat(m: [[u32; 4]; 4]) -> Mat4 {
    Mat4(m.map(vf))
}

/// One mismatch, printed so the line says which lane and what both sides had.
fn same(what: &str, i: usize, theirs: [u32; 4], ours: [u32; 4]) -> bool {
    if theirs == ours {
        return true;
    }
    println!(
        "{what} #{i}: theirs [{:08x} {:08x} {:08x} {:08x}] ours [{:08x} {:08x} {:08x} {:08x}]",
        theirs[0], theirs[1], theirs[2], theirs[3], ours[0], ours[1], ours[2], ours[3]
    );
    false
}

fn same_mat(what: &str, i: usize, theirs: [[u32; 4]; 4], ours: [[u32; 4]; 4]) -> bool {
    let mut ok = true;
    for r in 0..4 {
        ok &= same(&format!("{what} row {r}"), i, theirs[r], ours[r]);
    }
    ok
}

/// A generator. Plain host arithmetic on the way in: these are only inputs, and both sides get the same
/// patterns.
struct Gen(u64);

#[allow(clippy::float_arithmetic)]
impl Gen {
    fn bits(&mut self) -> u32 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (self.0 >> 32) as u32
    }
    fn unit(&mut self) -> f32 {
        self.bits() as f32 / 4294967296.0
    }
    fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.unit()
    }
    fn one_in(&mut self, n: u32) -> bool {
        self.bits() % n == 0
    }
    fn b(&mut self) -> bool {
        self.bits() & 0x10000 != 0
    }

    /// One lane: an ordinary number most of the time, and one of the awkward classes the rest — the
    /// two zeros, the largest magnitudes, the smallest normal, a denormal, and the two patterns the EE
    /// reads as the largest magnitude because it has no infinity and no NaN.
    fn lane(&mut self) -> u32 {
        match self.bits() % 24 {
            0 => 0x0000_0000,
            1 => 0x8000_0000,
            2 => 0x7f7f_ffff,
            3 => 0xff7f_ffff,
            4 => 0x0080_0000,
            5 => 0x0000_0001,
            6 => 0x7f80_0000,
            7 => 0x7fc0_0000,
            8 => 0x3f80_0000,
            9 => 0xbf80_0000,
            _ => self.range(-100.0, 100.0).to_bits(),
        }
    }

    fn quad(&mut self) -> [u32; 4] {
        core::array::from_fn(|_| self.lane())
    }

    fn mat(&mut self) -> [[u32; 4]; 4] {
        core::array::from_fn(|_| self.quad())
    }

    /// A unit quaternion, so that the matrix proofs spend most of their inputs on rotations the game
    /// could actually hold rather than on noise.
    fn unit_quat(&mut self) -> [u32; 4] {
        let v: [f32; 4] = core::array::from_fn(|_| self.range(-1.0, 1.0));
        let n = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2] + v[3] * v[3]).sqrt();
        if n < 1e-6 {
            return [0, 0, 0, 0x3f80_0000];
        }
        core::array::from_fn(|i| (v[i] / n).to_bits())
    }

    /// A quaternion: usually a unit one, sometimes an arbitrary quadword.
    fn quat(&mut self) -> [u32; 4] {
        if self.one_in(5) { self.quad() } else { self.unit_quat() }
    }

    /// A direction: usually an ordinary vector, sometimes exactly zero so that what the normalise
    /// idiom does with no length at all is covered, and sometimes huge so the sum of squares saturates.
    fn dir(&mut self) -> [u32; 4] {
        match self.bits() % 12 {
            0 => [0, 0, 0, 0x3f80_0000],
            1 => [0x7f7f_ffff, 0x7f7f_ffff, 0x7f7f_ffff, 0],
            _ => core::array::from_fn(|_| {
                if self.one_in(10) { self.lane() } else { self.range(-30.0, 30.0).to_bits() }
            }),
        }
    }
}

// --- the matrix side ----------------------------------------------------------------------------

/// The 4×4 product.
///
/// It has no function of its own in the image — the compiler inlined it everywhere — so the carrier is
/// the smallest call-free function that contains one: it does a little scalar arithmetic out of its two
/// float arguments, multiplies the two matrices at fixed offsets in the object it is handed, and leaves
/// the product at a third. Two lanes of the right-hand matrix are where that scalar arithmetic lands,
/// and it lands there *before* the matrix is loaded, so the harness reads both matrices back out of RAM
/// after the call and gives the port what the VU actually saw rather than what was written in. The two
/// float arguments are generated along with everything else, so those two lanes are not pinned to one
/// value across the run either.
#[test]
fn the_matrix_product() {
    let Some(mut ee) = game_or_skip() else { return };
    let at = addr("mat4_mul_carrier").unwrap();
    let (obj, rhs, lhs, out) = (SCRATCH, SCRATCH + 0x40, SCRATCH + 0x80, SCRATCH + 0xc0);
    let mut g = Gen(0x5f5e_0101);
    let mut wrong = 0;
    for i in 0..N {
        put_mat(&mut ee, lhs, g.mat());
        put_mat(&mut ee, rhs, g.mat());
        let (f12, f13) = (f32::from_bits(g.lane()), f32::from_bits(g.lane()));
        ee.call(at, &[obj as u64], &[f12, f13], 10_000);
        let (a, b) = (mat(get_mat(&ee, lhs)), mat(get_mat(&ee, rhs)));
        let ours = math::mul(&a, &b);
        if !same_mat("product", i, get_mat(&ee, out), ours.0.map(|r| r.to_bits())) {
            wrong += 1;
            assert!(wrong < 6, "stopping after six");
        }
    }
    assert_eq!(wrong, 0, "{wrong} of {N} matrix products differ");
}

/// A point through a matrix, one at a time — the loop the original provides, asked for a count of one.
#[test]
fn a_point_through_a_matrix() {
    let Some(mut ee) = game_or_skip() else { return };
    let at = addr("mat4_transform_points").unwrap();
    let (out, src, m) = (SCRATCH, SCRATCH + 0x10, SCRATCH + 0x20);
    let mut g = Gen(0x1234_5678);
    let mut wrong = 0;
    for i in 0..N {
        let (v, mm) = (g.quad(), g.mat());
        put(&mut ee, src, v);
        put_mat(&mut ee, m, mm);
        ee.call(at, &[out as u64, src as u64, m as u64, 1, 0x10, 0x10], &[], 10_000);
        let ours = math::transform_point(&mat(mm), vf(v));
        if !same("point", i, get(&ee, out), ours.to_bits()) {
            wrong += 1;
            assert!(wrong < 6, "stopping after six");
        }
    }
    assert_eq!(wrong, 0, "{wrong} of {N} transformed points differ");
}

/// The same over a run of points, which is how the original is actually called: the rows are loaded
/// once and the loop walks both arrays. Proves the loop, not just the arithmetic — a count of seven
/// catches an off-by-one that a count of one cannot.
#[test]
fn a_run_of_points_through_a_matrix() {
    let Some(mut ee) = game_or_skip() else { return };
    let at = addr("mat4_transform_points").unwrap();
    let (out, src, m) = (SCRATCH, SCRATCH + 0x100, SCRATCH + 0x200);
    let mut g = Gen(0x0bad_f00d);
    let mut wrong = 0;
    for i in 0..N / 7 {
        let n = 1 + i % 7;
        let vs: Vec<[u32; 4]> = (0..n).map(|_| g.quad()).collect();
        let mm = g.mat();
        for (k, v) in vs.iter().enumerate() {
            put(&mut ee, src + 0x10 * k as u32, *v);
        }
        put_mat(&mut ee, m, mm);
        // A sentinel past the end, so a loop that runs one point too far is a failure and not a
        // silently-matching zero.
        put(&mut ee, out + 0x10 * n as u32, [0xdead_beef; 4]);
        ee.call(at, &[out as u64, src as u64, m as u64, n as u64, 0x10, 0], &[], 100_000);

        let mut ours = vec![Vf::ZERO; n];
        let src_vals: Vec<Vf> = vs.iter().map(|v| vf(*v)).collect();
        math::transform_points(&mut ours, &src_vals, &mat(mm));
        for k in 0..n {
            if !same("run", i * 7 + k, get(&ee, out + 0x10 * k as u32), ours[k].to_bits()) {
                wrong += 1;
                assert!(wrong < 6, "stopping after six");
            }
        }
        assert_eq!(get(&ee, out + 0x10 * n as u32), [0xdead_beef; 4], "the loop ran past its count");
    }
    assert_eq!(wrong, 0, "{wrong} transformed points differ");
}

/// The look-at frame, over all four combinations of its two flags.
#[test]
fn the_look_at_frame() {
    let Some(mut ee) = game_or_skip() else { return };
    let at = addr("mat4_basis").unwrap();
    let (out, pos, fwd, up) = (SCRATCH, SCRATCH + 0x40, SCRATCH + 0x50, SCRATCH + 0x60);
    let mut g = Gen(0xfeed_c0de);
    let mut wrong = 0;
    for i in 0..N {
        let (p, f, u) = (g.quad(), g.dir(), g.dir());
        let (orth, keep) = (i % 4 >= 2, i % 2 == 1);
        put(&mut ee, pos, p);
        put(&mut ee, fwd, f);
        put(&mut ee, up, u);
        ee.call(at, &[out as u64, pos as u64, fwd as u64, up as u64, orth as u64, keep as u64], &[], 10_000);
        let ours = math::basis(vf(p), vf(f), vf(u), orth, keep);
        if !same_mat("basis", i, get_mat(&ee, out), ours.0.map(|r| r.to_bits())) {
            wrong += 1;
            assert!(wrong < 6, "stopping after six");
        }
    }
    assert_eq!(wrong, 0, "{wrong} of {N} look-at frames differ");
}

// --- quaternion to matrix and back ------------------------------------------------------------------

/// A quaternion and a position into a matrix.
#[test]
fn a_quaternion_and_a_position_into_a_matrix() {
    let Some(mut ee) = game_or_skip() else { return };
    let at = addr("vu_quat_to_mat3").unwrap();
    let (out, q, pos) = (SCRATCH, SCRATCH + 0x40, SCRATCH + 0x50);
    let mut g = Gen(0x00c0_ffee);
    let mut wrong = 0;
    for i in 0..N {
        let (qq, pp) = (g.quat(), g.quad());
        put(&mut ee, q, qq);
        put(&mut ee, pos, pp);
        ee.call(at, &[out as u64, q as u64, pos as u64], &[], 10_000);
        let ours = math::quat_to_mat4(vf(qq), vf(pp));
        if !same_mat("quat->mat", i, get_mat(&ee, out), ours.0.map(|r| r.to_bits())) {
            wrong += 1;
            assert!(wrong < 6, "stopping after six");
        }
    }
    assert_eq!(wrong, 0, "{wrong} of {N} quaternion matrices differ");
}

/// The two rows on their own — the scalar routine, whose arithmetic is not the matrix builder's with
/// two rows deleted.
#[test]
fn two_rows_of_a_quaternions_matrix() {
    let Some(mut ee) = game_or_skip() else { return };
    let at = addr("quat_two_matrix_rows").unwrap();
    let (q, fwd, up) = (SCRATCH, SCRATCH + 0x10, SCRATCH + 0x20);
    let mut g = Gen(0x7e57_7e57);
    let mut wrong = 0;
    for i in 0..N {
        let qq = g.quat();
        put(&mut ee, q, qq);
        // A sentinel in the fourth word of each row: the original writes three floats, not four.
        put(&mut ee, fwd, [0, 0, 0, 0xdead_beef]);
        put(&mut ee, up, [0, 0, 0, 0xdead_beef]);
        ee.call(at, &[q as u64, fwd as u64, up as u64], &[], 10_000);
        let (of, ou) = math::quat_axes(vf(qq));
        let theirs = (get(&ee, fwd), get(&ee, up));
        let ours = (
            [of[0].to_bits(), of[1].to_bits(), of[2].to_bits(), 0xdead_beef],
            [ou[0].to_bits(), ou[1].to_bits(), ou[2].to_bits(), 0xdead_beef],
        );
        if !same("forward", i, theirs.0, ours.0) || !same("up", i, theirs.1, ours.1) {
            wrong += 1;
            assert!(wrong < 6, "stopping after six");
        }
    }
    assert_eq!(wrong, 0, "{wrong} of {N} axis pairs differ");
}

/// A matrix back into a quaternion. Half the inputs are real rotation matrices, built by the port's own
/// [`math::quat_to_mat4`] out of a unit quaternion, so that the four branches are reached the way the
/// game reaches them; the other half are arbitrary quadwords, which is what finds the ties and the
/// negative candidates whose square root the instruction takes of the magnitude.
#[test]
fn a_matrix_back_into_a_quaternion() {
    let Some(mut ee) = game_or_skip() else { return };
    let at = addr("mat4_to_quat").unwrap();
    let (out, m) = (SCRATCH, SCRATCH + 0x10);
    let mut g = Gen(0x0123_4567);
    let (mut wrong, mut branches) = (0, [0usize; 4]);
    for i in 0..N {
        let mm = if i % 2 == 0 {
            math::quat_to_mat4(vf(g.unit_quat()), Vf::ZERO).0.map(|r| r.to_bits())
        } else {
            g.mat()
        };
        put_mat(&mut ee, m, mm);
        ee.call(at, &[out as u64, m as u64], &[], 10_000);
        let ours = math::mat4_to_quat(&mat(mm));
        // Which component came out as the square root: the branch the original took.
        let which = (0..4).max_by_key(|&k| F::from_bits(ours.to_bits()[k]).abs().to_bits()).unwrap();
        branches[which] += 1;
        if !same("mat->quat", i, get(&ee, out), ours.to_bits()) {
            wrong += 1;
            assert!(wrong < 6, "stopping after six");
        }
    }
    println!("largest component was x/y/z/w in {branches:?} of {N} inputs");
    assert!(branches.iter().all(|&n| n > 0), "not every branch was reached: {branches:?}");
    assert_eq!(wrong, 0, "{wrong} of {N} quaternions differ");
}

// --- the two that move a quaternion -----------------------------------------------------------------

/// The angular-velocity derivative.
#[test]
fn the_quaternion_derivative() {
    let Some(mut ee) = game_or_skip() else { return };
    let at = addr("quat_deriv").unwrap();
    let (out, q, w) = (SCRATCH, SCRATCH + 0x10, SCRATCH + 0x20);
    let mut g = Gen(0xabcd_ef01);
    let mut wrong = 0;
    for i in 0..N {
        let (qq, ww) = (g.quat(), g.dir());
        put(&mut ee, q, qq);
        put(&mut ee, w, ww);
        ee.call(at, &[out as u64, q as u64, w as u64], &[], 10_000);
        let ours = math::quat_deriv(vf(qq), vf(ww));
        if !same("deriv", i, get(&ee, out), ours.to_bits()) {
            wrong += 1;
            assert!(wrong < 6, "stopping after six");
        }
    }
    assert_eq!(wrong, 0, "{wrong} of {N} derivatives differ");
}

/// The spherical interpolation, against the original reading the arc cosine table the *game's* own
/// startup code builds — the port is given [`AcosTable::build`]'s, so this proves the lookup and the
/// table together.
#[test]
fn the_spherical_interpolation() {
    let Some(mut ee) = game_or_skip() else { return };
    // Hand the game's allocator one fixed buffer: an oracle run starts with an empty heap, and the
    // table builder checks for null.
    ee.hooks.insert(addr("mem_alloc").unwrap(), Box::new(|ee| ee.set(2, TABLE as u64)));
    ee.call(addr("acos_table_build").unwrap(), &[], &[], 2_000_000_000);
    assert_eq!(ee.v0(), 1, "the game's arc cosine table builder reported failure");
    let table = AcosTable::build();

    let at = addr("quat_slerp").unwrap();
    let (out, a, b) = (SCRATCH, SCRATCH + 0x10, SCRATCH + 0x20);
    let mut g = Gen(0x1a2b_3c4d);
    let (mut wrong, mut guarded) = (0, 0);
    for i in 0..N {
        let aa = g.quat();
        // A quarter of the pairs are the same quaternion, or nearly so, which is the only way to reach
        // the |cos| >= 1 guard often enough to prove what it leaves behind.
        let bb = match g.bits() % 4 {
            0 => aa,
            1 => aa.map(|w| w ^ 0x8000_0000),
            _ => g.quat(),
        };
        let t = match g.bits() % 8 {
            0 => 0.0,
            1 => 1.0,
            2 => g.range(-2.0, 3.0),
            _ => g.unit(),
        };
        let shortest = g.b();
        put(&mut ee, a, aa);
        put(&mut ee, b, bb);
        ee.call(at, &[out as u64, a as u64, b as u64, shortest as u64], &[t], 10_000);
        let ours = math::slerp(vf(aa), vf(bb), F(t), shortest, &table);
        if get(&ee, out) == vf(aa).to_bits() {
            guarded += 1;
        }
        if !same("slerp", i, get(&ee, out), ours.to_bits()) {
            wrong += 1;
            assert!(wrong < 6, "stopping after six");
        }
    }
    println!("{guarded} of {N} inputs came back as `a` (the nearly-parallel guard, or t = 0)");
    assert!(guarded > 100, "the |cos| >= 1 guard was barely reached: {guarded}");
    assert_eq!(wrong, 0, "{wrong} of {N} interpolations differ");
}
