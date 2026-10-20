//! N4 — the shell in a browser: a wasm32 build in `./kit check`, and that build drawing what the
//! desktop draws from the same disc.
//!
//! * [`the_shell_builds_for_the_browser`] builds `crates/openmx/src/web.rs` for
//!   `wasm32-unknown-unknown` through `tools/web.sh` — the same script a release uses — and reads the
//!   module's import section: everything it asks the page for must be one of the four host calls
//!   `web/worker.js` supplies. An import from anywhere else (`env`, `__wbindgen_*`, a WASI function)
//!   is a module the page cannot instantiate, and this is the only test that would see it.
//! * [`the_browser_draws_what_the_desktop_draws`] runs that module in node with `tests/web/host.mjs`
//!   standing in for the worker — the same imports, a file in place of the OPFS handle — over the
//!   user's own disc, and compares its frame with [`openmx_web::start_line`] drawn natively. Rendering
//!   is "looks right" in this project (AGENT.md), so the comparison has a tolerance, and what it
//!   measured is printed.
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

/// The four host calls `web/worker.js` supplies, under the import module the module names.
const HOST_CALLS: [&str; 4] = ["disc_size", "disc_read", "present", "log"];

/// What the page calls.
const EXPORTS: [&str; 4] = ["memory", "openmx_start", "openmx_frame", "openmx_message"];

/// The frame both sides draw: the PS2's own.
const SIZE: (usize, usize) = (640, 448);

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// `tools/web.sh --check`, once per test binary: the path of the module it built.
fn built() -> &'static PathBuf {
    static WASM: OnceLock<PathBuf> = OnceLock::new();
    WASM.get_or_init(|| {
        let script = repo().join("tools/web.sh");
        let out = Command::new("sh").arg(&script).arg("--check").output().expect("tools/web.sh must be runnable");
        let text = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
        assert!(out.status.success(), "{} --check failed:\n{text}", script.display());
        let path = PathBuf::from(String::from_utf8_lossy(&out.stdout).lines().last().unwrap_or("").trim());
        assert!(path.is_file(), "tools/web.sh --check named no module:\n{text}");
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
    let wasm = std::fs::read(built()).expect("read the module");
    let (imports, exports) = interface(&wasm);
    println!("{} bytes; imports {imports:?}; exports {exports:?}", wasm.len());
    for (module, field, _) in &imports {
        assert!(
            module == "openmx" && HOST_CALLS.contains(&field.as_str()),
            "the browser build imports {module}.{field}, which web/worker.js does not supply"
        );
    }
    for call in HOST_CALLS {
        assert!(imports.iter().any(|(_, f, _)| f == call), "{call} is no longer imported: is worker.js stale?");
    }
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
