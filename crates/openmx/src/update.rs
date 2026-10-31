//! The desktop updater (P191, opentennis's update.rs: B317, B374, M140). A newer build out shows on
//! the main menu under the corner (`menu.rs`'s [`crate::menu::Updater`], which this fills at start),
//! with Square to Update.
//!
//! A beta build (pages.yml builds the rolling `beta` release in GitHub Actions, whose `GITHUB_SHA` it
//! is built with) asks GitHub which commit the `beta` release is at; a release build
//! (`tools/release.sh` builds it with `OPENMX_VERSION=v1.2.3`, which `build.rs` passes on as
//! `OPENMX_TAG`) asks for the latest `v*` release instead. Both ask at start (the first time the main
//! menu is up) and every half hour by the wall clock, unless Settings → Updates is Never or
//! `--no-update-check` was given; a local build never asks. No network, a rate limit or bad JSON say
//! nothing. `OPENMX_LATEST=<build>` fakes a newer one anywhere (`menu.rs`'s `newer_build`);
//! `OPENMX_UPDATE_API=<url>` asks another releases/latest JSON (a local test server).
//!
//! A newer release (semver order, `-rc.1` before its release) offers Update: the platform's bare
//! binary (`openmx-linux-x86_64`) downloads beside the game as `openmx.new`, is checked against the
//! release's `SHA256SUMS`, signed with minisign by the key in [`PUBLIC_KEY`] (`tools/release.sh`'s,
//! plan/HUMAN.md), takes the game file's place (Linux renames it over the running one; Windows, which
//! can't, moves the old aside as `.old`, removed at the next start) and starts with the same
//! arguments. A game whose folder it can't write (installed system-wide, a Flatpak, Steam's library),
//! a release without this platform's files, and the beta (which has no signed sums) get the release
//! page instead. Updates on Always installs what it finds from the main menu by itself.

use crate::menu::Updates;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering::Relaxed};

/// The beta's release page, which Download opens when there is no release to install.
pub const RELEASES: &str = "https://github.com/FIZL0/openmx/releases/tag/beta";

/// The releases/latest JSON a release build asks, unless `OPENMX_UPDATE_API` names another.
const LATEST_API: &str = "https://api.github.com/repos/FIZL0/openmx/releases/latest";

/// The beta release's JSON, whose `target_commitish` is the commit it was built from.
const BETA_API: &str = "https://api.github.com/repos/FIZL0/openmx/releases/tags/beta";

/// This build's release tag (`v0.1.0`, `v1.1.0-rc.1`): only builds made by `tools/release.sh` have
/// one, and only they check for and install newer releases. A debug build never does.
const VERSION: Option<&str> = if cfg!(debug_assertions) { None } else { option_env!("OPENMX_TAG") };

/// The full commit a GitHub Actions build (the beta's) was made from.
const BETA_COMMIT: Option<&str> = if cfg!(debug_assertions) { None } else { option_env!("GITHUB_SHA") };

/// The minisign key `tools/release.sh` signs each release's `SHA256SUMS` with (its secret half stays
/// with the maintainer, `~/.config/openmx/release.key`; release.sh reads this line to refuse a release
/// the game would turn down).
pub const PUBLIC_KEY: &str = "RWR5k+YX2ExPu+K20wH/sYFDvspA24up2ppnqRox43r5olE12STps2Xe";

/// `--no-update-check`: no asking this run, whatever Settings says.
pub static NO_CHECK: AtomicBool = AtomicBool::new(false);

/// This platform's bare binary on a release.
const ASSET: Option<&str> = if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
    Some("openmx-linux-x86_64")
} else if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
    Some("openmx-windows-x86_64.exe")
} else {
    None
};

/// The menu's hooks into this file.
pub fn updater() -> crate::menu::Updater {
    crate::menu::Updater { latest, act, progress, installs }
}

/// What the checks found and what an install is doing.
#[derive(Default)]
struct State {
    /// The beta's newer commit (short).
    commit: Option<String>,
    /// A newer release.
    found: Option<Found>,
    /// The install's words, while it runs or after it failed.
    job: Option<String>,
    busy: bool,
}

