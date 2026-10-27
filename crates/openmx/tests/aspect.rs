//! V24 — the picture at any shape: the frame a window gets, each split view's size, the field of
//! view each view is drawn with, which of the original's two displays the frame stands for, and where
//! the overlay is pinned inside each view.
//!
//! The windows are a 4:3 one, a 16:9 one, a 21:9 one and a portrait one, each alone and split two
//! ways (both of the original's layouts) and four. The fields of view are read back out of the
//! projection the renderer builds, not out of the function that made them, so what is tested is what
//! a view is actually drawn with.

#[allow(dead_code)]
#[path = "../src/aspect.rs"]
mod aspect;
#[allow(dead_code)]
#[path = "../src/split.rs"]
mod split;

use aspect::{Aspect, Corner, Display};
use game_render::mat::v3;
use game_render::world::{Camera, vertical_fov};
use split::{Layout, Rect, Split};

/// The preset field of view every track's `Preset1` asks for, horizontal, in degrees.
const PRESET_FOV: f32 = 85.0;

/// The four windows, by name and size.
const WINDOWS: [(&str, (usize, usize)); 4] =
    [("4:3", (1440, 1080)), ("16:9", (1920, 1080)), ("21:9", (2560, 1080)), ("portrait", (1080, 1920))];

/// Alone, split two ways each way, and four ways.
fn layouts() -> [Layout; 4] {
    [
        Layout::for_players(1, Split::Stacked).unwrap(),
        Layout::for_players(2, Split::Stacked).unwrap(),
        Layout::for_players(2, Split::SideBySide).unwrap(),
        Layout::for_players(4, Split::Stacked).unwrap(),
    ]
}

/// The angles a view is drawn with, (horizontal, vertical) in degrees, measured off the projection:
/// where a point at the edge of the view lands in clip space, so a squashed projection — one built
/// for another shape than the view's — cannot hide behind the right number.
fn drawn_angles(view: Rect) -> (f32, f32) {
    let camera = Camera {
        eye: v3(0.0, 0.0, 0.0),
        at: v3(0.0, 0.0, 1.0),
        fov: aspect::fov(PRESET_FOV, view.w, view.h),
        near: 0.1,
        far: 100.0,
    };
    let m = camera.view_projection(view.w, view.h);
    // How far across the view, as a fraction of its half-width or half-height, a point one foot off
    // the axis at ten feet lands; the half-angle is the one whose tangent puts it at the edge.
    let across = |p| {
        let c = m.clip(p);
        (c[0] / c[3]).abs().max((c[1] / c[3]).abs())
    };
    let half_h = (0.1f32 / across(v3(1.0, 0.0, 10.0))).atan();
    let half_v = (0.1f32 / across(v3(0.0, 1.0, 10.0))).atan();
    (2.0 * half_h.to_degrees(), 2.0 * half_v.to_degrees())
}

#[test]
fn the_frame_fills_the_window_and_four_by_three_keeps_its_bars() {
    let want_fill = [(1440, 1080), (1920, 1080), (2560, 1080), (1080, 1920)];
    let want_tv = [(1440, 1080), (1440, 1080), (1440, 1080), (1080, 810)];
    for (i, (name, window)) in WINDOWS.iter().enumerate() {
        assert_eq!(Aspect::Fill.frame(*window, 1), want_fill[i], "{name} filled");
        assert_eq!(Aspect::Television.frame(*window, 1), want_tv[i], "{name} at 4:3");
    }
    // The 4:3 fit, as it always was (these were window.rs's before V24).
    assert_eq!(aspect::fit_frame((1920, 1080), 1), (1440, 1080)); // pillarboxed
    assert_eq!(aspect::fit_frame((640, 480), 1), (640, 480)); // exactly
    assert_eq!(aspect::fit_frame((800, 1000), 1), (800, 600)); // letterboxed
    assert_eq!(aspect::fit_frame((1280, 720), 1), (960, 720));
    assert_eq!(aspect::fit_frame((0, 0), 1), (1, 1));
    // `--scale 2` draws half and doubles it, at either shape.
    assert_eq!(Aspect::Fill.frame((1920, 1080), 2), (960, 540));
    assert_eq!(Aspect::Television.frame((1920, 1080), 2), (720, 540));
    // A window shrunk to nothing still has a pixel to draw.
    assert_eq!(Aspect::Fill.frame((0, 0), 1), (1, 1));
    // The default is fill, and the setting reads both ways it is written.
    assert_eq!(Aspect::default(), Aspect::Fill);
    assert_eq!(Aspect::parse("4:3"), Ok(Aspect::Television));
    assert_eq!(Aspect::parse("FILL"), Ok(Aspect::Fill));
    assert!(Aspect::parse("3:2").is_err());
}

