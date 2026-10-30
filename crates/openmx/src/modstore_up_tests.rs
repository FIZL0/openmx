//! The upload side's tests (P199): the token read from a paste and kept owner-only, the player's own
//! mods found (a disc-made one among them), a mod folder packed into a zip Install's checks take, the
//! re-upload plan, and a fake Hugging Face on localhost that takes commits (preupload, LFS in one PUT
//! and in parts, the commit's NDJSON, a PR's status): an upload opens a PR another player's Browse
//! lists and installs, a re-upload and a second mod go onto that PR, the same zip is skipped, My
//! public mods lists them, and take-down takes one out of the PR and then closes it.

use super::*;
use std::collections::{BTreeMap, HashMap};
use std::io::{Read as _, Write as _};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const TOKEN_OK: &str = "hf_testtesttesttesttesttesttesttest01";
const TOKEN_READ: &str = "hf_readreadreadreadreadreadreadread01";

fn unbase64(s: &str) -> Vec<u8> {
    let val = |c: u8| match c {
        b'A'..=b'Z' => c - b'A',
        b'a'..=b'z' => c - b'a' + 26,
        b'0'..=b'9' => c - b'0' + 52,
        b'+' => 62,
        _ => 63,
    };
    let mut out = Vec::new();
    for q in s.as_bytes().chunks(4) {
        let n = q.iter().filter(|&&c| c != b'=').count();
        let v = q.iter().take(n).fold(0u32, |a, &c| a << 6 | val(c) as u32) << (6 * (4 - n));
        out.extend(&v.to_be_bytes()[1..n]);
    }
    out
}

#[derive(Default)]
struct Pr {
    author: String,
    open: bool,
    files: BTreeMap<String, Vec<u8>>,
    head: u32,
}

/// The fake Hub's repo: `main`'s files, the PRs, LFS objects (and parts on their way), and every
/// commit as (where, summary).
#[derive(Default)]
struct Hub {
    main: BTreeMap<String, Vec<u8>>,
    prs: BTreeMap<u32, Pr>,
    lfs: HashMap<String, Vec<u8>>,
    parts: HashMap<String, BTreeMap<u32, Vec<u8>>>,
    commits: Vec<(String, String)>,
    multipart: usize,
}

struct Fake {
    base: String,
    hub: Arc<Mutex<Hub>>,
}

fn reply(status: u16, body: &[u8]) -> Vec<u8> {
    let mut out = format!("HTTP/1.1 {status} X\r\nContent-Length: {}\r\nETag: \"e{}\"\r\nConnection: close\r\n\r\n", body.len(), body.len()).into_bytes();
    out.extend(body);
    out
}

fn ok(v: Value) -> (u16, Vec<u8>) {
    (200, v.to_string().into_bytes())
}

impl Fake {
    fn start() -> Fake {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", l.local_addr().unwrap());
        let hub = Arc::new(Mutex::new(Hub::default()));
        let (h, b) = (hub.clone(), base.clone());
        std::thread::spawn(move || {
            for c in l.incoming() {
                let Ok(mut c) = c else { continue };
                let (h, b) = (h.clone(), b.clone());
                std::thread::spawn(move || {
                    let _ = c.set_read_timeout(Some(Duration::from_secs(10)));
                    let mut head = Vec::new();
                    let mut one = [0u8; 1];
                    while !head.ends_with(b"\r\n\r\n") && c.read(&mut one).map_or(false, |n| n == 1) {
                        head.push(one[0]);
                    }
                    let text = String::from_utf8_lossy(&head).to_string();
                    let mut words = text.lines().next().unwrap_or("").split(' ');
                    let (method, path) = (words.next().unwrap_or("").to_string(), words.next().unwrap_or("").to_string());
                    let header = |name: &str| text.lines().find_map(|l| l.split_once(':').filter(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.trim().to_string()));
                    let len: usize = header("content-length").and_then(|v| v.parse().ok()).unwrap_or(0);
                    let mut body = vec![0; len];
                    let _ = c.read_exact(&mut body);
                    let who = match header("authorization").as_deref() {
                        Some(a) if a == format!("Bearer {TOKEN_OK}") => Some("tester"),
                        Some(a) if a == format!("Bearer {TOKEN_READ}") => Some("reader"),
                        _ => None,
                    };
                    let (status, out) = Fake::route(&mut h.lock().unwrap(), &b, &method, &path, &body, who);
                    let _ = c.write_all(&reply(status, &out));
                });
            }
        });
        Fake { base, hub }
    }