static STATE: Mutex<State> = Mutex::new(State { commit: None, found: None, job: None, busy: false });

/// A newer release: its tag, page and the files an install needs.
#[derive(Clone, Debug, Default, PartialEq)]
struct Found {
    tag: String,
    page: String,
    /// This platform's binary, `SHA256SUMS` and `SHA256SUMS.sig` (all three, or no install).
    files: Option<[String; 3]>,
}

/// The newer release's tag or the beta's newer commit, if one is out; the first call starts the
/// checks (none under Never or `--no-update-check`, none in a local build).
fn latest(mode: Updates) -> Option<String> {
    static START: std::sync::Once = std::sync::Once::new();
    if mode == Updates::Never || NO_CHECK.load(Relaxed) {
        return None;
    }
    START.call_once(|| {
        // a release build asks for releases; else its full commit, only in an Actions build
        let ours = match (VERSION, BETA_COMMIT) {
            (Some(v), _) => Ok(v),
            (None, Some(sha)) => Err(sha),
            (None, None) => return,
        };
        std::thread::spawn(move || {
            let Ok(rt) = tokio::runtime::Builder::new_current_thread().enable_all().build() else { return };
            let api = std::env::var("OPENMX_UPDATE_API").unwrap_or_else(|_| LATEST_API.into());
            loop {
                let asked = std::time::SystemTime::now();
                match ours {
                    Ok(v) => match rt.block_on(check(&api, v, ASSET)) {
                        Ok(f) => {
                            println!("update check: {}", f.as_ref().map_or(format!("{v} is the latest"), |f| format!("{} is out", f.tag)));
                            STATE.lock().unwrap().found = f;
                        }
                        Err(e) => println!("update check: {e}"),
                    },
                    Err(ours) => match rt.block_on(beta()) {
                        Ok(sha) if sha != ours => STATE.lock().unwrap().commit = Some(sha[..7].to_string()),
                        Ok(_) => {}
                        Err(e) => println!("update check: {e}"),
                    },
                }
                // half an hour by the wall clock, not one 30-minute sleep: a suspended machine's sleep
                // stops counting
                while !due(asked, std::time::SystemTime::now()) {
                    std::thread::sleep(std::time::Duration::from_secs(5));
                }
            }
        });
    });
    let s = STATE.lock().unwrap();
    s.found.as_ref().map(|f| f.tag.clone()).or_else(|| s.commit.clone())
}

/// Whether the next check is due: half an hour after the last one `asked`, by the wall clock (a clock
/// set back to before it counts as due).
fn due(asked: std::time::SystemTime, now: std::time::SystemTime) -> bool {
    now.duration_since(asked).map_or(true, |d| d >= std::time::Duration::from_secs(30 * 60))
}

/// The commit the `beta` release was built from (`gh release create --target $GITHUB_SHA`: the full one).
async fn beta() -> Result<String, String> {
    let j = json(BETA_API).await?;
    j["target_commitish"].as_str().filter(|s| s.len() == 40 && s.bytes().all(|b| b.is_ascii_hexdigit())).map(str::to_string).ok_or_else(|| "no commit".into())
}

/// `url`'s JSON.
async fn json(url: &str) -> Result<serde_json::Value, String> {
    serde_json::from_slice(&fetch(url, 1 << 20).await?).map_err(|e| e.to_string())
}

/// `url`'s body, at most `cap` bytes.
async fn fetch(url: &str, cap: u64) -> Result<Vec<u8>, String> {
    let r = get(url).await?;
    if r.content_length().is_some_and(|n| n > cap) {
        return Err(format!("{url}: too big"));
    }
    let b = r.bytes().await.map_err(|e| e.to_string())?;
    if b.len() as u64 > cap {
        return Err(format!("{url}: too big"));
    }
    Ok(b.to_vec())
}

