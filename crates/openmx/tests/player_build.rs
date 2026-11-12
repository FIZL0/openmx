//! P262: the builds players download run the native sim (openmx's `native-sim`, F15), and the gate's do not.
//! Player builds come out of tools/build.sh (Linux, the Steam Deck's, Windows), tools/web.sh (the page),
//! tools/ios-ipa.sh (the iPhone app, the release's alone), tools/apk.sh (the Android app, P308), and .github/workflows/pages.yml (the beta, which builds
//! Linux without build.sh); tools/release.sh runs the first three. Plain `cargo build`/`cargo run`, the ./kit check gate,
//! ci.yml and `tools/web.sh --check` (the tests' page) stay exact.

use std::path::{Path, PathBuf};
use std::process::Command;

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read(path: &str) -> String {
    std::fs::read_to_string(repo().join(path)).unwrap_or_else(|e| panic!("{path}: {e}"))
}

/// What `tools/build.sh features [flag]` says the cargo arguments of a player build are.
fn build_features(flag: Option<&str>) -> String {
    let mut cmd = Command::new("bash");
    cmd.arg(repo().join("tools/build.sh")).arg("features");
    if let Some(flag) = flag {
        cmd.arg(flag);
    }
    let out = cmd.output().expect("tools/build.sh must be runnable");
    assert!(out.status.success(), "tools/build.sh features: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// tools/build.sh's default build (linux, windows, all: the release's and the Deck's) is native-sim, and
/// --exact-sim builds without it; both are `ship` (P312: no dev variables in a player's build).
#[test]
fn the_default_player_build_is_native_and_exact_sim_is_not() {
    assert_eq!(build_features(None), "--features ship,native-sim");
    assert_eq!(build_features(Some("--native-sim")), "--features ship,native-sim", "F15's flag still means native");
    assert_eq!(build_features(Some("--exact-sim")), "--features ship");
    // and the builds use what it says
    let build = read("tools/build.sh");
    assert!(build.contains("-e FEATURES=\"$features\"") && build.contains("--bin openmx $FEATURES'"), "the linux build drops the features");
    assert!(build.contains("--target $t $features"), "the windows build drops the features");
}

/// The other player builds: the page, the iPhone app and the beta's Linux job, which builds without
/// tools/build.sh. The beta's Windows job and the release go through build.sh with no --exact-sim.
#[test]
fn the_page_the_app_and_the_beta_are_native() {
    let web = read("tools/web.sh");
    assert!(web.contains("features=\"--features ship,native-sim\""), "tools/web.sh's release page is not native, or not ship");
    assert!(web.contains("--check) check=1 features= ;;"), "tools/web.sh --check (the tests' page) must stay exact");
    assert!(web.contains("--target \"$target\" $features"), "tools/web.sh drops the features");

    let ipa = read("tools/ios-ipa.sh");
    assert!(ipa.contains("features=\"--features ship,native-sim\""), "the iPhone app is not native, or not ship");
    assert!(ipa.contains("--exact-sim) features=\"--features ship\" ;;"), "the iPhone app's exact build is not ship");
    assert!(ipa.contains("--bin openmx $features"), "tools/ios-ipa.sh drops the features");

    // P308: the Android app, native and with `ship` (B492: no dev variables on a phone); build.sh android passes
    // --exact-sim on, and nothing else (P284: it once passed `--features native-sim`, which apk.sh refuses)
    let apk = read("tools/apk.sh");
    assert!(apk.contains("features=\"--features android,ship,native-sim\""), "the Android app is not native, or not ship");
    assert!(apk.contains("--example openmx $features"), "tools/apk.sh drops the features");
    assert!(read("tools/build.sh").contains("android) tools/apk.sh ${exact:+--exact-sim} ;;"), "build.sh android drops --exact-sim");

    let pages = read(".github/workflows/pages.yml");
    assert!(pages.contains("cargo build --locked --release -p openmx --bin openmx $(bash tools/build.sh features)"), "the beta's Linux build is not build.sh's");
    assert!(pages.contains("tools/build.sh windows\n"), "the beta's Windows build is not build.sh's default");
    assert!(pages.contains("tools/web.sh\n"), "the beta's page is not web.sh's default");
    assert!(!pages.contains("--exact-sim"), "a beta build asks for the exact sim");

    let release = read("tools/release.sh");
    assert!(release.contains("tools/build.sh all\n") && release.contains("tools/web.sh\n"), "the release does not take the default builds");
    assert!(release.contains("native sim"), "the release notes do not say which sim");
    assert!(release.contains("tools/build.sh android\n") && release.contains("openmx-android.apk"), "the release has no Android app");
    assert!(!release.contains("--exact-sim\n") && !release.contains("ios-ipa.sh \"$out/openmx-ios.ipa\" --exact-sim"));
    // P312: and refuses a player build script that drops `ship` (opentennis's release.sh grep)
    assert!(release.contains("for f in tools/build.sh tools/apk.sh tools/ios-ipa.sh tools/web.sh; do"), "the release does not check the builds are ship");
}

/// The gate's builds are exact: native-sim is no default feature of openmx (nor ps2-float's native of
/// anything's), ci.yml asks for neither, and gate.toml's only native build is F18's tolerance variant.
/// This test binary is one of the gate's, so it is exact too.
#[test]
fn the_gates_build_is_exact() {
    assert!(!ps2_float::NATIVE, "this test build runs the native sim: the gate must build without native-sim");
    for path in ["crates/openmx/Cargo.toml", "crates/ps2-float/Cargo.toml", "crates/game-sim/Cargo.toml", "Cargo.toml"] {
        for line in read(path).lines().map(str::trim) {
            if line.starts_with("default") && line.contains('=') {
                assert!(!line.contains("native"), "{path}: '{line}' makes the native sim a default");
            }
        }
    }
    let ci = read(".github/workflows/ci.yml");
    assert!(!ci.contains("native"), "ci.yml builds the native sim");
    let gate = read("gate.toml");
    let native = gate.lines().filter(|l| !l.trim_start().starts_with('#') && l.contains("native")).collect::<Vec<_>>();
    assert_eq!(native, ["features = \"ps2-float/native\"", "target_dir = \"target/native\"", "filter = \"native_tolerance\""], "gate.toml builds native beyond F18's variant");
}