    /// A rev's files: `main`, or `refs%2Fpr%2F<n>`.
    fn files<'a>(hub: &'a mut Hub, rev: &str) -> Option<&'a mut BTreeMap<String, Vec<u8>>> {
        match rev.strip_prefix("refs%2Fpr%2F").and_then(|n| n.parse::<u32>().ok()) {
            Some(n) => hub.prs.get_mut(&n).map(|p| &mut p.files),
            None if rev == "main" => Some(&mut hub.main),
            None => None,
        }
    }

    fn route(hub: &mut Hub, base: &str, method: &str, path: &str, body: &[u8], who: Option<&str>) -> (u16, Vec<u8>) {
        let api = format!("/api/datasets/{REPO}");
        let missing = (404, b"{\"error\":\"not found\"}".to_vec());
        let refused = (401, b"{\"error\":\"Invalid credentials in Authorization header\"}".to_vec());
        match method {
            "GET" if path == "/api/whoami-v2" => match who {
                Some(name) => ok(json!({ "name": name, "auth": { "accessToken": { "role": if name == "reader" { "read" } else { "write" } } } })),
                None => refused,
            },
            "GET" if path.starts_with(&format!("{api}/discussions?")) => {
                let list: Vec<Value> = hub.prs.iter().rev().map(|(n, p)| json!({ "num": n, "isPullRequest": true, "status": if p.open { "open" } else { "closed" }, "author": { "name": p.author } })).collect();
                if path.contains("&p=0") { ok(json!({ "discussions": list, "count": list.len() })) } else { ok(json!({ "discussions": [], "count": list.len() })) }
            }
            "GET" if path == format!("{api}/refs?include_prs=1") => ok(json!({ "pullRequests": hub.prs.iter().map(|(n, p)| json!({ "name": n.to_string(), "targetCommit": format!("c{n}-{}", p.head) })).collect::<Vec<_>>() })),
            "GET" if path.starts_with(&format!("{api}/tree/")) => {
                let rev = path[api.len() + 6..].trim_end_matches("/mods");
                let Some(files) = Fake::files(hub, rev) else { return missing };
                let mut ids: Vec<String> = files.keys().filter_map(|k| Some(k.strip_prefix("mods/")?.split_once('/')?.0.to_string())).collect();
                ids.dedup();
                if ids.is_empty() {
                    return missing;
                }
                ok(Value::Array(ids.iter().map(|id| json!({ "type": "directory", "path": format!("mods/{id}") })).collect()))
            }
            "GET" if path.starts_with(&format!("/datasets/{REPO}/resolve/")) => {
                let rest = &path[format!("/datasets/{REPO}/resolve/").len()..];
                let Some((rev, file)) = rest.split_once('/') else { return missing };
                Fake::files(hub, rev).and_then(|f| f.get(file).cloned()).map_or(missing, |b| (200, b))
            }
            _ if who.is_none() && !path.starts_with("/lfs-") => refused,
            "POST" if path.starts_with(&format!("{api}/preupload/")) => {
                let ask: Value = serde_json::from_slice(body).unwrap();
                let files: Vec<Value> = ask["files"].as_array().unwrap().iter().map(|f| {
                    let p = f["path"].as_str().unwrap();
                    json!({ "path": p, "uploadMode": if p.ends_with(".zip") || p.ends_with(".webp") { "lfs" } else { "regular" } })
                }).collect();
                ok(json!({ "files": files }))
            }
            "POST" if path == format!("/datasets/{REPO}.git/info/lfs/objects/batch") => {
                let ask: Value = serde_json::from_slice(body).unwrap();
                let (oid, size) = (ask["objects"][0]["oid"].as_str().unwrap().to_string(), ask["objects"][0]["size"].as_u64().unwrap());
                if hub.lfs.contains_key(&oid) {
                    return ok(json!({ "objects": [{ "oid": oid, "size": size }] }));
                }
                let verify = json!({ "href": format!("{base}/lfs-verify") });
                // a big object in parts of 256 bytes, as S3 multipart; a small one in one PUT
                if size > 600 {
                    let mut header = serde_json::Map::new();
                    header.insert("chunk_size".into(), json!("256"));
                    for i in 0..size.div_ceil(256) {
                        header.insert((i + 1).to_string(), json!(format!("{base}/lfs-part/{oid}/{}", i + 1)));
                    }
                    ok(json!({ "objects": [{ "oid": oid, "size": size, "actions": { "upload": { "href": format!("{base}/lfs-done/{oid}"), "header": header }, "verify": verify } }] }))
                } else {
                    ok(json!({ "objects": [{ "oid": oid, "size": size, "actions": { "upload": { "href": format!("{base}/lfs-put/{oid}") }, "verify": verify } }] }))
                }
            }
            "PUT" if path.starts_with("/lfs-put/") => {
                hub.lfs.insert(path["/lfs-put/".len()..].to_string(), body.to_vec());
                (200, Vec::new())
            }
            "PUT" if path.starts_with("/lfs-part/") => {
                let (oid, n) = path["/lfs-part/".len()..].split_once('/').unwrap();
                hub.parts.entry(oid.to_string()).or_default().insert(n.parse().unwrap(), body.to_vec());
                (200, Vec::new())
            }
            "POST" if path.starts_with("/lfs-done/") => {
                let oid = path["/lfs-done/".len()..].to_string();
                let done: Value = serde_json::from_slice(body).unwrap();
                assert_eq!(done["parts"].as_array().unwrap().len(), hub.parts[&oid].len(), "every part's ETag back");
                let whole: Vec<u8> = hub.parts.remove(&oid).unwrap().into_values().flatten().collect();
                if hex(&Sha256::digest(&whole)) != oid {
                    return (400, b"{\"error\":\"parts don't make the object\"}".to_vec());
                }
                hub.multipart += 1;
                hub.lfs.insert(oid, whole);
                ok(json!({}))
            }
            "POST" if path == "/lfs-verify" => {
                let v: Value = serde_json::from_slice(body).unwrap();
                if hub.lfs.get(v["oid"].as_str().unwrap()).map(|b| b.len() as u64) == v["size"].as_u64() { ok(json!({})) } else { (404, b"{\"error\":\"no such object\"}".to_vec()) }
            }
            "POST" if path.starts_with(&format!("{api}/commit/")) => {
                let rev = &path[api.len() + 8..];
                let lines: Vec<Value> = body.split(|&c| c == b'\n').filter(|l| !l.is_empty()).map(|l| serde_json::from_slice(l).unwrap()).collect();
                let summary = lines[0]["value"]["summary"].as_str().unwrap().to_string();
                let who = who.unwrap().to_string();
                let n = if rev == "main?create_pr=1" {
                    let n = hub.prs.keys().max().map_or(1, |n| n + 1);
                    hub.prs.insert(n, Pr { author: who.clone(), open: true, files: hub.main.clone(), head: 0 });
                    Some(n)
                } else if rev == "main" {
                    if who != "tester" {
                        return (403, b"{\"error\":\"not a member\"}".to_vec());
                    }
                    None
                } else {
                    let n: u32 = rev.trim_start_matches("refs%2Fpr%2F").parse().unwrap();
                    match hub.prs.get(&n) {
                        Some(p) if p.open && p.author == who => Some(n),
                        _ => return (403, b"{\"error\":\"not your open pull request\"}".to_vec()),
                    }
                };
                let mut files = match n {
                    Some(n) => std::mem::take(&mut hub.prs.get_mut(&n).unwrap().files),
                    None => std::mem::take(&mut hub.main),
                };
                for l in &lines[1..] {
                    let v = &l["value"];
                    let p = v["path"].as_str().unwrap().to_string();
                    match l["key"].as_str().unwrap() {
                        "file" => {
                            files.insert(p, unbase64(v["content"].as_str().unwrap()));
                        }
                        "lfsFile" => {
                            let Some(b) = hub.lfs.get(v["oid"].as_str().unwrap()) else { return (400, b"{\"error\":\"no LFS object\"}".to_vec()) };
                            files.insert(p, b.clone());
                        }
                        "deletedFolder" => files.retain(|k, _| !k.starts_with(&format!("{p}/"))),
                        other => panic!("a commit line {other}"),
                    }
                }
                match n {
                    Some(n) => {
                        let pr = hub.prs.get_mut(&n).unwrap();
                        pr.files = files;
                        pr.head += 1;
                    }
                    None => hub.main = files,
                }
                hub.commits.push((rev.to_string(), summary));
                let mut out = json!({ "commitUrl": format!("{base}/datasets/{REPO}/commit/x"), "commitOid": "x" });
                if rev == "main?create_pr=1" {
                    out["pullRequestUrl"] = json!(format!("{base}/datasets/{REPO}/discussions/{}", n.unwrap()));
                }
                ok(out)
            }
            "POST" if path.starts_with(&format!("{api}/discussions/")) && path.ends_with("/status") => {
                let n: u32 = path[api.len() + 13..].trim_end_matches("/status").parse().unwrap();
                let v: Value = serde_json::from_slice(body).unwrap();
                match hub.prs.get_mut(&n) {
                    Some(p) if p.author == who.unwrap() => {
                        p.open = v["status"] != "closed";
                        ok(json!({}))
                    }
                    _ => (403, b"{\"error\":\"not yours\"}".to_vec()),
                }
            }
            _ => missing,
        }
    }
}