/// GET `url` (GitHub's API asks for a User-Agent): its answer, when it is a success.
async fn get(url: &str) -> Result<reqwest::Response, String> {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(20))
        .read_timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|e| e.to_string())?;
    let r = client
        .get(url)
        .header("User-Agent", "openmx")
        .header("Accept", "application/vnd.github+json, application/octet-stream")
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !r.status().is_success() {
        return Err(format!("{url}: {}", r.status()));
    }
    Ok(r)
}

/// The latest release at `api` (GitHub's releases/latest JSON), if it's a `v*` newer than `ours`;
/// `asset` is this platform's binary.
async fn check(api: &str, ours: &str, asset: Option<&str>) -> Result<Option<Found>, String> {
    Ok(found(&json(api).await?, ours, asset))
}

/// A releases/latest answer `j` as an offer: a `v*` tag newer than `ours`, with its files when it
/// has them all.
fn found(j: &serde_json::Value, ours: &str, asset: Option<&str>) -> Option<Found> {
    let tag = j["tag_name"].as_str()?;
    if !tag.starts_with('v') || j["draft"].as_bool() == Some(true) || !newer(tag, ours) {
        return None;
    }
    let url = |name: &str| {
        j["assets"].as_array()?.iter().find(|a| a["name"].as_str() == Some(name))?["browser_download_url"].as_str().map(str::to_string)
    };
    let files = asset.and_then(|a| Some([url(a)?, url("SHA256SUMS")?, url("SHA256SUMS.sig")?]));
    Some(Found { tag: tag.into(), page: j["html_url"].as_str().unwrap_or(RELEASES).into(), files })
}

/// Whether version `a` comes after `b` (`v1.2.3`, `v0.1.0-rc.10`; semver order: a pre-release before
/// its release, numeric parts by number). Unreadable ones are never newer.
pub fn newer(a: &str, b: &str) -> bool {
    fn parse(s: &str) -> Option<([u64; 3], Option<&str>)> {
        let s = s.trim().trim_start_matches('v').split('+').next()?;
        let (core, pre) = s.split_once('-').map_or((s, None), |(c, p)| (c, Some(p)));
        let mut n = core.split('.').map(|x| x.parse().ok());
        let v = [n.next()??, n.next()??, n.next()??];
        n.next().is_none().then_some((v, pre))
    }
    let (Some((va, pa)), Some((vb, pb))) = (parse(a), parse(b)) else { return false };
    if va != vb {
        return va > vb;
    }
    match (pa, pb) {
        (None, Some(_)) => true,
        (_, None) => false,
        (Some(pa), Some(pb)) => {
            use std::cmp::Ordering::*;
            for (x, y) in pa.split('.').zip(pb.split('.')) {
                let o = match (x.parse::<u64>(), y.parse::<u64>()) {
                    (Ok(x), Ok(y)) => x.cmp(&y),
                    (Ok(_), Err(_)) => Less,
                    (Err(_), Ok(_)) => Greater,
                    _ => x.cmp(y),
                };
                if o != Equal {
                    return o == Greater;
                }
            }
            pa.split('.').count() > pb.split('.').count()
        }
    }
}

/// The game's own file, as it was at start (Linux's `/proc/self/exe` says "(deleted)" once it has
/// been swapped).
fn exe() -> Option<PathBuf> {
    static EXE: std::sync::OnceLock<Option<PathBuf>> = std::sync::OnceLock::new();
    EXE.get_or_init(|| std::env::current_exe().ok().and_then(|p| p.canonicalize().ok())).clone()
}

/// At start: notes the game's path and removes the `.old` an update on Windows leaves behind.
pub fn cleanup() {
    if let Some(e) = exe() {
        let _ = std::fs::remove_file(old(&e));
    }
}

fn old(exe: &Path) -> PathBuf {
    let mut s = exe.as_os_str().to_owned();
    s.push(".old");
    s.into()
}

/// Whether the game's folder takes a new file.
fn writable() -> bool {
    static OK: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *OK.get_or_init(|| {
        let Some(dir) = exe().and_then(|e| e.parent().map(Path::to_path_buf)) else { return false };
        let probe = dir.join(".openmx-update-probe");
        let ok = std::fs::write(&probe, b"").is_ok();
        let _ = std::fs::remove_file(&probe);
        ok
    })
}