/// P176: opentennis's choices. Each fixed shape is the largest frame of it inside every window, with
/// bars round it (pillarboxed in a wider window, letterboxed in a taller one, portrait too); Auto and
/// Stretch fill it; the frame stands for whichever display its shape is nearer; and the split views,
/// laid out in the frame, share it.
#[test]
fn each_fixed_shape_is_the_largest_one_inside_the_window() {
    let words = ["auto", "4:3", "16:9", "16:10", "21:9", "stretch"];
    assert_eq!(Aspect::ALL.map(|a| Aspect::parse(a.name()).unwrap()), Aspect::ALL);
    assert_eq!(words.map(|w| Aspect::parse(w).unwrap()), Aspect::ALL);
    // (window) -> the frame at 4:3, 16:9, 16:10, 21:9.
    let want: [[(usize, usize); 4]; 4] = [
        [(1440, 1080), (1440, 810), (1440, 900), (1440, 617)],
        [(1440, 1080), (1920, 1080), (1728, 1080), (1920, 822)],
        [(1440, 1080), (1920, 1080), (1728, 1080), (2520, 1080)],
        [(1080, 810), (1080, 607), (1080, 675), (1080, 462)],
    ];
    let fixed = [Aspect::Television, Aspect::Wide, Aspect::Monitor, Aspect::Cinema];
    for (i, (name, window)) in WINDOWS.iter().enumerate() {
        assert_eq!(Aspect::Stretch.frame(*window, 1), Aspect::Fill.frame(*window, 1), "{name}");
        for (j, a) in fixed.into_iter().enumerate() {
            let (w, h) = a.frame(*window, 1);
            assert_eq!((w, h), want[i][j], "{name} at {}", a.name());
            // Inside the window, and touching two of its sides: the bars are on the other two.
            assert!(w <= window.0 && h <= window.1 && (w == window.0 || h == window.1), "{name} {}", a.name());
            // Its shape, to the pixel the rounding down leaves.
            let (x, y) = a.ratio().unwrap();
            assert!((w * y).abs_diff(h * x) < x.max(y), "{name} {}: {w}x{h}", a.name());
            // 4:3 is the television, every wider shape the 16:9 display.
            let display = if a == Aspect::Television { Display::Television } else { Display::Wide };
            assert_eq!(Display::of(w, h, a), display, "{name} {}", a.name());
            // The split views tile the fixed frame, not the window.
            for layout in layouts() {
                let rects = layout.rects(w, h);
                assert_eq!(rects.iter().map(|r| r.w * r.h).sum::<usize>(), w * h, "{name} {} {layout:?}", a.name());
                assert!(rects.iter().all(|r| r.x + r.w <= w && r.y + r.h <= h), "{name} {} {layout:?}", a.name());
            }
        }
    }
    // `--scale 2` halves it, as at 4:3.
    assert_eq!(Aspect::Wide.frame((2560, 1080), 2), (960, 540));
    assert_eq!(Aspect::Cinema.frame((0, 0), 1), (1, 1));
}

#[test]
fn every_view_takes_its_own_share_of_the_window() {
    // (window, layout) -> the first view's size, every other view the same at these even sizes.
    let want: [[(usize, usize); 4]; 4] = [
        // alone, stacked, side by side, quarters
        [(1440, 1080), (1440, 540), (720, 1080), (720, 540)],
        [(1920, 1080), (1920, 540), (960, 1080), (960, 540)],
        [(2560, 1080), (2560, 540), (1280, 1080), (1280, 540)],
        [(1080, 1920), (1080, 960), (540, 1920), (540, 960)],
    ];
    for (i, (name, window)) in WINDOWS.iter().enumerate() {
        let (w, h) = Aspect::Fill.frame(*window, 1);
        for (j, layout) in layouts().into_iter().enumerate() {
            let rects = layout.rects(w, h);
            for r in &rects {
                assert_eq!((r.w, r.h), want[i][j], "{name} {layout:?}");
            }
            // And they cover the frame.
            assert_eq!(rects.iter().map(|r| r.w * r.h).sum::<usize>(), w * h, "{name} {layout:?}");
        }
    }
}

