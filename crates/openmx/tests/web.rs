//! N4 — the shell in a browser: a wasm32 build in `./kit check`, and that build drawing what the
//! desktop draws from the same disc.
//!
//! * [`the_shell_builds_for_the_browser`] builds `crates/openmx/src/web.rs` for
//!   `wasm32-unknown-unknown` through `tools/web.sh` — the same script a release uses, wasm-bindgen
//!   and all — and reads the module's import section: everything it imports must come from
//!   wasm-bindgen's glue, and the glue must ask the page for exactly the host calls
//!   `web/worker.js` supplies ([`HOST_CALLS`]). An import from anywhere else (`env`: a C dependency's missing symbol,
//!   a WASI function) is a module the page cannot instantiate, and this is the only test that would
//!   see it.
//! * [`the_browser_draws_what_the_desktop_draws`] runs that module in node with `tests/web/host.mjs`
//!   standing in for the worker — the same host calls, a file in place of the OPFS handle — over the
//!   user's own disc, and compares its frame with [`openmx_web::start_line`] drawn natively. Rendering
//!   is "looks right" in this project (AGENT.md), so the comparison has a tolerance, and what it
//!   measured is printed.
//! * [`the_browser_rides_on_the_gpu`] opens the page in headless chromium on the machine's GPU and
//!   rides on WebGPU and on WebGL2 (P48): a lit frame, and the pictures a second it rode at.
//!
//! The build needs the wasm32 standard library. `tools/web.sh` takes it from this machine's
//! toolchain, or downloads the matching upstream release once into `$TMPDIR/openmx-cross`; with
//! neither (offline and nothing cached) the build fails and says so — it is not skipped, because a
//! skipped build gate is no gate. The second test skips without a disc or without node, and says so.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use game_render::disc::{Disc, configured_image};
use openmx_web::{TRACK, rgba, start_line};

/// The host calls `web/worker.js` supplies, on its global `openmx`: the disc, the picture and the
/// log (N4); and the game's canvas size, keys, pointer, gamepads, sound, files and frame clock
/// (P196, `src/web_host.rs`); and which GPU API draws and its first frame (P48, `src/web_gpu.rs`).
const HOST_CALLS: [&str; 16] = [
    "disc_size", "disc_read", "present", "log", "view", "keys", "clicks", "gamepads", "gamepad_name", "play", "queued", "file_read", "file_write", "frame", "gpu", "drawn",
];

/// The module wasm-bindgen's glue answers the imports under (`--target web`).
const GLUE: &str = "./openmx_web_bg.js";

/// What the page calls, through the glue: the game (P196), the picture, and the online ride
/// (`Online`'s methods).
const EXPORTS: [&str; 10] = ["memory", "openmx_run", "openmx_start", "openmx_frame", "online_new", "online_host", "online_join", "online_frame", "online_state", "online_ticks"];

/// The frame both sides draw: the PS2's own.
const SIZE: (usize, usize) = (640, 448);

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// `tools/web.sh --check`, once per test binary: the directory of the module and its glue
/// (`openmx_web_bg.wasm`, `openmx_web.js`).
pub fn built() -> &'static PathBuf {
    static WASM: OnceLock<PathBuf> = OnceLock::new();
    WASM.get_or_init(|| {
        let script = repo().join("tools/web.sh");
        let out = Command::new("sh").arg(&script).arg("--check").output().expect("tools/web.sh must be runnable");
        let text = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
        assert!(out.status.success(), "{} --check failed:\n{text}", script.display());
        let path = PathBuf::from(String::from_utf8_lossy(&out.stdout).lines().last().unwrap_or("").trim());
        assert!(path.join("openmx_web_bg.wasm").is_file() && path.join("openmx_web.js").is_file(), "tools/web.sh --check named no module:\n{text}");
        path
    })
}

/// A LEB128 unsigned integer at `at`, and where it ends.
fn leb(b: &[u8], mut at: usize) -> (u64, usize) {
    let (mut v, mut shift) = (0u64, 0);
    loop {
        let byte = b[at];
        at += 1;
        v |= u64::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return (v, at);
        }
        shift += 7;
    }
}

fn name(b: &[u8], at: usize) -> (String, usize) {
    let (len, at) = leb(b, at);
    let end = at + len as usize;
    (String::from_utf8_lossy(&b[at..end]).into_owned(), end)
}

