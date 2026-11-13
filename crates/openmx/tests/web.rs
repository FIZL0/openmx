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
/// (P196, `src/web_host.rs`); and which GPU API draws and its first frame (P48, `src/web_gpu.rs`); and the mod
/// store's downloads into OPFS, its unpacking and a page opened for Report (P198, `src/modstore.rs`); and the
/// newer build the page saw, the last one started and a reload (P193); and its Hugging Face token kept by
/// the page and what is pasted into it (P199, `src/modstore_up.rs`); and a folder of its files listed (P217).
const HOST_CALLS: [&str; 34] = [
    "disc_size", "disc_read", "present", "log", "view", "keys", "clicks", "mouse", "gamepads", "gamepad_name", "play", "queued", "file_read", "file_write", "frame", "gpu", "drawn",
    "store_fetch", "store_got", "store_read", "store_close", "store_put", "store_drop", "open_url", "latest", "last_build", "reload",
    "token", "token_keep", "pasted",
    // P223: the device as the page sees it, and the display's measured rate.
    "device", "refresh",
    // P217: the installed mods (OPFS `mods/`) listed for the shell's mods readers.
    "file_tree", "file_size",
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
        // cargo hands a test the build script's rustc-env, OPENMX_VERSION among them, which would stamp the module a
        // release (OPENMX_TAG, its splash's corner a tag, F14): it is a dev build, as the tests' own
        let out = Command::new("sh").arg(&script).arg("--check").env_remove("OPENMX_VERSION").output().expect("tools/web.sh must be runnable");
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
    assert_eq!(field("clear"), "true", "CLUTCH on the score bar, or MODIFIER on the camera's name: {text}");
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
/// `?args=--ride`, once as it comes (WebGPU), once with `?webgl2`, and once as a phone's page on
/// WebGPU at the phone's whole canvas (P205), each on this machine's GPU
/// (chromium's own Vulkan, not SwiftShader). Each must draw on the API asked for — not fall back to
/// the software picture — show a frame that is not black, and ride at 30 pictures a second or more,
/// as the worker counts them; what each rode at is printed. Each must also start one of the disc's
/// songs, which the page's console says. The module is `tools/web.sh --check`'s,
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
    // And a phone's page (P205): an iPhone's 844×390 at 3 device pixels to one, drawn at 2 device
    // pixels to one (P223, opentennis's B547: 1688×780) — not the software picture's 640×448 cap —
    // with the phone's defaults.
    let phone = "844,390,3";
    for (query, api, viewport) in [("", "webgpu", ""), ("webgl2", "webgl2", ""), ("", "webgpu", phone)] {
        let api_seen = if viewport.is_empty() { api.to_string() } else { format!("{api} on a phone") };
        let scratch = std::env::temp_dir().join(format!("openmx-web-gpu-{}-{api}{}", std::process::id(), if viewport.is_empty() { "" } else { "-phone" }));
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
            .env("OPENMX_GPU_PHONE", viewport)
            .output()
            .expect("run node");
        let _ = std::fs::remove_dir_all(&scratch);
        let text = String::from_utf8_lossy(&run.stdout).into_owned();
        assert!(run.status.success(), "gpu.mjs ({api}) failed:\n{}{text}", String::from_utf8_lossy(&run.stderr));
        // The disc's music plays in the page: a song is opened again from the page's disc when it
        // starts, which a disc path the browser cannot open as a file left silent (no songs, the menus'
        // sounds still playing).
        let log = String::from_utf8_lossy(&run.stderr);
        assert!(log.contains("music: playing disc:"), "{api_seen}: no song of the disc's started:\n{log}");
        assert_eq!(field(&text, "gpu"), api, "the page did not draw on {api}:\n{text}");
        assert!(field(&text, "drawn").starts_with(api), "no first {api} frame:\n{text}");
        if !viewport.is_empty() {
            assert_eq!(field(&text, "drawn"), format!("{api} 1688 780"), "the phone's frame is not its canvas's size at 2 pixels to one:\n{text}");
        }
        let lit: f64 = field(&text, "lit").parse().expect("lit");
        let pixels: f64 = field(&text, "pixels").parse().expect("pixels");
        assert!(lit > pixels / 2.0, "{api}: only {lit} of {pixels} pixels are not black:\n{text}");
        // The first seconds are the module settling and the world streaming in; the rest is the ride.
        let list = text.split("\"fps\":[").nth(1).and_then(|s| s.split(']').next()).unwrap_or("");
        let mut fps: Vec<f64> = list.split(',').filter_map(|f| f.parse().ok()).skip(2).collect();
        assert!(!fps.is_empty(), "{api}: no pictures a second counted:\n{text}");
        fps.sort_by(f64::total_cmp);
        let median = fps[fps.len() / 2];
        println!("{api_seen}: {median} fps (median of {:?}), {lit} of {pixels} pixels lit", fps);
        assert!(median >= 30.0, "{api_seen} rides at {median} fps, under 30:\n{text}");
    }
}