/// Whether `exe` is in a Steam library (`steamapps/`), whose files Steam owns and checks.
fn steams(exe: &Path) -> bool {
    exe.components().any(|c| c.as_os_str().eq_ignore_ascii_case("steamapps"))
}

/// The game file a found release installs over, or none: its page instead.
fn target(f: &Found) -> Option<PathBuf> {
    f.files.as_ref()?;
    let exe = exe()?;
    (std::env::var_os("FLATPAK_ID").is_none() && !steams(&exe) && writable()).then_some(exe)
}

/// Square on the notice: a newer release installs and the game restarts into it; else the release
/// page (or the beta's) opens.
fn act() {
    let mut s = STATE.lock().unwrap();
    if s.busy {
        return;
    }
    let Some(f) = s.found.clone() else {
        drop(s);
        return page(RELEASES);
    };
    let (Some(exe), Some(files)) = (target(&f), f.files.clone()) else {
        drop(s);
        return page(&f.page);
    };
    (s.busy, s.job) = (true, Some(format!("Downloading {}...", f.tag)));
    drop(s);
    std::thread::spawn(move || {
        let job = |says: String| STATE.lock().unwrap().job = Some(says);
        let shown = |got: u64, total: u64| job(format!("Downloading {}... {} / {} MB", f.tag, got >> 20, total.max(got) >> 20));
        let done = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
            Ok(rt) => rt.block_on(install(&files, &exe, PUBLIC_KEY, &shown)),
            Err(e) => Err(e.to_string()),
        };
        match done {
            Ok(()) => {
                job(format!("Starting {}...", f.tag));
                // the same arguments (the disc), from the new file at the old one's path
                match std::process::Command::new(&exe).args(std::env::args_os().skip(1)).spawn() {
                    Ok(_) => std::process::exit(0),
                    Err(e) => job(format!("Updated to {}: start the game again ({e}).", f.tag)),
                }
            }
            Err(e) => {
                println!("update: {e}");
                job(format!("Update failed: {e}"));
            }
        }
        STATE.lock().unwrap().busy = false;
    });
}

fn page(url: &str) {
    if let Err(e) = crate::modstore::open(url) {
        println!("update: {e}");
    }
}

/// The install's words while it runs, or why it failed.
fn progress() -> Option<String> {
    STATE.lock().unwrap().job.clone()
}

/// Whether Square installs in place (Update), else it opens the release page (Download).
fn installs() -> bool {
    STATE.lock().unwrap().found.as_ref().is_some_and(|f| target(f).is_some())
}

/// `files` (this platform's binary, `SHA256SUMS`, `SHA256SUMS.sig`) installed over `exe`: the binary
/// downloads to `openmx.new` beside it ([`verified`]), then takes `exe`'s place. Any failure leaves
/// `exe` as it was and no `openmx.new`.
async fn install(files: &[String; 3], exe: &Path, key: &str, shown: &dyn Fn(u64, u64)) -> Result<(), String> {
    let new = exe.with_file_name("openmx.new");
    verified(files, &new, key, shown).await?;
    let ok = swap(&new, exe, cfg!(windows));
    if ok.is_err() {
        let _ = std::fs::remove_file(&new);
    }
    ok
}

/// `files[0]` downloaded to `to`, once the sums' signature is checked against `key` and the
/// download's sha256 against them; any failure leaves no `to`.
async fn verified(files: &[String; 3], to: &Path, key: &str, shown: &dyn Fn(u64, u64)) -> Result<(), String> {
    let name = files[0].rsplit('/').next().unwrap_or_default().to_string();
    let sums = fetch(&files[1], 1 << 16).await?;
    let sig = fetch(&files[2], 1 << 12).await?;
    let want = signed_sum(&sums, &sig, key, &name)?;
    let got = download(&files[0], to, shown).await;
    let ok = got.and_then(|h| if h == want { Ok(()) } else { Err(format!("{name}'s checksum doesn't match the signed one")) });
    if ok.is_err() {
        let _ = std::fs::remove_file(to);
    }
    ok
}