/// A module's imports as (module, name, kind) and its exports' names, read off the binary format's
/// import (2) and export (7) sections. Only the parts this test looks at are decoded.
fn interface(wasm: &[u8]) -> (Vec<(String, String, u8)>, Vec<String>) {
    assert_eq!(&wasm[..8], b"\0asm\x01\0\0\0", "not a version-1 WebAssembly module");
    let (mut imports, mut exports) = (Vec::new(), Vec::new());
    let mut at = 8;
    while at < wasm.len() {
        let id = wasm[at];
        let (len, body) = leb(wasm, at + 1);
        let end = body + len as usize;
        match id {
            2 => {
                let (count, mut p) = leb(wasm, body);
                for _ in 0..count {
                    let (module, q) = name(wasm, p);
                    let (field, q) = name(wasm, q);
                    let kind = wasm[q];
                    // Only functions are expected; anything else has a description this does not walk.
                    assert_eq!(kind, 0, "{module}.{field} is imported as a non-function (kind {kind})");
                    let (_type_index, q) = leb(wasm, q + 1);
                    imports.push((module, field, kind));
                    p = q;
                }
            }
            7 => {
                let (count, mut p) = leb(wasm, body);
                for _ in 0..count {
                    let (field, q) = name(wasm, p);
                    let (_index, q) = leb(wasm, q + 1);
                    exports.push(field);
                    p = q;
                }
            }
            _ => {}
        }
        at = end;
    }
    (imports, exports)
}

#[test]
fn the_shell_builds_for_the_browser() {
    let wasm = std::fs::read(built().join("openmx_web_bg.wasm")).expect("read the module");
    let glue = std::fs::read_to_string(built().join("openmx_web.js")).expect("read the glue");
    let (imports, exports) = interface(&wasm);
    println!("{} bytes; {} imports; exports {:?}", wasm.len(), imports.len(), exports.iter().filter(|e| !e.starts_with("__") && !e.contains("closure")).collect::<Vec<_>>());
    for (module, field, _) in &imports {
        assert!(module == GLUE, "the browser build imports {module}.{field}, which nothing in the page supplies");
    }
    // the page's own calls, as the glue makes them: `openmx.<name>(`, and nothing else on `openmx`
    let mut asked: Vec<&str> = glue.split("openmx.").skip(1).filter_map(|r| r.split('(').next()).filter(|n| n.chars().all(|c| c == '_' || c.is_ascii_alphanumeric())).collect();
    asked.sort();
    asked.dedup();
    let mut want = HOST_CALLS.to_vec();
    want.sort();
    assert_eq!(asked, want, "the glue asks the page for other calls than web/worker.js supplies");
    for export in EXPORTS {
        assert!(exports.iter().any(|e| e == export), "the browser build does not export {export}");
    }
}

#[test]
fn the_browser_draws_what_the_desktop_draws() {
    let mut disc = match Disc::open_configured() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("skipped: {e}");
            return;
        }
    };
    if Command::new("node").arg("--version").output().is_err() {
        eprintln!("skipped: no node to stand in for the browser");
        return;
    }
    let iso = configured_image().expect("the disc opened, so its path resolves");
    let mut desktop = Vec::new();
    rgba(&start_line(&mut disc, TRACK, SIZE.0, SIZE.1).expect("the desktop draws the start line"), &mut desktop);

    let out = std::env::temp_dir().join(format!("openmx-web-{}.rgba", std::process::id()));
    let host = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/web/host.mjs");
    let run = Command::new("node")
        .arg(&host)
        .arg(built())
        .arg(&iso)
        .arg(SIZE.0.to_string())
        .arg(SIZE.1.to_string())
        .arg(&out)
        .output()
        .expect("run node");
    assert!(run.status.success(), "the browser build failed: {}", String::from_utf8_lossy(&run.stderr));
    let browser = std::fs::read(&out).expect("the host wrote the frame");
    let _ = std::fs::remove_file(&out);
    assert_eq!(browser.len(), desktop.len(), "the frames are different sizes");

    // A picture that is all fog would match itself on both sides and prove nothing.
    let fog = &desktop[..4];
    let drawn = desktop.chunks_exact(4).filter(|p| p != &fog).count();
    let pixels = desktop.len() / 4;
    assert!(drawn > pixels / 4, "only {drawn} of {pixels} pixels are not fog: the start line did not draw");

    let mut differ = 0;
    let mut worst = 0u8;
    for (a, b) in desktop.chunks_exact(4).zip(browser.chunks_exact(4)) {
        let d = a.iter().zip(b).map(|(x, y)| x.abs_diff(*y)).max().unwrap_or(0);
        if d > 0 {
            differ += 1;
        }
        worst = worst.max(d);
    }
    println!("{drawn} of {pixels} pixels drawn; {differ} differ between the builds, by at most {worst}");
    // Host `f32` on both sides, but not the same libm: the browser's sine and cosine are Rust's own,
    // the desktop's are the C library's, and a last-bit difference in the camera moves an edge.
    assert!(differ <= pixels / 100 && worst <= 64 || differ == 0, "{differ} pixels differ, by up to {worst}");
}