/// A WebGPU validation error on the menu (P257, `tests/web/gpu.mjs`'s fault): a phone's Safari
/// stopped on 'gpu: device mismatch' with the menu's sound playing over a dead canvas, the page's
/// fallback to WebGL2 being for failures before the first frame alone. The page opened to the front
/// end on WebGPU, and after 30 WebGPU frames a texture past every limit asked for in the worker: the
/// shell must end saying the front end failed on WebGPU, the page start again on `?webgl2`, and the
/// menu draw there, lit. The module is `tools/web.sh --check`'s. Skips without the disc, node or
/// chromium.
#[test]
fn a_webgpu_error_on_the_menu_starts_again_on_webgl2() {
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
    const FLAGS: [&str; 4] = ["--enable-unsafe-webgpu", "--enable-features=Vulkan", "--use-angle=vulkan", "--ignore-gpu-blocklist"];
    let scratch = std::env::temp_dir().join(format!("openmx-web-gpu-fault-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&scratch);
    let run = Command::new("node")
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/web/gpu.mjs"))
        .arg(repo().join("web"))
        .arg(&iso)
        .arg(scratch.join("profile"))
        .arg(scratch.join("menu.png"))
        .arg("")
        .args(FLAGS)
        .env("OPENMX_GPU_PKG", built())
        .env("OPENMX_GPU_SECONDS", "4")
        .env("OPENMX_GPU_ARGS", "")
        .env("OPENMX_GPU_FAULT", "30")
        .env("OPENMX_GPU_WANT", "webgl2")
        .env("OPENMX_GPU_LOG", "1")
        .output()
        .expect("run node");
    let _ = std::fs::remove_dir_all(&scratch);
    let text = String::from_utf8_lossy(&run.stdout).into_owned();
    let log = String::from_utf8_lossy(&run.stderr);
    assert!(run.status.success(), "gpu.mjs failed:\n{log}{text}");
    assert!(log.contains("fault: a texture past every limit"), "the fault never ran on WebGPU:\n{log}");
    assert!(log.contains("gpu: validation error in "), "the worker did not say where the error came from:\n{log}");
    assert!(log.contains("openmx: webgpu failed in the front end ("), "the page did not start again for the front end's error:\n{log}");
    let field = |key: &str| -> String {
        text.split(&format!("\"{key}\":")).nth(1).unwrap_or("").split([',', '}']).next().unwrap_or("").trim_matches('"').to_string()
    };
    assert_eq!(field("gpu"), "webgl2", "the page did not start again on webgl2:\n{text}");
    assert!(field("drawn").starts_with("webgl2"), "no webgl2 frame:\n{text}");
    let lit: f64 = field("lit").parse().expect("lit");
    let pixels: f64 = field("pixels").parse().expect("pixels");
    assert!(lit > pixels / 4.0, "only {lit} of {pixels} pixels of the menu are lit:\n{text}");
    println!("webgpu's fault on the menu: started again on {}, {lit} of {pixels} pixels lit", field("drawn"));
    // P257b: the page's log (`?fps`) kept the failed WebGPU run across the restart: the worker's own line naming the
    // error, then the page that started on webgl2.
    let kept = text.split("\"log\":\"").nth(1).unwrap_or("");
    let named = kept.find("worker log: gpu: validation error in ");
    let again = kept.find("---- the page loaded again ----");
    assert!(named.is_some() && again.is_some() && named < again, "the log lost the WebGPU run across the restart:\n{kept}");
    assert!(kept[again.unwrap_or(0)..].contains("&webgl2"), "the log has no page on webgl2 after the restart:\n{kept}");
}

/// A phone's race on sx01 on WebGPU (P336, `tests/web/gpu.mjs`): the user's iPhone's Safari ended every ride's first
/// frame with 'gpu: encoder state is invalid', WebKit's words for an encoder some command in it broke, whatever it
/// was. The page opened as a phone's (844×390 at 3 to one, touch) with every WebGPU limit no more than that iPhone's,
/// straight into an eight-rider race on sx01: it must show 1400 pictures and more on WebGPU, past the start, with no
/// validation error in the worker's console. Then the same race with the encoder of the frame's ground meshes stage
/// broken after 120 frames, as WebKit finds one: the error must name that stage and its frame (each stage of a frame
/// is an encoder and an error scope of its own on WebGPU), the shell leave the ride with it, and the page start again
/// on WebGL2 and draw the race there, lit. The module is `tools/web.sh --check`'s. Skips without the disc, node or
/// chromium.
#[test]
fn a_phone_race_on_sx01_rides_on_webgpu_and_a_broken_stage_is_named() {
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
    const FLAGS: [&str; 4] = ["--enable-unsafe-webgpu", "--enable-features=Vulkan", "--use-angle=vulkan", "--ignore-gpu-blocklist"];
    let field = |text: &str, key: &str| -> String {
        text.split(&format!("\"{key}\":")).nth(1).unwrap_or("").split([',', '}']).next().unwrap_or("").trim_matches('"').to_string()
    };
    let run = |name: &str, envs: &[(&str, &str)]| -> (String, String) {
        let scratch = std::env::temp_dir().join(format!("openmx-web-sx01-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&scratch);
        let run = Command::new("node")
            .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/web/gpu.mjs"))
            .arg(repo().join("web"))
            .arg(&iso)
            .arg(scratch.join("profile"))
            .arg(scratch.join("race.png"))
            .arg("")
            .args(FLAGS)
            .env("OPENMX_GPU_PKG", built())
            .env("OPENMX_GPU_PHONE", "844,390,3")
            .env("OPENMX_GPU_LIMITS", "iphone")
            .env("OPENMX_GPU_ARGS", "--ride --track sx01 --race --racers 8")
            .envs(envs.iter().copied())
            .output()
            .expect("run node");
        let _ = std::fs::remove_dir_all(&scratch);
        let (text, log) = (String::from_utf8_lossy(&run.stdout).into_owned(), String::from_utf8_lossy(&run.stderr).into_owned());
        assert!(run.status.success(), "gpu.mjs ({name}) failed:\n{log}{text}");
        (text, log)
    };
    let (text, log) = run("ride", &[("OPENMX_GPU_FRAMES", "1500")]);
    assert!(log.contains("limits: an iPhone's"), "the iPhone's limits were not set:\n{log}");
    assert!(!log.contains("validation error"), "a WebGPU validation error in the race:\n{log}");
    assert_eq!(field(&text, "gpu"), "webgpu", "the race is not on WebGPU:\n{text}");
    assert_eq!(field(&text, "drawn"), "webgpu 1688 780", "no first WebGPU frame at the phone's size:\n{text}");
    let list = text.split("\"fps\":[").nth(1).and_then(|s| s.split(']').next()).unwrap_or("");
    let shown: f64 = list.split(',').filter_map(|f| f.parse::<f64>().ok()).sum();
    assert!(shown >= 1400.0, "only {shown} pictures shown:\n{text}");
    println!("sx01's race on a phone's WebGPU: {shown:.0} pictures, no validation error");

    let (text, log) = run("broken", &[("OPENMX_GPU_SECONDS", "3"), ("OPENMX_GPU_BREAK", "the ground meshes,120"), ("OPENMX_GPU_WANT", "webgl2")]);
    assert!(log.contains("break: the encoder of the ground meshes"), "no encoder was broken:\n{log}");
    assert!(log.contains("gpu: validation error in the ground meshes, frame "), "the broken stage is not named:\n{log}");
    assert!(log.contains("webgpu failed in a ride: gpu: "), "the shell did not leave the ride on the error:\n{log}");
    assert_eq!(field(&text, "gpu"), "webgl2", "the page did not start again on WebGL2:\n{text}");
    assert!(field(&text, "drawn").starts_with("webgl2"), "no WebGL2 frame after the restart:\n{text}");
    let lit: f64 = field(&text, "lit").parse().expect("lit");
    let pixels: f64 = field(&text, "pixels").parse().expect("pixels");
    assert!(lit > pixels / 2.0, "only {lit} of {pixels} pixels of the race on WebGL2 are lit:\n{text}");
    println!("a broken ground meshes encoder: named, and the race started again on {}", field(&text, "drawn"));
}

/// The page log for bug reports (P257b, `tests/web/log.mjs`): a phone has no Web Inspector, so with
/// `?fps` (or the Settings switch) the page keeps every console line, the worker's too, and a Save
/// log button downloads them. Headless chromium picks a file that is no disc: the shell ends on it
/// and the page shows the error on its failed screen; the page is reloaded (as the WebGPU fallback's
/// restart does), and the new page's Save log must download a file holding the module's own line
/// from the worker and the failed screen's error, from before the reload. The module is
/// `tools/web.sh --check`'s. Skips without node or chromium.
#[test]
fn the_page_log_keeps_the_worker_and_the_boot_error() {
    for tool in ["node", "chromium"] {
        if Command::new(tool).arg("--version").output().is_err() {
            eprintln!("skipped: no {tool}");
            return;
        }
    }
    let scratch = std::env::temp_dir().join(format!("openmx-web-log-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&scratch);
    std::fs::create_dir_all(&scratch).expect("scratch dir");
    // a few sectors of nothing: no ISO 9660 volume
    let junk = scratch.join("junk.iso");
    std::fs::write(&junk, vec![0x5a; 64 * 2048]).expect("write the junk disc");
    let run = Command::new("node")
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/web/log.mjs"))
        .arg(repo().join("web"))
        .arg(&junk)
        .arg(scratch.join("profile"))
        .arg(scratch.join("downloads"))
        .env("OPENMX_LOG_PKG", built())
        .output()
        .expect("run node");
    let _ = std::fs::remove_dir_all(&scratch);
    let text = String::from_utf8_lossy(&run.stdout).into_owned();
    assert!(run.status.success(), "log.mjs failed:\n{}{text}", String::from_utf8_lossy(&run.stderr));
    let field = |key: &str| -> String {
        text.split(&format!("\"{key}\":")).nth(1).unwrap_or("").split([',', '}']).next().unwrap_or("").trim_matches('"').to_string()
    };
    assert_eq!(field("state"), "failed", "the page did not fail on a file that is no disc:\n{text}");
    let error = field("status");
    assert!(!error.is_empty(), "the failed screen shows no error:\n{text}");
    assert_eq!(field("button"), "true", "no Save log button after the reload:\n{text}");
    assert_eq!(field("file"), "openmx-log.txt", "Save log downloaded nothing:\n{text}");
    let log = text.split("\"log\":\"").nth(1).unwrap_or("");
    let again = log.find("---- the page loaded again ----").unwrap_or(0);
    assert!(again > 0, "the log does not span the reload:\n{log}");
    assert!(log[..again].contains("worker log: renderer: "), "the worker's console line is not in the log:\n{log}");
    assert!(log[..again].contains(&format!("error: openmx: {error}")), "the failed screen's error is not in the log:\n{log}");
    println!("saved log: {} lines", log.split("\\n").count());
}

/// A pad in the page (P236, `tests/web/gamepad.mjs`): headless chromium with a pad behind
/// `navigator.getGamepads()` shaped as chromium's own — not listed until a button is pressed,
/// `gamepadconnected` then, the standard mapping — not K13's empty stub. The first Cross press on
/// the splash both shows the pad and makes it 1P (the console says so); Cross alone takes it
/// through the menus to a ride with it seated; L2 held keeps the speedometer still and R2 moves
/// it. Not a kernel pad: none can be made on the machine the tests run on (no uinput, and CDP has
/// no gamepad to send). The module is `tools/web.sh --check`'s. Skips without the disc, node or
/// chromium.
#[test]
fn a_pad_the_browser_shows_on_a_press_rides() {
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
    const FLAGS: [&str; 4] = ["--enable-unsafe-webgpu", "--enable-features=Vulkan", "--use-angle=vulkan", "--ignore-gpu-blocklist"];
    let drive = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/web/gamepad.mjs");
    let scratch = std::env::temp_dir().join(format!("openmx-web-gamepad-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&scratch);
    let run = Command::new("node")
        .arg(&drive)
        .arg(repo().join("web"))
        .arg(&iso)
        .arg(scratch.join("profile"))
        .arg(scratch.join("ride.png"))
        .args(FLAGS)
        .env("OPENMX_GPU_PKG", built())
        .output()
        .expect("run node");
    let _ = std::fs::remove_dir_all(&scratch);
    let text = String::from_utf8_lossy(&run.stdout).into_owned();
    let log = String::from_utf8_lossy(&run.stderr);
    assert!(run.status.success(), "gamepad.mjs failed:\n{log}{text}");
    let field = |key: &str| -> String { text.split(&format!("\"{key}\":")).nth(1).unwrap_or("").split([',', '}']).next().unwrap_or("").trim_matches('"').to_string() };
    assert_eq!(field("first"), "true", "the press that showed the pad did not make it 1P:\n{log}{text}");
    assert_eq!(field("seated"), "true", "the pad never took a seat on a ride:\n{log}{text}");
    let braked: f64 = field("braked").parse().expect("braked");
    let gas: f64 = field("gas").parse().expect("gas");
    println!("speedometer changed {braked} on the brake, {gas} on the gas");
    assert!(braked < 0.02, "the speedometer moved with L2 held ({braked}):\n{text}");
    assert!(gas > 0.05, "R2 did not move the speedometer ({gas}):\n{text}");
}

/// A pad through a browser's own Gamepad API (P245, `tests/web/firefox_gamepad.mjs`): headless
/// Firefox (`tools/firefox.sh`'s pinned release) with a pad added by Firefox's gamepad test service,
/// which goes through the browser's gamepad service, listing and events as a USB pad does —
/// where [`a_pad_the_browser_shows_on_a_press_rides`]'s pad is a script shaped as chromium's. The
/// pad is plugged in on the splash and must stay out of `getGamepads()` until it is pressed; that
/// first Cross makes it 1P, Cross alone reaches a ride with it seated, L2 keeps the speedometer
/// still and R2 moves it. Firefox has no WebGPU on Linux, so with a display (the gate's) the page
/// draws on WebGL2, which it must (B13: Firefox's worker refused the canvas's second getContext and
/// the page never rode), and without one in software. Skips without the disc or node, or when
/// Firefox can't be fetched (offline with nothing cached).
#[test]
fn a_firefox_pad_rides() {
    let iso = match Disc::open_configured() {
        Ok(_) => configured_image().expect("the disc opened, so its path resolves"),
        Err(e) => {
            eprintln!("skipped: {e}");
            return;
        }
    };
    if Command::new("node").arg("--version").output().is_err() {
        eprintln!("skipped: no node");
        return;
    }
    let fetch = Command::new("sh").arg(repo().join("tools/firefox.sh")).output().expect("tools/firefox.sh must be runnable");
    if !fetch.status.success() {
        eprintln!("skipped: no Firefox: {}", String::from_utf8_lossy(&fetch.stderr));
        return;
    }
    let firefox = String::from_utf8_lossy(&fetch.stdout).lines().last().unwrap_or("").trim().to_string();
    let drive = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/web/firefox_gamepad.mjs");
    let scratch = std::env::temp_dir().join(format!("openmx-web-firefox-gamepad-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&scratch);
    let run = Command::new("node")
        .arg(&drive)
        .arg(&firefox)
        .arg(repo().join("web"))
        .arg(&iso)
        .arg(scratch.join("profile"))
        .arg(scratch.join("ride.png"))
        .env("OPENMX_GPU_PKG", built())
        .output()
        .expect("run node");
    let _ = std::fs::remove_dir_all(&scratch);
    let text = String::from_utf8_lossy(&run.stdout).into_owned();
    let log = String::from_utf8_lossy(&run.stderr);
    assert!(run.status.success(), "firefox_gamepad.mjs failed:\n{log}{text}");
    let field = |key: &str| -> String { text.split(&format!("\"{key}\":")).nth(1).unwrap_or("").split([',', '}']).next().unwrap_or("").trim_matches('"').to_string() };
    assert_eq!(field("listed"), "false", "Firefox listed the pad before any press:\n{log}{text}");
    assert_eq!(field("first"), "true", "the press that showed the pad did not make it 1P:\n{log}{text}");
    assert_eq!(field("seated"), "true", "the pad never took a seat on a ride:\n{log}{text}");
    let braked: f64 = field("braked").parse().expect("braked");
    let gas: f64 = field("gas").parse().expect("gas");
    println!("speedometer changed {braked} on the brake, {gas} on the gas ({} drawing)", field("gpu"));
    if std::env::var_os("DISPLAY").is_some() || std::env::var_os("WAYLAND_DISPLAY").is_some() {
        assert_eq!(field("gpu"), "webgl2", "Firefox with a display did not draw on WebGL2:\n{log}{text}");
    }
    assert!(braked < 0.02, "the speedometer moved with L2 held ({braked}):\n{text}");
    assert!(gas > 0.05, "R2 did not move the speedometer ({gas}):\n{text}");
}

/// A pad the browser does not map "standard" (P244, `tests/web/pads.mjs`): web/pads.js reads an
/// evdev pad's trigger and hat axes (Firefox on Linux), chromium's hat switch and a cheap pad's hat
/// pair as the standard mapping's L2/R2 and d-pad, its sticks off the axes left, and its buttons
/// from 6 on in their standard places; a standard pad as it is. Skips without node.
#[test]
fn a_pad_without_the_standard_mapping_keeps_its_d_pad_and_triggers() {
    if Command::new("node").arg("--version").output().is_err() {
        eprintln!("skipped: no node");
        return;
    }
    let drive = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/web/pads.mjs");
    let run = Command::new("node").arg(&drive).arg(repo().join("web")).output().expect("run node");
    let text = String::from_utf8_lossy(&run.stdout);
    print!("{text}");
    assert!(run.status.success(), "pads.mjs failed:\n{}{text}", String::from_utf8_lossy(&run.stderr));
}

/// The mod store in the page (P198, `tests/web/store.mjs`): headless chromium opens the page with
/// `--screen store` pointed at a fake store on the page's own loopback server, the disc picked; one
/// Space (Cross) on the store's only card downloads its mod.zip through the redirect to the "CDN" into
/// OPFS, checks and unpacks it, and the track is in OPFS `mods/tracks/<id>/` with its store.json,
/// the download gone. Then (P217) the shell lists it, out of OPFS, with its picture (P261), and the page opened again with
/// `--track` on it rides it. The module is `tools/web.sh --check`'s. Skips without the disc, node or
/// chromium.
#[test]
fn the_browser_installs_from_the_store() {
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
    let scratch = std::env::temp_dir().join(format!("openmx-web-store-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&scratch);
    let drive = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/web/store.mjs");
    let run = Command::new("node").arg(&drive).arg(repo().join("web")).arg(built()).arg(&iso).arg(scratch.join("profile")).output().expect("run node");
    let _ = std::fs::remove_dir_all(&scratch);
    let text = String::from_utf8_lossy(&run.stdout).into_owned();
    println!("{}{text}", String::from_utf8_lossy(&run.stderr));
    assert!(run.status.success(), "store.mjs failed");

    let field = |key: &str| -> String { text.split(&format!("\"{key}\":")).nth(1).unwrap_or("").split([',', '}']).next().unwrap_or("").trim_matches('"').to_string() };
    let list = |key: &str| -> String { text.split(&format!("\"{key}\":[")).nth(1).unwrap_or("").split(']').next().unwrap_or("").to_string() };
    assert_eq!(field("state"), "started", "{text}");
    assert_eq!(
        list("files"),
        r#""tracks/web_store_track/Col/mdwall.col","tracks/web_store_track/Env/MDFLAT/MDFLAT.ps2.trn","tracks/web_store_track/Env/MDFLAT/MDFLAT.scn","tracks/web_store_track/filler.dat","tracks/web_store_track/mod.json","tracks/web_store_track/previews/0.png","tracks/web_store_track/store.json""#,
        "the mod's files, and its store.json, in OPFS mods/: {text}"
    );
    assert_eq!(field("stamp"), field("sha"), "store.json names the zip it came from: {text}");
    assert_eq!(list("left"), "", "the download is gone from OPFS store/: {text}");
    // one GET at the store and one at the CDN it sends to, neither asking a Range (B242)
    assert_eq!((field("zips").as_str(), field("ranges").as_str()), ("2", "0"), "{text}");
    // P217: the install read OPFS mods/ again, so Free Ride's list has the track; and the page rides it
    assert!(field("listed").starts_with("mods: 1 tracks"), "the shell's track list did not see the install: {text}");
    // P261: and its card has the zip's own picture, read out of OPFS (no still: the page has no binary to take one)
    assert_eq!(field("pictured"), "mods: pictures of web_store_track", "the track's card has no picture: {text}");
    assert!(field("rode").starts_with("mods: web_store_track (Web Store Track) from"), "the ride did not load the installed track: {text}");
    assert_eq!(field("ride"), "started", "the ride on the installed track failed: {text}");
    let frames: f64 = field("frames").parse().unwrap_or(0.0);
    assert!(frames > 0.0, "the ride on the installed track drew nothing: {text}");
}

/// The in-game update notice (P193, `tests/web/update.mjs`): with a ride on the canvas a changed
/// `version.txt` puts up neither the page's Reload banner nor anything of the game's; on the next
/// visit, with another build stored as the last one started, the game's main menu offers the newer
/// build (and the splash before it, F14) and says "Updated to …", the page then stores this build as the last one, and Square
/// reloads the page into the newer build. On the
/// software renderer, the disc read in place. Skips without the disc, node or chromium.
#[test]
fn a_new_build_shows_on_the_menu_not_the_ride() {
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
    let scratch = std::env::temp_dir().join(format!("openmx-web-update-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&scratch);
    let drive = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/web/update.mjs");
    let run = Command::new("node")
        .arg(&drive)
        .arg(repo().join("web"))
        .arg(&iso)
        .arg(scratch.join("profile"))
        .env("OPENMX_GPU_PKG", built())
        .output()
        .expect("run node");
    let _ = std::fs::remove_dir_all(&scratch);
    let text = String::from_utf8_lossy(&run.stdout).into_owned();
    assert!(run.status.success(), "update.mjs failed:\n{}{text}", String::from_utf8_lossy(&run.stderr));
    let part = |key: &str| -> String { text.split(&format!("\"{key}\":{{")).nth(1).unwrap_or("").split('}').next().unwrap_or("").to_string() };
    let (ride, menu) = (part("ride"), part("menu"));
    assert!(ride.contains("\"state\":\"started\""), "the ride did not stay on: {text}");
    assert!(ride.contains("\"banner\":false"), "the page's banner went up over the ride: {text}");
    assert!(ride.contains("\"said\":[]"), "the game showed something about builds during the ride: {text}");
    assert!(menu.contains("\"banner\":false"), "the page's banner went up over the game: {text}");
    assert!(menu.contains("the splash offers v0.0.2 2222222"), "the splash did not offer the newer build: {text}");
    assert!(menu.contains("the main menu offers v0.0.2 2222222"), "the main menu did not offer the newer build: {text}");
    assert!(menu.contains(&format!("the main menu says Updated to {}", concat!("v", env!("OPENMX_VERSION"), " · ", env!("OPENMX_COMMIT")))), "no \"Updated to\" on a new build's first start: {text}");
    assert!(menu.contains(&format!("\"stored\":\"{}\"", env!("OPENMX_COMMIT"))), "the page did not keep this build as the last started: {text}");
    assert!(menu.contains("\"reloaded\":\"Build v0.0.2 2222222.\""), "Square on the main menu did not reload into the newer build: {text}");
}

/// The mod store in the page against huggingface.co itself (P218, `tests/web/store_live.mjs`): no
/// fake, the store is FIZL0/openmx-mods. Cross installs the listing's first card into OPFS through the
/// worker's cross-origin fetches (API, resolve, CDN redirect), Triangle opens Report's
/// discussions/new in a new tab, Square removes the mod. Ignored: it needs the network and a mod on
/// the store — `cargo test -p openmx --test web the_browser_uses_the_live_store -- --ignored`.
#[test]
#[ignore]
fn the_browser_uses_the_live_store() {
    let iso = match Disc::open_configured() {
        Ok(_) => configured_image().expect("the disc opened, so its path resolves"),
        Err(e) => {
            eprintln!("skipped: {e}");
            return;
        }
    };
    let scratch = std::env::temp_dir().join(format!("openmx-web-store-live-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&scratch);
    let drive = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/web/store_live.mjs");
    let run = Command::new("node").arg(&drive).arg(repo().join("web")).arg(built()).arg(&iso).arg(scratch.join("profile")).output().expect("run node");
    let _ = std::fs::remove_dir_all(&scratch);
    let text = String::from_utf8_lossy(&run.stdout).into_owned();
    assert!(run.status.success(), "store_live.mjs failed:\n{}", String::from_utf8_lossy(&run.stderr));
    let field = |key: &str| -> String { text.split(&format!("\"{key}\":")).nth(1).unwrap_or("").split([',', '}']).next().unwrap_or("").trim_matches('"').to_string() };
    let list = |key: &str| -> String { text.split(&format!("\"{key}\":[")).nth(1).unwrap_or("").split(']').next().unwrap_or("").to_string() };
    assert_eq!(field("state"), "started", "{text}");
    assert!(list("files").contains("/store.json\""), "nothing installed into OPFS mods/: {text}");
    assert_eq!(field("stamp").len(), 64, "{text}");
    assert_eq!(list("left"), "", "the download is still in OPFS store/: {text}");
    // signed out, Hugging Face sends Report's tab to its sign-in, the prefilled page its next=
    assert!(field("report").starts_with("https://huggingface.co/") && field("report").contains("discussions%2Fnew%3Ftitle%3DReport"), "Report opened no prefilled tab: {text}");
    assert_eq!(list("removed"), "", "Remove left files in OPFS mods/: {text}");
    // the only failed requests the store's own 404s: no index.json yet, no mods/ on main
    for f in list("failed").split("\",\"").filter(|f| !f.is_empty()) {
        assert!(f.contains("index.json") || f.contains("/tree/main/mods"), "a store request failed: {f}\n{text}");
    }
}