/// `name`'s sha256 (hex) from `sums` (`sha256sum`'s lines), once `sig` proves `key` signed them.
fn signed_sum(sums: &[u8], sig: &[u8], key: &str, name: &str) -> Result<String, String> {
    let key = minisign_verify::PublicKey::from_base64(key).map_err(|e| e.to_string())?;
    let sig = minisign_verify::Signature::decode(std::str::from_utf8(sig).map_err(|e| e.to_string())?).map_err(|e| format!("bad signature file: {e}"))?;
    key.verify(sums, &sig, false).map_err(|e| format!("the checksums' signature doesn't match ({e})"))?;
    String::from_utf8_lossy(sums)
        .lines()
        .find_map(|l| {
            let (h, n) = l.split_once(char::is_whitespace)?;
            (n.trim().trim_start_matches('*') == name).then(|| h.to_ascii_lowercase())
        })
        .ok_or_else(|| format!("{name} isn't in the checksums"))
}

/// `url` streamed into `to` (at most 1 GB): its sha256 (hex).
async fn download(url: &str, to: &Path, shown: &dyn Fn(u64, u64)) -> Result<String, String> {
    use sha2::Digest;
    use std::io::Write;
    let mut r = get(url).await?;
    let total = r.content_length().unwrap_or(0);
    let mut f = std::fs::File::create(to).map_err(|e| format!("can't write {}: {e}", to.display()))?;
    let (mut got, mut hash) = (0u64, sha2::Sha256::new());
    while let Some(c) = r.chunk().await.map_err(|e| format!("download cut short: {e}"))? {
        got += c.len() as u64;
        if got > 1 << 30 {
            return Err("download too big".into());
        }
        hash.update(&c);
        f.write_all(&c).map_err(|e| format!("can't write {}: {e}", to.display()))?;
        shown(got, total);
    }
    f.sync_all().map_err(|e| e.to_string())?;
    Ok(hash.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

/// `new` in `exe`'s place. Linux: made executable and renamed over it (the running game keeps its
/// open copy). Windows (`windows`), where a running .exe can't be replaced but can be renamed: `exe` →
/// `exe.old`, `new` → `exe` (put back if that fails).
fn swap(new: &Path, exe: &Path, windows: bool) -> Result<(), String> {
    let e = |what: &str, err: std::io::Error| format!("can't {what}: {err}");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(new, std::fs::Permissions::from_mode(0o755)).map_err(|x| e("make it executable", x))?;
    }
    if windows && exe.exists() {
        let old = old(exe);
        let _ = std::fs::remove_file(&old);
        std::fs::rename(exe, &old).map_err(|x| e("move the old game aside", x))?;
        if let Err(x) = std::fs::rename(new, exe) {
            let _ = std::fs::rename(&old, exe);
            return Err(e("put the new game in place", x));
        }
        Ok(())
    } else {
        std::fs::rename(new, exe).map_err(|x| e("put the new game in place", x))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// A throwaway minisign key (made for this test, `minisign -G -W`; its secret is gone) and what it
    /// signed: the sums of a "new build" (`new build\n`), and sums with a wrong hash.
    const KEY: &str = "RWTzwBYSIqAg5Q1JFncp1n8jVLgBx08Gj7Bbg/lPq7bjAONDJkIiriV7";
    const BIN: &[u8] = b"new build\n";
    const SUMS: &str = "288765569ab03c0fbf107b0f870fb74c42fff3d4b295613911a955cf8e967492  openmx-linux-x86_64\n";
    const SIG: &str = "untrusted comment: signature from minisign secret key
RUTzwBYSIqAg5T43arXrp0ywfxz75zVesf98AxHTxl9DOcJxUz0M0HMmsreltKC+WkKohavhN5I8YinmhXgnv9fG8D6FmDA4ago=
trusted comment: test
5j16w9rVTICti/+67UwKh8s6I4ijCKFZzWcep63+hievw5WHe3HiAa8WUoqv5fiUVBRYEEIrpjssLD/2nhduBw==
";
    const BAD_SUMS: &str = "0000000000000000000000000000000000000000000000000000000000000000  openmx-linux-x86_64\n";
    const BAD_SUMS_SIG: &str = "untrusted comment: signature from minisign secret key
RUTzwBYSIqAg5Zf2B4Q9C+h72hZzAz1GmToZXUc77/yAqFlNrihKbPWXYY8aKcooklgS2mlqYe2+jbJsc8AwM9Q7yBP4k7TzFQY=
trusted comment: test
IRD1/dIIR81+kXZO+KtiJjXG9biT1jhLFBvcihZw8cQjZbBXq5xLtwDJwMqeUI+v1tVwoJt17sI8NTAUlOWBAg==
";

    /// The same sums signed with the release key (`tools/release.sh`'s, P191), which [`PUBLIC_KEY`] is.
    const RELEASE_SIG: &str = "untrusted comment: signature from minisign secret key
RUR5k+YX2ExPu/D5XFdIUGyZehp1T39fNFqoHJd424w46O2XPGgyT8sSOlNkWnLZfROkfVhvmXpiwED4aTbFcA0g6n3mJdzdDwI=
trusted comment: openMX test
Hs5KaCqvNkJ1Vjp/bntiFqm0CkKzxeGJhK9L+76A3G/wL+xeLy8au+kcLCnYzhku/rp99xxUM9X1p8bbY19JDg==
";

    /// The key built in is the release key's: sums it signed verify, and the test key's don't.
    #[test]
    fn the_built_in_key_takes_the_release_keys_signature() {
        assert_eq!(signed_sum(SUMS.as_bytes(), RELEASE_SIG.as_bytes(), PUBLIC_KEY, "openmx-linux-x86_64").unwrap(), SUMS[..64]);
        assert!(signed_sum(SUMS.as_bytes(), SIG.as_bytes(), PUBLIC_KEY, "openmx-linux-x86_64").is_err());
    }

    fn rt() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap()
    }

    /// An HTTP server on localhost answering GETs from `files` (path → body), 404 otherwise. Its base URL.
    fn serve(files: HashMap<String, Vec<u8>>) -> String {
        use std::io::{BufRead, BufReader, Write};
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", l.local_addr().unwrap());
        std::thread::spawn(move || {
            for mut c in l.incoming().flatten() {
                let mut line = String::new();
                let mut r = BufReader::new(c.try_clone().unwrap());
                let _ = r.read_line(&mut line);
                loop {
                    let mut h = String::new();
                    if r.read_line(&mut h).map_or(true, |n| n == 0) || h.trim().is_empty() {
                        break;
                    }
                }
                let path = line.split_whitespace().nth(1).unwrap_or("/").to_string();
                let (status, body) = files.get(&path).map_or(("404 Not Found", &b""[..]), |b| ("200 OK", b));
                let _ = write!(c, "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
                let _ = c.write_all(body);
            }
        });
        base
    }

    /// A release `tag` at a local server whose sums and signature are `sums`/`sig`: the API URL.
    fn release(tag: &str, sums: &str, sig: &str) -> String {
        // the JSON names the files' server, so it comes from a second one
        let files: HashMap<String, Vec<u8>> = [("/openmx-linux-x86_64", BIN.to_vec()), ("/SHA256SUMS", sums.as_bytes().to_vec()), ("/SHA256SUMS.sig", sig.as_bytes().to_vec())]
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect();
        let base = serve(files);
        let asset = |n: &str| serde_json::json!({ "name": n, "browser_download_url": format!("{base}/{n}") });
        let j = serde_json::json!({
            "tag_name": tag, "draft": false, "html_url": format!("{base}/page"),
            "assets": [asset("openmx-linux-x86_64"), asset("SHA256SUMS"), asset("SHA256SUMS.sig"), asset("openmx-linux-x86_64.tar.gz")],
        });
        let api = serve([("/latest".to_string(), j.to_string().into_bytes())].into());
        format!("{api}/latest")
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("openmx-update-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// The check → verify → swap path against a local server: a newer release is found and replaces
    /// the game file (on the Windows path leaving the old one as `.old`); the same or an older version
    /// isn't offered.
    #[test]
    fn a_local_servers_release_is_found_and_installed() {
        let rt = rt();
        let dir = scratch("install");
        let quiet = |_: u64, _: u64| {};
        let asset = Some("openmx-linux-x86_64");

        let api = release("v0.2.0-rc.9", SUMS, SIG);
        assert_eq!(rt.block_on(check(&api, "v0.2.0-rc.9", asset)).unwrap(), None);
        assert_eq!(rt.block_on(check(&api, "v0.2.0-rc.10", asset)).unwrap(), None);
        assert_eq!(rt.block_on(check(&api, "v0.2.0", asset)).unwrap(), None);

        let f = rt.block_on(check(&api, "v0.1.0", asset)).unwrap().expect("offered");
        assert_eq!(f.tag, "v0.2.0-rc.9");
        let exe = dir.join("openmx");
        std::fs::write(&exe, b"old build\n").unwrap();
        rt.block_on(install(&f.files.expect("files"), &exe, KEY, &quiet)).unwrap();
        assert_eq!(std::fs::read(&exe).unwrap(), BIN);
        assert!(!dir.join("openmx.new").exists());
        #[cfg(unix)]
        assert_eq!(std::os::unix::fs::PermissionsExt::mode(&std::fs::metadata(&exe).unwrap().permissions()) & 0o111, 0o111);

        // Windows' renames: openmx.exe → openmx.exe.old, openmx.new → openmx.exe
        let win = dir.join("openmx.exe");
        std::fs::write(&win, b"old build\n").unwrap();
        std::fs::write(dir.join("openmx.new"), BIN).unwrap();
        swap(&dir.join("openmx.new"), &win, true).unwrap();
        assert_eq!(std::fs::read(&win).unwrap(), BIN);
        assert_eq!(std::fs::read(old(&win)).unwrap(), b"old build\n");

        // no binary for this platform: offered (the page), nothing to install
        let f = rt.block_on(check(&api, "v0.1.0", Some("openmx-plan9"))).unwrap().unwrap();
        assert!(f.files.is_none() && f.page.ends_with("/page"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A signed SHA256SUMS verifies; a signature by another key (the release key), sums that don't
    /// match their signature, or a signed wrong checksum are refused, and the old game stays.
    #[test]
    fn a_tampered_release_is_refused() {
        assert_eq!(signed_sum(SUMS.as_bytes(), SIG.as_bytes(), KEY, "openmx-linux-x86_64").unwrap(), SUMS[..64]);
        assert!(signed_sum(SUMS.as_bytes(), SIG.as_bytes(), KEY, "openmx-windows-x86_64.exe").unwrap_err().contains("isn't in"));
        let mut flipped = SUMS.as_bytes().to_vec();
        flipped[0] ^= 1;
        assert!(signed_sum(&flipped, SIG.as_bytes(), KEY, "openmx-linux-x86_64").unwrap_err().contains("signature"));
        assert!(signed_sum(SUMS.as_bytes(), b"not a signature", KEY, "openmx-linux-x86_64").is_err());

        let rt = rt();
        let dir = scratch("tampered");
        for (api, key) in [
            (release("v0.2.0", SUMS, SIG), PUBLIC_KEY),
            (release("v0.2.0", BAD_SUMS, SIG), KEY),
            (release("v0.2.0", BAD_SUMS, BAD_SUMS_SIG), KEY),
        ] {
            let f = rt.block_on(check(&api, "v0.1.0", Some("openmx-linux-x86_64"))).unwrap().unwrap();
            let exe = dir.join("openmx");
            std::fs::write(&exe, b"old build\n").unwrap();
            let e = rt.block_on(install(&f.files.unwrap(), &exe, key, &|_, _| {})).unwrap_err();
            assert!(e.contains("signature") || e.contains("checksum"), "{e}");
            assert_eq!(std::fs::read(&exe).unwrap(), b"old build\n");
            assert!(!dir.join("openmx.new").exists());
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// No server, or one answering with something other than a release, is an error or nothing,
    /// which the menu never shows.
    #[test]
    fn no_network_or_bad_json_says_nothing() {
        let rt = rt();
        assert!(rt.block_on(check("http://127.0.0.1:9/latest", "v0.0.1", ASSET)).is_err());
        let api = serve([("/latest".to_string(), b"<html>rate limited</html>".to_vec())].into());
        assert!(rt.block_on(check(&format!("{api}/latest"), "v0.0.1", ASSET)).is_err());
        let api = serve([("/latest".to_string(), br#"{"message":"Not Found"}"#.to_vec())].into());
        assert_eq!(rt.block_on(check(&format!("{api}/latest"), "v0.0.1", ASSET)).unwrap(), None);
        assert!(rt.block_on(check(&format!("{api}/nothing"), "v0.0.1", ASSET)).is_err());
    }

    #[test]
    fn versions_in_semver_order() {
        let order = ["v0.1.0-alpha.1", "v0.1.0-alpha.2", "v0.1.0-alpha.10", "v0.1.0-beta", "v0.1.0-rc.1", "v0.1.0", "v0.1.1", "v0.2.0", "v1.0.0-alpha.1", "v1.0.0", "v1.10.0"];
        for (i, a) in order.iter().enumerate() {
            for (j, b) in order.iter().enumerate() {
                assert_eq!(newer(a, b), i > j, "{a} vs {b}");
            }
        }
        assert!(!newer("beta", "v0.1.0") && !newer("v0.1.0", "beta") && !newer("v1.2", "v1.0.0"));
        assert!(newer("1.0.0", "v0.9.9"));
    }

    /// A check is due half an hour after the last by the wall clock; a clock set back counts as due.
    #[test]
    fn checks_again_half_an_hour_later() {
        let t = std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_800_000_000);
        let min = |m: u64| std::time::Duration::from_secs(60 * m);
        assert!(!due(t, t) && !due(t, t + min(29)));
        assert!(due(t, t + min(30)) && due(t, t + min(7 * 60)));
        assert!(due(t, t - min(1)));
    }

    #[test]
    fn steam_owns_only_its_library() {
        assert!(steams(Path::new("/home/deck/.local/share/Steam/steamapps/common/openMX/openmx")));
        assert!(steams(Path::new("D:/SteamLibrary/SteamApps/common/openMX/openmx.exe")));
        assert!(!steams(Path::new("/home/deck/Games/openmx/openmx")));
    }

    #[test]
    fn a_found_release_needs_a_v_tag_and_all_its_files() {
        let j = serde_json::json!({ "tag_name": "beta", "assets": [] });
        assert_eq!(found(&j, "v0.0.1", ASSET), None);
        let j = serde_json::json!({ "tag_name": "v2.0.0", "draft": true, "assets": [] });
        assert_eq!(found(&j, "v1.0.0", ASSET), None);
        let j = serde_json::json!({ "tag_name": "v2.0.0", "html_url": "p", "assets": [{ "name": "SHA256SUMS", "browser_download_url": "s" }] });
        assert_eq!(found(&j, "v1.0.0", Some("openmx-linux-x86_64")), Some(Found { tag: "v2.0.0".into(), page: "p".into(), files: None }));
    }

    /// P247: a platform with no binary of its own (iOS) still sees a newer release, as its page.
    #[test]
    fn an_iphone_finds_a_newer_release_as_its_page() {
        let all = ["openmx-linux-x86_64", "SHA256SUMS", "SHA256SUMS.sig"].map(|n| serde_json::json!({ "name": n, "browser_download_url": n }));
        let j = serde_json::json!({ "tag_name": "v2.0.0", "html_url": "p", "assets": all });
        let f = found(&j, "v1.0.0", None).unwrap();
        assert_eq!((f.tag.as_str(), f.files), ("v2.0.0", None));
        assert_eq!(target(&found(&j, "v1.0.0", None).unwrap()), None);
    }
}