/// Inside the shapes the original drew — 2:3 to 32:9 — every view keeps the preset's horizontal
/// angle, which is the original's rule on its television and in its own 16:9 mode alike, and the
/// vertical one is whatever the view's own shape leaves. Outside them the short side's angle holds
/// at the range's edge and the long side widens.
#[test]
fn each_view_is_drawn_at_its_own_shape() {
    let tv_vertical = vertical_fov(PRESET_FOV, 4.0 / 3.0).to_degrees();
    // The disc's own number: 85 across at 4:3 is the 68.997 down the human's save state drew with.
    assert!((tv_vertical - 68.997).abs() < 0.01, "{tv_vertical}");
    let widest_vertical = vertical_fov(PRESET_FOV, 32.0 / 9.0).to_degrees();
    let narrowest_vertical = vertical_fov(PRESET_FOV, 2.0 / 3.0).to_degrees();
    for (name, window) in WINDOWS {
        let (w, h) = Aspect::Fill.frame(window, 1);
        for layout in layouts() {
            for view in layout.rects(w, h) {
                let shape = view.w as f32 / view.h as f32;
                let (across, down) = drawn_angles(view);
                // Square pixels: the two angles agree with the view's own shape.
                let want_across =
                    2.0 * ((down / 2.0).to_radians().tan() * shape).atan().to_degrees();
                assert!((across - want_across).abs() < 0.05, "{name} {layout:?} {view:?}: squashed");
                let what = format!("{name} {layout:?} {}x{}: {across:.2} across, {down:.2} down", view.w, view.h);
                if shape > 32.0 / 9.0 {
                    assert!((down - widest_vertical).abs() < 0.05, "{what}");
                    assert!(across > PRESET_FOV + 1.0, "{what}");
                } else if shape < 2.0 / 3.0 {
                    assert!((down - narrowest_vertical).abs() < 0.05, "{what}");
                    assert!(across < PRESET_FOV - 1.0, "{what}");
                } else {
                    assert!((across - PRESET_FOV).abs() < 0.05, "{what}");
                }
                if (shape - 4.0 / 3.0).abs() < 1e-3 {
                    assert!((down - tv_vertical).abs() < 0.05, "{what}");
                }
            }
        }
    }
    // The cases by name, so the rule's two edges are each met at least once: the 21:9 stacked
    // strip (2560x540) is past 32:9, and the portrait window's side-by-side half (540x1920) is
    // narrower than 2:3, and so is the portrait window itself.
    assert!((drawn_angles(Rect { x: 0, y: 0, w: 2560, h: 540 }).1 - widest_vertical).abs() < 0.05);
    assert!((drawn_angles(Rect { x: 0, y: 0, w: 1080, h: 1920 }).1 - narrowest_vertical).abs() < 0.05);
    // The 16:9 window alone: the preset's 85 across, and less down than the television's.
    let (across, down) = drawn_angles(Rect { x: 0, y: 0, w: 1920, h: 1080 });
    assert!((across - 85.0).abs() < 0.05 && down < tv_vertical - 10.0, "{across} {down}");
}

#[test]
fn the_frame_stands_for_the_nearer_of_the_two_displays() {
    let of = |w, h| Display::of(w, h, Aspect::Fill);
    assert_eq!(of(1440, 1080), Display::Television);
    assert_eq!(of(640, 448), Display::Television, "the PS2's own frame");
    assert_eq!(of(1500, 1000), Display::Television, "3:2");
    assert_eq!(of(1080, 1920), Display::Television, "portrait");
    assert_eq!(of(1920, 1200), Display::Wide, "16:10");
    assert_eq!(of(1920, 1080), Display::Wide);
    assert_eq!(of(2560, 1080), Display::Wide, "21:9");
    // `--aspect 4:3` is the television whatever the frame.
    assert_eq!(Display::of(1920, 1080, Aspect::Television), Display::Television);
}

/// The original's loader: by layout on the television, and at 16:9 `w` for the full screen and the
/// stacked split, `x` for side by side, `z` for the rest.
#[test]
fn each_display_reads_its_loaders_preset_copies() {
    let [full, stacked, side, quarters] = layouts();
    let tv = Display::Television;
    let wide = Display::Wide;
    assert_eq!(
        [full, stacked, side, quarters].map(|l| aspect::preset_suffix(l, tv)),
        ["", "h", "v", "q"]
    );
    assert_eq!(
        [full, stacked, side, quarters].map(|l| aspect::preset_suffix(l, wide)),
        ["w", "w", "x", "z"]
    );
}

/// The stacked split's pitch trim: its strips are short on the television and get it, and nothing
/// gets it at 16:9, where the original skips the test.
#[test]
fn only_the_televisions_short_views_trim_their_pitch() {
    for (name, window) in WINDOWS {
        let (w, h) = Aspect::Fill.frame(window, 1);
        let display = Display::of(w, h, Aspect::Fill);
        for layout in layouts() {
            let view = layout.rects(w, h)[0];
            let short = aspect::short_viewport(display, view.w, view.h);
            let want = display == Display::Television && (view.h as f32) < 0.55 * view.w as f32;
            assert_eq!(short, want, "{name} {layout:?}");
        }
    }
    // The original's own case: its 640x224 strip on the television.
    assert!(aspect::short_viewport(Display::Television, 640, 224));
    assert!(!aspect::short_viewport(Display::Wide, 640, 224));
}

/// Something pinned to a corner sits that corner's margin in from it in every view, whatever the
/// view's shape — never in a 4:3 box inside a wide view, never scaled.
#[test]
fn a_corner_is_the_views_own_corner() {
    let size = (200, 100);
    for (name, window) in WINDOWS {
        let (w, h) = Aspect::Fill.frame(window, 1);
        for layout in layouts() {
            for view in layout.rects(w, h) {
                let at = |c| aspect::corner((view.w, view.h), size, c, 4);
                let (vw, vh) = (view.w as i32, view.h as i32);
                assert_eq!(at(Corner::TopLeft), (4, 4), "{name} {layout:?}");
                assert_eq!(at(Corner::TopRight), (vw - 204, 4), "{name} {layout:?}");
                assert_eq!(at(Corner::BottomLeft), (4, vh - 104), "{name} {layout:?}");
                assert_eq!(at(Corner::BottomRight), (vw - 204, vh - 104), "{name} {layout:?}");
            }
        }
    }
    // A view too small to hold it keeps it at the top left and clips the far side.
    assert_eq!(aspect::corner((100, 50), size, Corner::BottomRight, 4), (4, 4));
}