/// The page itself, in chromium: the first visit asks for the disc, copies it into OPFS and draws; a
/// reload draws the same picture straight off that copy without asking. And with OPFS refused (a
/// private window) the page plays off the picked file and asks again next time.
///
/// Ignored because each run copies the whole disc (2.2 GB) into a scratch browser profile under
/// `$TMPDIR` — seconds on a tmpfs, but not something every `./kit check` should do. Run it by hand:
///
/// `cargo test -p openmx --test web -- --ignored --nocapture the_page_starts_in_a_browser`
#[test]
#[ignore = "copies the disc into a scratch chromium profile; run it by hand"]
fn the_page_starts_in_a_browser() {
    let iso = match Disc::open_configured() {
        Ok(_) => configured_image().expect("the disc opened, so its path resolves"),
        Err(e) => panic!("{e}"),
    };
    // The page fetches the release build from web/pkg, which is what tools/web.sh (no --check) leaves.
    let script = repo().join("tools/web.sh");
    let out = Command::new("sh").arg(&script).output().expect("tools/web.sh must be runnable");
    assert!(out.status.success(), "tools/web.sh: {}", String::from_utf8_lossy(&out.stderr));

    let drive = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/web/chromium.mjs");
    let visit = |flag: Option<&str>| -> String {
        let profile = std::env::temp_dir().join(format!("openmx-web-profile-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&profile);
        let mut cmd = Command::new("node");
        cmd.arg(&drive).arg(repo().join("web")).arg(&iso).arg(&profile);
        if let Some(flag) = flag {
            cmd.arg(flag);
        }
        let run = cmd.output().expect("run node");
        let _ = std::fs::remove_dir_all(&profile);
        let text = String::from_utf8_lossy(&run.stdout).into_owned();
        println!("{}{text}", String::from_utf8_lossy(&run.stderr));
        assert!(run.status.success(), "chromium.mjs failed");
        text
    };
    // The JSON is flat enough to read with `split`; the workspace has no JSON reader to spend on it.
    let states = |text: &str| -> Vec<String> {
        text.split("\"state\":\"").skip(1).map(|s| s.split('"').next().unwrap_or("").to_string()).collect()
    };
    let shots = |text: &str| -> Vec<String> {
        text.split("\"sha256\":\"").skip(1).map(|s| s.split('"').next().unwrap_or("").to_string()).collect()
    };

    let stored = visit(None);
    assert_eq!(states(&stored), ["started", "started"], "the OPFS copy did not carry the second visit");
    let s = shots(&stored);
    assert_eq!(s[0], s[1], "the second visit drew a different picture");

    let private = visit(Some("--no-opfs"));
    assert_eq!(states(&private), ["started", "asking"], "with OPFS refused the page should play off the file and ask again");
    assert_eq!(shots(&private)[0], s[0], "the picked file drew a different picture from the OPFS copy");
}

/// The launcher (P181) in headless chromium at a desktop's width and a phone's
/// (`tests/web/shots.mjs`): with nothing stored the page is the landing page and asks for the
/// disc; with a disc in OPFS it opens on the menu stage with Ride lit. Neither look may pan
/// sideways, and both must be in the menus' own face. Needs no disc and no game module: the stored
/// disc is a stand-in neither look reads. Skips without node or chromium. With `OPENMX_WEB_SHOTS`
/// set to a directory the shots are kept there, to hold beside the concept.
#[test]
fn the_launcher_fits_a_phone_and_a_desktop() {
    for tool in ["node", "chromium"] {
        if Command::new(tool).arg("--version").output().is_err() {
            eprintln!("skipped: no {tool}");
            return;
        }
    }
    let scratch = std::env::temp_dir().join(format!("openmx-web-shots-{}", std::process::id()));
    let out = std::env::var_os("OPENMX_WEB_SHOTS").map(PathBuf::from).unwrap_or_else(|| scratch.join("out"));
    let drive = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/web/shots.mjs");
    let run = Command::new("node").arg(&drive).arg(repo().join("web")).arg(&out).arg(scratch.join("profile")).output().expect("run node");
    let _ = std::fs::remove_dir_all(&scratch);
    let text = String::from_utf8_lossy(&run.stdout).into_owned();
    println!("{}{text}", String::from_utf8_lossy(&run.stderr));
    assert!(run.status.success(), "shots.mjs failed");

    // Flat JSON, read with `split` as the_page_starts_in_a_browser does: one object per shot.
    let shots: Vec<&str> = text.split("{\"state\":").skip(1).collect();
    assert_eq!(shots.len(), 4, "two looks at two widths:\n{text}");
    let field = |shot: &str, key: &str| -> String {
        shot.split(&format!("\"{key}\":")).nth(1).unwrap_or("").split([',', '}']).next().unwrap_or("").trim_matches('"').to_string()
    };
    for (shot, (state, width)) in shots.iter().zip([("asking", 1440), ("menu", 1440), ("asking", 390), ("menu", 390)]) {
        assert!(shot.starts_with(&format!("\"{state}\"")), "wanted {state} at {width}: {shot}");
        assert_eq!(field(shot, "width"), width.to_string());
        let scroll: u32 = field(shot, "scroll").parse().expect("scrollWidth");
        assert!(scroll <= width, "{state} at {width} pans sideways: {scroll} wide");
        assert_eq!(field(shot, "fonts"), "true", "{state} at {width}: Saira did not load");
        let lit = field(shot, "lit");
        assert_eq!(lit, if state == "menu" { "Play" } else { "null" }, "{state} at {width}: the Play button");
    }
}

/// The on-screen pad (P197) in headless chromium with a phone's touch screen emulated
/// (`tests/web/touch.mjs`): none on the site; once the game has started, every control on the
/// screen, none over another and none smaller than a fingertip; the stick, GAS and BRAKE held by
/// three fingers at once reach `navigator.getGamepads()` as the standard mapping's axes 0/1 and R2
/// and L2, and all let go with the fingers; a quick tap on the stick's area is a click on the
/// canvas (the menus'); a drag scrolls nothing and the viewport refuses zoom; upright the pad
/// gives way to the hint to turn the phone; a hardware key hides it and a touch brings it back.
/// Needs no disc and no game module. Skips without node or chromium; with `OPENMX_WEB_SHOTS` set
/// to a directory its two shots are kept there.
#[test]
fn the_touch_pad_rides_on_a_phone() {
    for tool in ["node", "chromium"] {
        if Command::new(tool).arg("--version").output().is_err() {
            eprintln!("skipped: no {tool}");
            return;
        }
    }
    let scratch = std::env::temp_dir().join(format!("openmx-web-touch-{}", std::process::id()));
    let out = std::env::var_os("OPENMX_WEB_SHOTS").map(PathBuf::from).unwrap_or_else(|| scratch.join("out"));
    let drive = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/web/touch.mjs");
    let run = Command::new("node").arg(&drive).arg(repo().join("web")).arg(&out).arg(scratch.join("profile")).output().expect("run node");
    let _ = std::fs::remove_dir_all(&scratch);
    let text = String::from_utf8_lossy(&run.stdout).into_owned();
    println!("{}{text}", String::from_utf8_lossy(&run.stderr));
    assert!(run.status.success(), "touch.mjs failed");

    // Flat-ish JSON read with `split`, as the other page tests do.
    let field = |key: &str| -> String {
        let rest = text.split(&format!("\"{key}\":")).nth(1).unwrap_or("");
        if rest.starts_with('{') { rest[..=rest.find('}').unwrap_or(0)].to_string() } else { rest.split([',', '}']).next().unwrap_or("").trim_matches('"').to_string() }
    };
    assert_eq!(field("site"), "false", "the site has no pad: {text}");
    assert_eq!(field("shown"), "true", "a started game on a touch screen shows the pad: {text}");
    assert_eq!(field("buttons"), "10", "{text}");
    assert_eq!(field("inside"), "true", "a control off the screen: {text}");
    assert_eq!(field("overlaps"), "0", "controls on top of each other: {text}");
    assert_eq!(field("small"), "0", "a control smaller than a fingertip: {text}");
    let axes = |key: &str| -> Vec<f64> {
        let f = field(key);
        f.split("\"axes\":[").nth(1).unwrap_or("").split(']').next().unwrap_or("").split(',').filter_map(|v| v.parse().ok()).collect()
    };
    let down = |key: &str| -> String { field(key).split("\"down\":").nth(1).unwrap_or("").trim_end_matches('}').to_string() };
    for key in ["steer", "gas", "both"] {
        let a = axes(key);
        assert!(a.len() == 2 && a[0] > 0.9 && a[1] > 0.1 && a[1] < 0.4, "{key}: the stick pushed right and a little down: {a:?}");
    }
    assert_eq!(down("steer"), "[]", "the stick presses no button");
    assert_eq!(down("gas"), "[7]", "GAS is R2");
    assert_eq!(down("both"), "[6,7]", "BRAKE is L2, with GAS still held");
    assert_eq!(axes("released"), [0.0, 0.0], "the stick centres when the thumb lifts");
    assert_eq!(down("released"), "[]", "every button up with the fingers");
    assert_eq!(field("tap"), "1", "a quick tap on the stick's area clicks the canvas once");
    assert_eq!(text.split("\"scroll\":[").nth(1).unwrap_or("").split(']').next(), Some("0,0,0,0"), "the page scrolled: {text}");
    assert_eq!(field("zoom"), "true", "the viewport still zooms while riding");
    let portrait = field("portrait");
    assert!(portrait.contains("\"pad\":\"none\"") && portrait.contains("\"hint\":\"flex\""), "upright: {portrait}");
    assert_eq!(field("keyboard"), "false", "a hardware key hides the pad");
    assert_eq!(field("again"), "true", "a touch brings it back");
}

/// The ride on the GPU in headless chromium (P48, `tests/web/gpu.mjs`): the page opened with
/// `?args=--ride`, once as it comes (WebGPU) and once with `?webgl2`, each on this machine's GPU
/// (chromium's own Vulkan, not SwiftShader). Each must draw on the API asked for — not fall back to
/// the software picture — show a frame that is not black, and ride at 30 pictures a second or more,
/// as the worker counts them; what each rode at is printed. The module is `tools/web.sh --check`'s,
/// served as the page's pkg/. Skips without the disc, node or chromium.
#[test]
fn the_browser_rides_on_the_gpu() {
    let iso = match Disc::open_configured() {
        Ok(_) => configured_image().expect("the disc opened, so its path resolves"),
        Err(e) => {
            eprintln!("skipped: {e}");
            return;
        }
    };
    for tool in ["node", "chromium"] {
        if Command::new(tool).arg("--version").output().is_err() {
            eprintln!("skipped: no {tool}");
            return;
        }
    }
    // Headless chromium draws WebGL2 on SwiftShader (~2 fps for this ride) and has no WebGPU at all
    // unless told to use the GPU: these are the flags that give it the machine's own.
    const FLAGS: [&str; 4] = ["--enable-unsafe-webgpu", "--enable-features=Vulkan", "--use-angle=vulkan", "--ignore-gpu-blocklist"];
    let drive = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/web/gpu.mjs");
    let field = |text: &str, key: &str| -> String {
        text.split(&format!("\"{key}\":")).nth(1).unwrap_or("").split([',', '}']).next().unwrap_or("").trim_matches('"').to_string()
    };
    for (query, api) in [("", "webgpu"), ("webgl2", "webgl2")] {
        let scratch = std::env::temp_dir().join(format!("openmx-web-gpu-{}-{api}", std::process::id()));
        let _ = std::fs::remove_dir_all(&scratch);
        let run = Command::new("node")
            .arg(&drive)
            .arg(repo().join("web"))
            .arg(&iso)
            .arg(scratch.join("profile"))
            .arg(scratch.join("ride.png"))
            .arg(query)
            .args(FLAGS)
            .env("OPENMX_GPU_PKG", built())
            .env("OPENMX_GPU_SECONDS", "10")
            .output()
            .expect("run node");
        let _ = std::fs::remove_dir_all(&scratch);
        let text = String::from_utf8_lossy(&run.stdout).into_owned();
        assert!(run.status.success(), "gpu.mjs ({api}) failed:\n{}{text}", String::from_utf8_lossy(&run.stderr));
        assert_eq!(field(&text, "gpu"), api, "the page did not draw on {api}:\n{text}");
        assert!(field(&text, "drawn").starts_with(api), "no first {api} frame:\n{text}");
        let lit: f64 = field(&text, "lit").parse().expect("lit");
        let pixels: f64 = field(&text, "pixels").parse().expect("pixels");
        assert!(lit > pixels / 2.0, "{api}: only {lit} of {pixels} pixels are not black:\n{text}");
        // The first seconds are the module settling and the world streaming in; the rest is the ride.
        let list = text.split("\"fps\":[").nth(1).and_then(|s| s.split(']').next()).unwrap_or("");
        let mut fps: Vec<f64> = list.split(',').filter_map(|f| f.parse().ok()).skip(2).collect();
        assert!(!fps.is_empty(), "{api}: no pictures a second counted:\n{text}");
        fps.sort_by(f64::total_cmp);
        let median = fps[fps.len() / 2];
        println!("{api}: {median} fps (median of {:?}), {lit} of {pixels} pixels lit", fps);
        assert!(median >= 30.0, "{api} rides at {median} fps, under 30:\n{text}");
    }
}