/// A scratch settings folder and mods folder.
fn scratch(name: &str) -> (PathBuf, PathBuf) {
    let dir = std::env::temp_dir().join(format!("openmx-modstore-up-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("mods")).unwrap();
    (dir.clone(), dir.join("mods"))
}

/// Events until `done` says one is the last, or 30 s.
fn until(s: &Store, done: impl Fn(&Event) -> bool) -> Vec<Event> {
    let (end, mut out) = (Instant::now() + Duration::from_secs(30), Vec::new());
    while Instant::now() < end {
        let got = s.take();
        let last = got.iter().any(&done);
        out.extend(got);
        if last {
            return out;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("no last event in 30 s: {out:?}");
}

/// A music pack of the player's own in `mods`: its `mod.json` (with `permission`) and one song.
fn music(mods: &Path, id: &str, permission: &str, version: &str, song: &[u8]) -> Local {
    let dir = mods.join("music").join(id);
    std::fs::create_dir_all(dir.join("Aud")).unwrap();
    let json = format!(r#"{{"standard": 1, "kind": "music", "id": "{id}", "name": "Songs {id}", "version": "{version}", "author": "me", "permission": "{permission}", "source": {{"game": "Made by me"}}, "songs": [{{"file": "Aud/one.ogg", "title": "One"}}]}}"#);
    std::fs::write(dir.join("mod.json"), json).unwrap();
    std::fs::write(dir.join("Aud/one.ogg"), song).unwrap();
    local_of(&dir, "music").unwrap()
}

fn upload(s: &Store, l: &Local) -> Up {
    s.upload(vec![l.clone()]);
    let got = until(s, |e| matches!(e, Event::Upload(id, up) if *id == l.id && up.over()));
    got.into_iter().rev().find_map(|e| if let Event::Upload(_, up) = e { Some(up) } else { None }).unwrap()
}

fn mine(s: &Store) -> Vec<Entry> {
    s.mine();
    match until(s, |e| matches!(e, Event::Mine(_))).into_iter().find_map(|e| if let Event::Mine(m) = e { Some(m) } else { None }) {
        Some(Ok(m)) => m,
        other => panic!("{other:?}"),
    }
}

fn take_down(s: &Store, e: &Entry, all: &[Entry]) -> Result<String, String> {
    s.take_down(e, all);
    until(s, |ev| matches!(ev, Event::TakenDown(..))).into_iter().find_map(|ev| if let Event::TakenDown(_, r) = ev { Some(r) } else { None }).unwrap()
}

fn sign_in(s: &Store, pasted: &str) -> Result<String, String> {
    s.sign_in(pasted);
    until(s, |e| matches!(e, Event::Signed(_))).into_iter().find_map(|e| if let Event::Signed(r) = e { Some(r) } else { None }).unwrap()
}

#[test]
fn a_token_is_found_in_a_paste_and_kept_owner_only() {
    assert_eq!(token_in(&format!("  {TOKEN_OK}\n")).as_deref(), Some(TOKEN_OK));
    assert_eq!(token_in(&format!("token: \"{TOKEN_OK}\",")).as_deref(), Some(TOKEN_OK));
    assert_eq!(token_in("hf_short"), None);
    assert_eq!(token_in("no token here"), None);
    assert_eq!(account_in(&format!("FIZL0\n{TOKEN_OK}\n")), Some(("FIZL0".into(), TOKEN_OK.into())));
    assert_eq!(account_in(""), None, "signed out");
    assert_eq!(account_in(&format!("\n{TOKEN_OK}")), None, "no name");

    let fake = Fake::start();
    let (dir, mods) = scratch("token");
    let s = Store::new(&fake.base, &dir, &mods);
    // an older file left readable by others is tightened before the token goes in
    std::fs::write(dir.join(TOKEN), "old").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(dir.join(TOKEN), std::fs::Permissions::from_mode(0o644)).unwrap();
    }
    assert!(sign_in(&s, "nothing pasted").unwrap_err().contains("No Hugging Face token"));
    assert!(sign_in(&s, "hf_000000000000000000000000000000000").unwrap_err().contains("didn't take that token"));
    assert!(sign_in(&s, TOKEN_READ).unwrap_err().contains("can only read"));
    assert_eq!(std::fs::read_to_string(dir.join(TOKEN)).unwrap(), "old", "a refused token is never kept");
    assert_eq!(sign_in(&s, &format!("{TOKEN_OK}\n")), Ok("tester".into()));
    assert_eq!(s.signed_in().as_deref(), Some("tester"));
    assert_eq!(account(&dir), Some(("tester".into(), TOKEN_OK.into())));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(std::fs::metadata(dir.join(TOKEN)).unwrap().permissions().mode() & 0o777, 0o600);
    }
    s.sign_out();
    assert_eq!(s.signed_in(), None);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_players_mods_are_listed_and_a_store_install_is_not_theirs() {
    let (dir, mods) = scratch("locals");
    let own = music(&mods, "disc_music", "own-disc", "1.0.0", b"OggS disc");
    music(&mods, "my_music", "public-domain", "1.0.0", b"OggS mine");
    music(&mods, "bare_music", "", "1.0.0", b"OggS bare");
    let installed = music(&mods, "store_music", "public-domain", "1.0.0", b"OggS store");
    std::fs::write(installed.dir.join(STAMP), "{}").unwrap();
    // a folder whose mod.json is another kind's, and one not a mod at all
    std::fs::create_dir_all(mods.join("music/stray")).unwrap();
    let ids: Vec<String> = locals(&mods).into_iter().map(|l| l.id).collect();
    assert_eq!(ids, ["bare_music", "disc_music", "my_music"], "by name, the store's install left out");
    assert_eq!(own.source, "Made by me");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_packed_mod_passes_installs_checks_and_bad_folders_are_refused() {
    let (dir, mods) = scratch("pack");
    let l = music(&mods, "my_music", "public-domain", "1.0.0", &[9; 5000]);
    std::fs::write(l.dir.join("Aud/notes.txt"), "made at home").unwrap();
    std::fs::write(l.dir.join(".DS_Store"), "junk").unwrap();
    let p = package(&mods, &l, "tester").unwrap();
    let Blob::File(zip, size, sha) = p.zip.clone() else { panic!() };
    let z = std::fs::read(&zip).unwrap();
    assert_eq!((z.len() as u64, hex(&Sha256::digest(&z))), (size, sha.clone()));
    assert_eq!((p.meta.author.as_str(), p.meta.kind, p.meta.sha256.as_str(), p.meta.standard), ("tester", Kind::Music, sha.as_str(), 1));
    assert!(p.preview.is_none(), "a music pack has no picture");
    // the zip as Install reads it: every file back, byte for byte
    let mut read = |at: u64, n: usize| z.get(at as usize..at as usize + n).map(<[u8]>::to_vec).ok_or_else(|| "cut short".to_string());
    let d = ZipDir::read(size, &mut read).unwrap();
    let mut got = Vec::new();
    d.each(&mut read, |n, b| {
        got.push((n.to_string(), b));
        Ok(())
    })
    .unwrap();
    assert_eq!(got.iter().map(|g| g.0.as_str()).collect::<Vec<_>>(), ["Aud/notes.txt", "Aud/one.ogg", "mod.json"]);
    assert_eq!(got[1].1, vec![9; 5000]);
    drop(p);
    assert!(!zip.exists(), "the upload's zip goes with it");

    // what Install would refuse is refused here first
    std::fs::write(l.dir.join("setup.exe"), "MZ").unwrap();
    assert!(package(&mods, &l, "tester").err().unwrap().contains(".exe"));
    std::fs::remove_file(l.dir.join("setup.exe")).unwrap();
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink("/etc/passwd", l.dir.join("Aud/two.ogg")).unwrap();
        assert!(package(&mods, &l, "tester").err().unwrap().contains("a link"));
        std::fs::remove_file(l.dir.join("Aud/two.ogg")).unwrap();
    }
    // a mod.json whose kind is not its folder's is not a mod of that folder
    std::fs::write(l.dir.join("mod.json"), r#"{"standard": 1, "kind": "music", "id": "other_id", "name": "x", "permission": "public-domain"}"#).unwrap();
    assert_eq!(local_of(&l.dir, "music"), None);
    assert!(!mods.join(".upload-my_music.zip").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_track_preview_goes_up_as_webp() {
    let (dir, mods) = scratch("preview");
    let still = crate::track_previews::still(&mods, "my_track");
    std::fs::create_dir_all(still.parent().unwrap()).unwrap();
    let mut png = Vec::new();
    {
        let mut e = png::Encoder::new(&mut png, 960, 540);
        e.set_color(png::ColorType::Rgba);
        e.set_depth(png::BitDepth::Eight);
        e.write_header().unwrap().write_image_data(&vec![200; 960 * 540 * 4]).unwrap();
    }
    std::fs::write(&still, &png).unwrap();
    let l = Local { id: "my_track".into(), kind: Kind::Track, name: "Mine".into(), version: String::new(), source: String::new(), category: String::new(), dir: mods.join("tracks/my_track") };
    let w = preview_of(&mods, &l).unwrap();
    assert!(w.starts_with(b"RIFF") && &w[8..12] == b"WEBP");
    let p = decode_preview(&w).unwrap();
    assert_eq!((p.width, p.height), (480, 270), "Browse's width");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_re_upload_is_planned_over_the_players_own_pr() {
    let m = |v: &str, sha: &str| Meta { id: "x".into(), version: v.into(), sha256: sha.into(), ..Meta::default() };
    let e = |v: &str, sha: &str, rev: &str| Entry { meta: m(v, sha), rev: rev.into(), verified: rev == "main" };
    assert_eq!(plan(None, &m("1", "a")), Plan::New);
    assert!(matches!(plan(Some(&e("1", "a", "refs/pr/3")), &m("1", "a")), Plan::Skip(_)), "the same zip");
    assert!(matches!(plan(Some(&e("1.10", "a", "refs/pr/3")), &m("1.9", "b")), Plan::Skip(_)), "an older version");
    assert_eq!(plan(Some(&e("1.0", "a", "refs/pr/3")), &m("1", "b")), Plan::Onto(3), "1.0 is 1");
    assert_eq!(plan(Some(&e("1", "a", "main")), &m("2", "b")), Plan::New, "merged: a new upload");
    assert_eq!(newest_pr(&[e("1", "a", "main"), e("1", "a", "refs/pr/3"), e("1", "a", "refs/pr/7")]), Some(7));
    assert_eq!(pr_in(&json!({ "pullRequestUrl": "https://huggingface.co/datasets/FIZL0/openmx-mods/discussions/12" })), Some(12));
    assert_eq!(base64(b""), "");
    assert_eq!(base64(b"f"), "Zg==");
    assert_eq!(base64(b"fo"), "Zm8=");
    assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    assert_eq!(unbase64(&base64(&(0..=255).collect::<Vec<u8>>())), (0..=255).collect::<Vec<u8>>());
}

#[test]
fn an_upload_is_listed_installed_re_uploaded_and_taken_down() {
    let fake = Fake::start();
    let (dir, mods) = scratch("e2e");
    let s = Store::new(&fake.base, &dir, &mods);
    let song = |n: u8| (0..3000u32).map(|i| (i * 7 + n as u32) as u8).collect::<Vec<u8>>();
    let a = music(&mods, "a_music", "public-domain", "1.0.0", &song(1));
    let b = music(&mods, "b_music", "Made by its uploader", "1.0.0", &song(2));

    // signed out: nothing goes
    assert_eq!(upload(&s, &a), Up::Failed("Sign in first".into()));
    assert_eq!(sign_in(&s, TOKEN_OK), Ok("tester".into()));

    // the first upload opens a pull request, its zip in parts; the kept listing goes stale, and the
    // line says others see it once their Browse reads the store again (it keeps its listing an hour)
    s.list(true);
    until(&s, |e| matches!(e, Event::Listing(_)));
    let kept_at = || serde_json::from_slice::<serde_json::Value>(&std::fs::read(dir.join(CACHE)).unwrap()).unwrap()["at"].as_u64();
    assert_ne!(kept_at(), Some(0));
    let up = upload(&s, &a);
    assert!(matches!(&up, Up::Done(l) if l.contains("pull request #1") && l.contains("after Start on Browse")), "{up:?}");
    assert_eq!(kept_at(), Some(0), "an upload done marks Browse's listing stale");
    {
        let hub = fake.hub.lock().unwrap();
        let pr = &hub.prs[&1];
        assert_eq!((pr.author.as_str(), pr.open), ("tester", true));
        assert_eq!(pr.files.keys().collect::<Vec<_>>(), ["mods/a_music/meta.json", "mods/a_music/mod.zip"]);
        assert_eq!(hub.multipart, 1);
        assert!(!mods.join(".upload-a_music.zip").exists(), "the upload's zip is gone");
    }

    // another player's Browse lists it as tester's, unreviewed, and installs it
    let (dir2, mods2) = scratch("e2e-other");
    let other = Store::new(&fake.base, &dir2, &mods2);
    other.list(true);
    let listed = match until(&other, |e| matches!(e, Event::Listing(_))).into_iter().find_map(|e| if let Event::Listing(l) = e { Some(l) } else { None }) {
        Some(Ok(l)) => l,
        x => panic!("{x:?}"),
    };
    assert_eq!(listed.iter().map(|e| (e.key(), e.rev.as_str(), e.verified)).collect::<Vec<_>>(), [("tester/a_music".to_string(), "refs/pr/1", false)]);
    other.install(&listed[0]);
    let got = until(&other, |e| matches!(e, Event::Installed(..)));
    assert!(got.iter().any(|e| matches!(e, Event::Installed(_, Ok(_)))), "{got:?}");
    assert_eq!(std::fs::read(mods2.join("music/a_music/Aud/one.ogg")).unwrap(), song(1));

    // a new version goes over the copy on the same PR; a second mod joins that PR; the same zip again is skipped
    std::fs::write(a.dir.join("Aud/one.ogg"), song(3)).unwrap();
    let up = upload(&s, &a);
    assert!(matches!(&up, Up::Done(l) if l.contains("updated") && l.contains("#1")), "{up:?}");
    let up = upload(&s, &b);
    assert!(matches!(&up, Up::Done(l) if l.contains("#1")), "{up:?}");
    let up = upload(&s, &b);
    assert!(matches!(&up, Up::Skipped(l) if l.contains("on the store already")), "{up:?}");
    {
        let hub = fake.hub.lock().unwrap();
        assert_eq!(hub.prs.len(), 1, "one PR for the player, not one a mod");
        assert_eq!(hub.commits.len(), 3, "one commit a mod: {:?}", hub.commits);
        assert!(hub.commits[1].1.starts_with("Update mod"), "{:?}", hub.commits);
        let meta: Meta = serde_json::from_slice(&hub.prs[&1].files["mods/a_music/meta.json"]).unwrap();
        assert_eq!(meta.sha256, hex(&Sha256::digest(&hub.prs[&1].files["mods/a_music/mod.zip"])));
        assert_eq!(meta.author, "tester");
    }

    // Upload all: in turn, a status line each
    s.upload(vec![b.clone(), a.clone()]);
    let got = until(&s, |e| matches!(e, Event::Upload(id, up) if id == "a_music" && up.over()));
    assert!(got.iter().any(|e| matches!(e, Event::Upload(id, Up::Skipped(_)) if id == "b_music")), "{got:?}");
    assert!(got.iter().any(|e| matches!(e, Event::Upload(id, Up::Skipped(_)) if id == "a_music")), "{got:?}");

    // My public mods: both, on PR #1
    let all = mine(&s);
    assert_eq!(all.iter().map(|e| (e.meta.id.as_str(), e.rev.as_str())).collect::<Vec<_>>(), [("a_music", "refs/pr/1"), ("b_music", "refs/pr/1")]);
    // not another account's to take down
    let mut theirs = all[0].clone();
    theirs.meta.author = "someone".into();
    assert!(take_down(&s, &theirs, &all).unwrap_err().contains("not yours"));
    // one of two: out of the PR, which stays open
    let said = take_down(&s, &all[0], &all).unwrap();
    assert!(said.contains("taken out of your pull request #1"), "{said}");
    {
        let hub = fake.hub.lock().unwrap();
        assert!(hub.prs[&1].open);
        assert!(!hub.prs[&1].files.keys().any(|k| k.starts_with("mods/a_music/")));
    }
    let left = mine(&s);
    assert_eq!(left.len(), 1);
    // the last: the PR is closed
    let said = take_down(&s, &left[0], &left).unwrap();
    assert!(said.contains("closed"), "{said}");
    assert!(!fake.hub.lock().unwrap().prs[&1].open);
    assert_eq!(mine(&s), []);

    // a merged mod's take-down is a PR deleting it, for a maintainer
    {
        let mut hub = fake.hub.lock().unwrap();
        let files = hub.prs[&1].files.clone();
        hub.main = files;
    }
    let on_main = mine(&s);
    assert_eq!(on_main.iter().map(|e| (e.meta.id.as_str(), e.rev.as_str(), e.verified)).collect::<Vec<_>>(), [("b_music", "main", true)]);
    let said = take_down(&s, &on_main[0], &on_main).unwrap();
    assert!(said.contains("merges pull request #2"), "{said}");
    assert!(!fake.hub.lock().unwrap().prs[&2].files.keys().any(|k| k.starts_with("mods/b_music/")));
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&dir2);
}

#[test]
fn the_maintainers_index_and_card_go_on_main() {
    let fake = Fake::start();
    let (dir, mods) = scratch("index");
    let s = Store::new(&fake.base, &dir, &mods);
    assert_eq!(sign_in(&s, TOKEN_OK), Ok("tester".into()));
    let a = music(&mods, "a_music", "public-domain", "1.0.0", b"OggS a");
    assert!(matches!(upload(&s, &a), Up::Done(_)));
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    let ix = rt.block_on(s.index_now()).unwrap();
    let none = |_: String| {};
    rt.block_on(s.commit(TOKEN_OK, "index", &[("index.json".into(), Blob::Bytes(ix.to_string().into_bytes()))], &[], To::Main, &none)).unwrap();
    rt.block_on(s.commit(TOKEN_OK, "card", &[("README.md".into(), Blob::Bytes(b"# openMX mods\n".to_vec()))], &[], To::Main, &none)).unwrap();
    let hub = fake.hub.lock().unwrap();
    assert_eq!(hub.main["README.md"], b"# openMX mods\n");
    // the index Browse reads: PR #1 at the head it read, its mod as its author's
    let read: super::super::Index = serde_json::from_slice(&hub.main["index.json"]).unwrap();
    assert_eq!(read.pulls[&1].commit, "c1-1");
    let heads = BTreeMap::from([(1, "c1-1".to_string())]);
    let got = parts(Some(read), vec![(1, "tester".into())], &heads);
    match &got[1] {
        Part::Have(l) => assert_eq!(l.iter().map(|e| e.key()).collect::<Vec<_>>(), ["tester/a_music"]),
        other => panic!("{other:?}"),
    }
    drop(hub);
    let _ = std::fs::remove_dir_all(&dir);
}

/// P230: a texture pack of the player's own in `mods/textures/<id>/` is one of their mods, packs into a
/// zip Install's checks (and `textures`' pack reader) take, and goes up with its biggest texture as its
/// picture; a pack with no key-named texture doesn't pack.
#[test]
fn a_texture_pack_uploads() {
    let (dir, mods) = scratch("textures");
    let pack = mods.join("textures/my_pack");
    std::fs::create_dir_all(pack.join("textures/bikes")).unwrap();
    std::fs::write(pack.join("mod.json"), r#"{"standard": 1, "kind": "textures", "id": "my_pack", "name": "My Pack", "version": "1.0.0", "source": {"game": "MX Unleashed"}}"#).unwrap();
    let png = |w: u32, h: u32| {
        let mut b = Vec::new();
        let mut e = png::Encoder::new(&mut b, w, h);
        e.set_color(png::ColorType::Rgba);
        e.set_depth(png::BitDepth::Eight);
        e.write_header().unwrap().write_image_data(&vec![90; (w * h * 4) as usize]).unwrap();
        b
    };
    std::fs::write(pack.join("textures/bikes/frame-00000000000000aa.png"), png(64, 32)).unwrap();
    std::fs::write(pack.join("textures/small-00000000000000bb.png"), png(4, 4)).unwrap();
    let l = locals(&mods).into_iter().find(|l| l.id == "my_pack").expect("the pack is one of the player's mods");
    assert_eq!((l.kind, l.name.as_str(), l.source.as_str()), (Kind::Textures, "My Pack", "MX Unleashed"));
    let p = package(&mods, &l, "tester").unwrap();
    assert_eq!((p.meta.kind, p.meta.id.as_str()), (Kind::Textures, "my_pack"));
    let w = p.preview.as_deref().expect("a pack's picture");
    assert_eq!(decode_preview(w).map(|p| (p.width, p.height)), Some((64, 32)), "its biggest texture");
    drop(p);

    std::fs::remove_file(pack.join("textures/bikes/frame-00000000000000aa.png")).unwrap();
    std::fs::rename(pack.join("textures/small-00000000000000bb.png"), pack.join("textures/00000000000000bb.png")).unwrap();
    let why = package(&mods, &l, "tester").err().unwrap();
    assert!(why.contains("no textures"), "{why}");
    let _ = std::fs::remove_dir_all(&dir);
}
