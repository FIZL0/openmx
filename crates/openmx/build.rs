// The build stamp (P189, opentennis's crates/hst/build.rs, M22): which commit this is, for the online handshake's
// build id (`handshake::set_build`, so two builds refuse each other by commit) and the main menu's corner.
//
// OPENMX_COMMIT in the environment wins, in any profile: tools/build.sh, tools/release.sh and pages.yml set it (a
// worktree's or a container's checkout may have no usable git). Else a release build asks git for HEAD's first 7
// characters, + when the tree is dirty; a dev or test build says "dev", so a commit never rebuilds the tests (P59 in
// opentennis: watching .git/logs/HEAD rebuilt the test binaries on every commit in the shared checkout).
// OPENMX_VERSION (a release's tag, tools/release.sh) is passed through for the corner, the crate's version without it.
//
// Emits OPENMX_COMMIT, OPENMX_VERSION and OPENMX_BUILD_ID (and OPENMX_TAG for a release, P191), the id: "<crate version> <commit>, 30 Hz" — an unstamped
// dev build's is then handshake.rs's own default, so dev builds of one version still meet each other.
// `crates/openmx/tests/build_stamp.rs` mounts this file for its functions.

/// The commit stamp: `env` (OPENMX_COMMIT) when set, else in a `release` build `head` (git's full HEAD hash) cut to
/// 7 characters with + when `dirty`, "unknown" with no git; "dev" otherwise.
pub fn commit(env: Option<&str>, release: bool, head: Option<&str>, dirty: bool) -> String {
    if let Some(c) = env.map(str::trim).filter(|c| !c.is_empty()) {
        return c.to_string();
    }
    if !release {
        return "dev".into();
    }
    // always 7 characters, like ${GITHUB_SHA::7} in the Actions jobs (`--short` grows on an ambiguous prefix)
    let c = head.and_then(|h| h.get(..7)).unwrap_or("unknown");
    format!("{c}{}", if dirty { "+" } else { "" })
}

/// The online build id of a build of crate version `version` stamped `commit`.
pub fn id(version: &str, commit: &str) -> String {
    format!("{version} {commit}, 30 Hz")
}

/// The version the corner shows, without its v: OPENMX_VERSION's (`v0.1.0` or `0.1.0`) when set, else the crate's.
pub fn version(env: Option<&str>, crate_version: &str) -> String {
    env.map(str::trim).filter(|v| !v.is_empty()).map_or(crate_version, |v| v.strip_prefix('v').unwrap_or(v)).to_string()
}

#[allow(dead_code)]
fn main() {
    println!("cargo:rerun-if-env-changed=OPENMX_COMMIT");
    println!("cargo:rerun-if-env-changed=OPENMX_VERSION");
    let git = |args: &[&str]| {
        std::process::Command::new("git").args(args).output().ok().filter(|o| o.status.success()).map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
    };
    let env = std::env::var("OPENMX_COMMIT").ok();
    let release = std::env::var("PROFILE").as_deref() == Ok("release");
    let stamped = env.as_deref().is_some_and(|c| !c.trim().is_empty());
    let (mut head, mut dirty) = (None, false);
    if release && !stamped {
        for f in ["HEAD", "logs/HEAD", "index"] {
            if let Some(p) = git(&["rev-parse", "--git-path", f]) {
                println!("cargo:rerun-if-changed={p}");
            }
        }
        head = git(&["rev-parse", "HEAD"]);
        dirty = git(&["status", "--porcelain", "--untracked-files=no"]).is_some_and(|s| !s.is_empty());
    }
    let commit = commit(env.as_deref(), release, head.as_deref(), dirty);
    let crate_version = std::env::var("CARGO_PKG_VERSION").unwrap_or_default();
    println!("cargo:rustc-env=OPENMX_COMMIT={commit}");
    println!("cargo:rustc-env=OPENMX_VERSION={}", version(std::env::var("OPENMX_VERSION").ok().as_deref(), &crate_version));
    println!("cargo:rustc-env=OPENMX_BUILD_ID={}", id(&crate_version, &commit));
    // P191: a release's tag only when it is one, for the updater (`src/update.rs`) to ask for newer releases
    if let Some(v) = std::env::var("OPENMX_VERSION").ok().filter(|v| !v.trim().is_empty()) {
        println!("cargo:rustc-env=OPENMX_TAG=v{}", version(Some(&v), &crate_version));
    }
}
