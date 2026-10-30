//! The mod store's tests (P198): a zip writer for good and bad mods, every bad one refused whole and
//! leaving nothing behind, a seeded fuzz over the zip and meta readers, and a fake store on localhost
//! (its `mod.zip` behind a redirect to a CDN path, as Hugging Face's is) that lists, installs,
//! updates and removes.

use super::*;
use std::collections::HashMap;
use std::io::{Read as _, Write as _};
use std::net::TcpListener;
use std::time::{Duration, Instant};

/// One zip entry, with what a hostile zip may lie about.
#[derive(Clone)]
struct Put {
    name: String,
    data: Vec<u8>,
    deflate: bool,
    /// The Unix mode in the top half of the external attributes (made on Unix), if any.
    mode: Option<u32>,
    flags: u16,
    method: Option<u16>,
    /// The sizes and CRC the headers say, if not the data's.
    size: Option<u32>,
    crc: Option<u32>,
}

fn put(name: &str, data: &[u8]) -> Put {
    Put { name: name.into(), data: data.to_vec(), deflate: true, mode: None, flags: 0, method: None, size: None, crc: None }
}

/// A zip of `files`.
fn zip(files: &[Put]) -> Vec<u8> {
    let (mut out, mut dir) = (Vec::new(), Vec::new());
    for f in files {
        let packed = if f.deflate { miniz_oxide::deflate::compress_to_vec(&f.data, 6) } else { f.data.clone() };
        let method = f.method.unwrap_or(if f.deflate { 8 } else { 0 });
        let (crc, size) = (f.crc.unwrap_or(crc32(&f.data)), f.size.unwrap_or(f.data.len() as u32));
        let at = out.len() as u32;
        out.extend(0x0403_4b50u32.to_le_bytes());
        out.extend([20, 0]);
        out.extend(f.flags.to_le_bytes());
        out.extend(method.to_le_bytes());
        out.extend([0; 4]);
        out.extend(crc.to_le_bytes());
        out.extend((packed.len() as u32).to_le_bytes());
        out.extend(size.to_le_bytes());
        out.extend((f.name.len() as u16).to_le_bytes());
        out.extend([0, 0]);
        out.extend(f.name.as_bytes());
        out.extend(&packed);
        dir.extend(0x0201_4b50u32.to_le_bytes());
        let made: u16 = if f.mode.is_some() { 3 << 8 | 20 } else { 20 };
        dir.extend(made.to_le_bytes());
        dir.extend([20, 0]);
        dir.extend(f.flags.to_le_bytes());
        dir.extend(method.to_le_bytes());
        dir.extend([0; 4]);
        dir.extend(crc.to_le_bytes());
        dir.extend((packed.len() as u32).to_le_bytes());
        dir.extend(size.to_le_bytes());
        dir.extend((f.name.len() as u16).to_le_bytes());
        dir.extend([0; 8]);
        dir.extend((f.mode.unwrap_or(0) << 16).to_le_bytes());
        dir.extend(at.to_le_bytes());
        dir.extend(f.name.as_bytes());
    }
    let dir_at = out.len() as u32;
    out.extend(&dir);
    out.extend(0x0605_4b50u32.to_le_bytes());
    out.extend([0; 4]);
    out.extend((files.len() as u16).to_le_bytes());
    out.extend((files.len() as u16).to_le_bytes());
    out.extend((dir.len() as u32).to_le_bytes());
    out.extend(dir_at.to_le_bytes());
    out.extend([0, 0]);
    out
}

fn mod_json(id: &str, kind: &str) -> Vec<u8> {
    format!(
        r#"{{"standard": 1, "kind": "{kind}", "id": "{id}", "name": "Test {id}", "version": "1.0.0", "author": "someone", "permission": "test", "source": {{"game": "MX vs. ATV Unleashed"}}, "todo": [], "songs": []}}"#
    )
    .into_bytes()
}

/// A track mod's files: its `mod.json` and its one scene.
fn track(id: &str, scene: &[u8]) -> Vec<Put> {
    vec![put("mod.json", &mod_json(id, "track")), put("MC01.scn", scene), put("previews/0.png", &[7; 300])]
}

fn sha(b: &[u8]) -> String {
    hex(&Sha256::digest(b))
}

fn meta(id: &str, kind: Kind, z: &[u8]) -> Meta {
    Meta { id: id.into(), kind, name: format!("Test {id}"), author: "someone".into(), source: "MX vs. ATV Unleashed".into(), version: "1.0.0".into(), size: z.len() as u64, sha256: sha(z), standard: 1, category: String::new() }
}

fn reader(z: &[u8]) -> impl FnMut(u64, usize) -> Result<Vec<u8>, String> + '_ {
    move |at, n| z.get(at as usize..at as usize + n).map(<[u8]>::to_vec).ok_or_else(|| "the zip is cut short".to_string())
}

/// `z` read and unpacked in memory, as Install does it: the directory, `mod.json` against `m`, every
/// file.
fn unpack(z: &[u8], m: &Meta) -> Result<Vec<(String, Vec<u8>)>, String> {
    let mut read = reader(z);
    let dir = ZipDir::read(z.len() as u64, &mut read)?;
    dir.check_mod_json(&mut read, m)?;
    let mut out = Vec::new();
    dir.each(&mut read, |n, b| {
        out.push((n.to_string(), b));
        Ok(())
    })?;
    Ok(out)
}

#[test]
fn a_good_zip_unpacks_and_its_one_folder_is_dropped() {
    let z = zip(&track("mxvatv_mc01", b"scene"));
    let got = unpack(&z, &meta("mxvatv_mc01", Kind::Track, &z)).unwrap();
    assert_eq!(got.iter().map(|f| f.0.as_str()).collect::<Vec<_>>(), ["mod.json", "MC01.scn", "previews/0.png"]);
    assert_eq!(got[1].1, b"scene");

    let wrapped: Vec<Put> = track("mxvatv_mc01", b"scene").into_iter().map(|mut p| {
        p.name = format!("mxvatv_mc01/{}", p.name);
        p
    }).chain([Put { deflate: false, ..put("mxvatv_mc01/", b"") }, put("__MACOSX/._x.png", b"junk"), put("mxvatv_mc01/.DS_Store", b"junk")]).collect();
    let z = zip(&wrapped);
    let got = unpack(&z, &meta("mxvatv_mc01", Kind::Track, &z)).unwrap();
    assert_eq!(got.iter().map(|f| f.0.as_str()).collect::<Vec<_>>(), ["mod.json", "MC01.scn", "previews/0.png"]);
}

/// Every way a mod zip can be hostile or wrong, by what it is.
fn bad_zips() -> Vec<(&'static str, Vec<u8>, Meta)> {
    let id = "bad_mod";
    let good = |extra: Vec<Put>| {
        let mut all = track(id, b"scene");
        all.extend(extra);
        zip(&all)
    };
    let mut out: Vec<(&str, Vec<u8>)> = vec![
        ("climbs out", good(vec![put("../evil.json", b"{}")])),
        ("climbs out deeper", good(vec![put("a/../../evil.json", b"{}")])),
        ("rooted", good(vec![put("/etc/evil.json", b"{}")])),
        ("backslash", good(vec![put("a\\..\\evil.json", b"{}")])),
        ("drive", good(vec![put("C:evil.json", b"{}")])),
        ("device name", good(vec![put("con.json", b"{}")])),
        ("dot part", good(vec![put("a/./b.json", b"{}")])),
        ("trailing dot", good(vec![put("a./b.json", b"{}")])),
        ("control char", good(vec![put("a\nb.json", b"{}")])),
        ("program", good(vec![put("tools/run.exe", b"MZ")])),
        ("library", good(vec![put("lib.so", b"\x7fELF")])),
        ("script", good(vec![put("go.sh", b"#!/bin/sh")])),
        ("no extension", good(vec![put("README", b"hi")])),
        ("symlink", good(vec![Put { mode: Some(0o120777), ..put("link.png", b"/etc/passwd") }])),
        ("device file", good(vec![Put { mode: Some(0o020666), ..put("tty.png", b"") }])),
        ("twice", good(vec![put("mc01.SCN", b"other")])),
        ("encrypted", good(vec![Put { flags: 1, ..put("x.png", b"x") }])),
        ("bzip2", good(vec![Put { method: Some(12), ..put("x.png", b"x") }])),
        ("folder with data", good(vec![Put { deflate: false, size: Some(5), ..put("dir/", b"") }])),
        ("lies about its size", good(vec![Put { size: Some(3), ..put("x.png", &[1; 3000]) }])),
        ("bad crc", good(vec![Put { crc: Some(1), ..put("x.png", b"x") }])),
        ("bomb", good(vec![Put { size: Some(1), crc: Some(0), ..put("bomb.dat", &vec![0; MAX_META as usize * 4]) }])),
        ("brings a store.json", good(vec![put("store.json", b"{}")])),
        ("no mod.json", zip(&[put("MC01.scn", b"scene")])),
        ("two folders", zip(&[put("a/mod.json", &mod_json(id, "track")), put("b/MC01.scn", b"scene")])),
        ("another id", zip(&track("other_mod", b"scene"))),
        ("another kind", zip(&[put("mod.json", &mod_json(id, "music"))])),
        ("a newer standard", zip(&[put("mod.json", &String::from_utf8(mod_json(id, "track")).unwrap().replace("\"standard\": 1", "\"standard\": 9").into_bytes())])),
        ("mod.json not json", zip(&[put("mod.json", b"{ not json")])),
        ("not a zip", b"PK\x03\x04 this is not a zip at all".to_vec()),
        ("empty", Vec::new()),
    ];
    // cut off before its end record
    let mut cut = good(vec![]);
    cut.truncate(cut.len() - 30);
    out.push(("cut short", cut));
    // a zip64 or split archive's end record
    let mut z64 = good(vec![]);
    let n = z64.len();
    z64[n - 12..n - 8].copy_from_slice(&[0xff, 0xff, 0xff, 0xff]);
    out.push(("zip64", z64));
    // more entries than the cap
    let mut many = track(id, b"scene");
    many.extend((0..MAX_ENTRIES).map(|i| Put { deflate: false, ..put(&format!("{i}.txt"), b"") }));
    out.push(("too many files", zip(&many)));
    out.into_iter().map(|(what, z)| (what, z.clone(), meta(id, Kind::Track, &z))).collect()
}

#[test]
fn every_bad_zip_is_refused() {
    for (what, z, m) in bad_zips() {
        assert!(unpack(&z, &m).is_err(), "{what}: taken");
    }
}

#[test]
fn plain_paths() {
    for ok in ["mod.json", "a/b/c.psm", "previews/0.png", "x.y.dat", "dir/"] {
        assert!(plain(ok), "{ok}");
    }
    for bad in ["", "/a", "a//b", "..", "a/..", "./a", "a\\b", "c:x", "NUL", "lpt1.txt", "com9", "a/aux.json", "a ", "a\u{7f}b", &"x".repeat(256)] {
        assert!(!plain(bad), "{bad:?}");
    }
}

#[test]
fn meta_json_is_checked() {
    let z = zip(&track("mxvatv_mc01", b"scene"));
    let good = serde_json::to_vec(&meta("mxvatv_mc01", Kind::Track, &z)).unwrap();
    let m = meta_of(&good, "mxvatv_mc01").unwrap();
    assert_eq!(m.kind, Kind::Track);
    assert!(meta_of(&good, "other_mod").is_none(), "its folder's id");
    let edit = |f: &dyn Fn(&mut serde_json::Value)| {
        let mut v: serde_json::Value = serde_json::from_slice(&good).unwrap();
        f(&mut v);
        meta_of(&serde_json::to_vec(&v).unwrap(), "mxvatv_mc01")
    };
    assert!(edit(&|v| v["sha256"] = "abc".into()).is_none(), "a short sha");
    assert!(edit(&|v| v["sha256"] = "z".repeat(64).into()).is_none(), "not hex");
    assert!(edit(&|v| v["size"] = 0.into()).is_none(), "empty");
    assert!(edit(&|v| v["size"] = (MAX_ZIP + 1).into()).is_none(), "zip64");
    assert!(edit(&|v| v["kind"] = "plugin".into()).is_none(), "a kind with no folder");
    assert!(edit(&|v| v["standard"] = 2.into()).is_none(), "a newer standard");
    assert!(edit(&|v| v["name"] = "\u{1b}\u{7}\n".into()).is_none(), "a name of control characters");
    assert_eq!(edit(&|v| v["name"] = "\u{1b}[2Jx".into()).unwrap().name, "[2Jx");
    let long = edit(&|v| v["name"] = "n".repeat(500).into()).unwrap();
    assert_eq!(long.name.len(), 48);
    let upper = edit(&|v| v["sha256"] = v["sha256"].as_str().unwrap().to_uppercase().into()).unwrap();
    assert_eq!(upper.sha256, m.sha256);
}

/// MD66: `meta.json`'s `category`, which Browse filters the Tracks tab by: kept as plain text of at
/// most 24 characters, and a meta.json without one (every upload before it) still reads, Other or,
/// for a world, World; one with none is written without the key.
#[test]
fn meta_json_carries_the_category() {
    let z = zip(&track("mxvatv_fs01", b"scene"));
    let good = serde_json::to_vec(&meta("mxvatv_fs01", Kind::Track, &z)).unwrap();
    assert!(!String::from_utf8_lossy(&good).contains("category"), "none written where there is none");
    let m = meta_of(&good, "mxvatv_fs01").unwrap();
    assert_eq!((m.category.as_str(), m.category_word().as_str()), ("", "Other"));
    assert_eq!(Meta { kind: Kind::World, ..m.clone() }.category_word(), "World");
    let with = |c: &str| {
        let mut v: serde_json::Value = serde_json::from_slice(&good).unwrap();
        v["category"] = c.into();
        meta_of(&serde_json::to_vec(&v).unwrap(), "mxvatv_fs01").unwrap()
    };
    let fs = with(" Freestyle ");
    assert_eq!((fs.category.as_str(), fs.category_word().as_str()), ("Freestyle", "Freestyle"));
    assert_eq!(with("\u{1b}\u{7}").category_word(), "Other", "control characters alone are none");
    assert_eq!(with(&"c".repeat(100)).category.len(), 24);
    let back = meta_of(&serde_json::to_vec(&fs).unwrap(), "mxvatv_fs01").unwrap();
    assert_eq!(back, fs, "written and read back");
}

/// xorshift64*: the fuzz's seeded randomness.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

/// Damage `b`: flip, overwrite, cut, splice in its own bytes, or write a big number.
fn mutate(r: &mut Rng, mut b: Vec<u8>) -> Vec<u8> {
    for _ in 0..1 + r.below(4) {
        let n = b.len();
        match r.below(5) {
            0 if n > 0 => b[r.below(n)] ^= 1 << r.below(8),
            1 if n > 0 => b[r.below(n)] = r.next() as u8,
            2 => b.truncate(r.below(n + 1)),
            3 if n > 4 => {
                let (from, to, len) = (r.below(n), r.below(n), r.below(16));
                let piece: Vec<u8> = b[from..(from + len).min(n)].to_vec();
                for (i, x) in piece.into_iter().enumerate() {
                    if to + i < b.len() {
                        b[to + i] = x;
                    }
                }
            }
            _ if n > 4 => {
                let at = r.below(n - 3);
                b[at..at + 4].copy_from_slice(&[0xff, 0xff, 0xff, (r.next() as u8) | 0x7f]);
            }
            _ => b.push(r.next() as u8),
        }
    }
    b
}

#[test]
fn the_zip_and_meta_readers_take_any_bytes() {
    let id = "fuzz_mod";
    let mut files = track(id, &[3; 2000]);
    files.push(Put { deflate: false, ..put("stored.dat", b"stored bytes") });
    files.push(put("sub/deep/thing.psm", &(0..4000u32).map(|i| (i * 7 % 251) as u8).collect::<Vec<_>>()));
    let good = zip(&files);
    let m = meta(id, Kind::Track, &good);
    let meta_bytes = serde_json::to_vec(&m).unwrap();
    let mut r = Rng(0x0198_f00d);
    let (mut taken, cases) = (0, 20_000);
    for _ in 0..cases {
        let z = mutate(&mut r, good.clone());
        if let Ok(files) = unpack(&z, &m) {
            taken += 1;
            assert!(files.iter().all(|f| plain(&f.0)), "an unplain name taken");
            assert!(files.iter().map(|f| f.1.len() as u64).sum::<u64>() <= MAX_TOTAL);
        }
        let _ = meta_of(&mutate(&mut r, meta_bytes.clone()), id);
        let _ = check_mod_json(&mutate(&mut r, mod_json(id, "track")), &m);
        let _ = ids_in(&mutate(&mut r, br#"[{"type":"directory","path":"mods/fuzz_mod"}]"#.to_vec()));
        let _ = pulls_in(&mutate(&mut r, br#"{"discussions":[{"num":3,"isPullRequest":true,"status":"open","author":{"name":"a"}}],"count":1}"#.to_vec()));
        let _ = decode_preview(&mutate(&mut r, webp(8, 4)));
    }
    // most damage is caught; a flip in a stored file's padding or a zip comment is not damage
    assert!(taken < cases / 2, "{taken} of {cases} damaged zips taken");
}

/// A `w`×`h` lossless WebP.
fn webp(w: u32, h: u32) -> Vec<u8> {
    let rgba: Vec<u8> = (0..w * h).flat_map(|i| [(i * 9) as u8, 80, 200, 255]).collect();
    let mut out = Vec::new();
    image_webp::WebPEncoder::new(&mut out).encode(&rgba, w, h, image_webp::ColorType::Rgba8).unwrap();
    out
}

#[test]
fn previews_decode_and_shrink() {
    let p = decode_preview(&webp(960, 540)).unwrap();
    assert_eq!((p.width, p.height), (PREVIEW_W as usize, 270));
    assert_eq!(&p.rgba[1..4], &[80, 200, 255]);
    assert!(decode_preview(b"not a picture").is_none());
    assert!(decode_preview(b"").is_none());
}

#[test]
fn listings_merge_by_id_and_author() {
    let z = zip(&track("a_track", b"x"));
    let e = |id: &str, author: &str, rev: &str, sha: &str| {
        let mut m = meta(id, Kind::Track, &z);
        m.author = author.into();
        m.sha256 = sha.into();
        m.name = id.into();
        entry(m, rev, None)
    };
    let main = vec![e("b_track", "ann", "main", "1"), e("a_track", "ann", "main", "2")];
    let pr = vec![e("a_track", "ann", "refs/pr/4", "3"), e("a_track", "bob", "refs/pr/4", "4"), e("b_track", "cat", "refs/pr/4", "1")];
    let all = merge(vec![main, pr]);
    let keys: Vec<(String, bool)> = all.iter().map(|e| (e.key(), e.verified)).collect();
    // ann's PR of a_track is under her main copy; bob's is his own; cat's copy of b_track is main's zip
    assert_eq!(keys, [("ann/a_track".into(), true), ("bob/a_track".into(), false), ("ann/b_track".into(), true)]);
}

#[test]
fn the_index_is_read_only_where_it_is_current() {
    let z = zip(&track("a_track", b"x"));
    let m = meta("a_track", Kind::Track, &z);
    let ix = |pull_commit: &str| Index {
        version: 1,
        pulls: [(7, Pull { commit: pull_commit.into() })].into(),
        entries: vec![entry(m.clone(), "main", None), entry(m.clone(), "refs/pr/7", None)],
    };
    let heads: BTreeMap<u32, String> = [(7, "abc".to_string()), (9, "def".to_string())].into();
    let open = vec![(9, "bob".to_string()), (7, "ann".to_string())];
    let got = parts(Some(ix("abc")), open.clone(), &heads);
    assert!(matches!(&got[0], Part::Have(l) if l.len() == 1 && l[0].verified));
    assert_eq!(got[1], Part::Read("refs/pr/9".into(), Some("bob".into())));
    assert!(matches!(&got[2], Part::Have(l) if l.len() == 1 && !l[0].verified && l[0].meta.author == "ann"));
    // a PR pushed to since the index was built is read again
    assert_eq!(parts(Some(ix("old")), open.clone(), &heads)[2], Part::Read("refs/pr/7".into(), Some("ann".into())));
    // no index: every rev read
    assert_eq!(parts(None, open, &heads).len(), 3);
}

#[test]
fn report_prefills_the_id() {
    let z = zip(&track("a_track", b"x"));
    let url = report_url(&entry(meta("a_track", Kind::Track, &z), "refs/pr/3", None));
    assert!(url.starts_with("https://huggingface.co/datasets/FIZL0/openmx-mods/discussions/new?title=Report%3A%20Test%20a_track%20%28a_track%29&description=Mod%20id%3A%20a_track%0A"), "{url}");
}

// ---- a fake store on localhost ------------------------------------------------------------------

/// What the fake store answers: a path's status and body; `mod.zip` paths redirect to `/cdn/…`.
#[derive(Default)]
struct Routes {
    files: HashMap<String, (u16, Vec<u8>)>,
    asked: Vec<String>,
}

struct Fake {
    base: String,
    routes: Arc<Mutex<Routes>>,
}

impl Fake {
    fn start() -> Fake {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", l.local_addr().unwrap());
        let routes = Arc::new(Mutex::new(Routes::default()));
        let r = routes.clone();
        std::thread::spawn(move || {
            for c in l.incoming() {
                let Ok(mut c) = c else { continue };
                let r = r.clone();
                std::thread::spawn(move || {
                    let _ = c.set_read_timeout(Some(Duration::from_secs(10)));
                    let mut head = Vec::new();
                    let mut b = [0u8; 1];
                    while !head.ends_with(b"\r\n\r\n") && c.read(&mut b).map_or(false, |n| n == 1) {
                        head.push(b[0]);
                    }
                    let line = String::from_utf8_lossy(&head).lines().next().unwrap_or("").to_string();
                    let path = line.split(' ').nth(1).unwrap_or("").to_string();
                    let mut routes = r.lock().unwrap();
                    routes.asked.push(path.clone());
                    let reply = if path.ends_with("/mod.zip") && !path.starts_with("/cdn/") {
                        format!("HTTP/1.1 302 Found\r\nLocation: /cdn{path}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").into_bytes()
                    } else {
                        let key = path.strip_prefix("/cdn").filter(|_| path.starts_with("/cdn/")).unwrap_or(&path).to_string();
                        let (status, body) = routes.files.get(&key).cloned().unwrap_or((404, b"{\"error\":\"not found\"}".to_vec()));
                        let mut out = format!("HTTP/1.1 {status} X\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len()).into_bytes();
                        out.extend(body);
                        out
                    };
                    drop(routes);
                    let _ = c.write_all(&reply);
                });
            }
        });
        Fake { base, routes }
    }

    fn set(&self, path: &str, status: u16, body: Vec<u8>) {
        self.routes.lock().unwrap().files.insert(path.into(), (status, body));
    }

    fn file(rev: &str, path: &str) -> String {
        format!("/datasets/{REPO}/resolve/{}/{path}", rev_part(rev))
    }

    /// Mod `id` on `rev`: its meta.json, mod.zip and (if any) preview.webp.
    fn publish(&self, rev: &str, m: &Meta, z: &[u8], preview: Option<Vec<u8>>) {
        self.set(&Fake::file(rev, &format!("mods/{}/meta.json", m.id)), 200, serde_json::to_vec(m).unwrap());
        self.set(&Fake::file(rev, &format!("mods/{}/mod.zip", m.id)), 200, z.to_vec());
        if let Some(p) = preview {
            self.set(&Fake::file(rev, &format!("mods/{}/preview.webp", m.id)), 200, p);
        }
    }

    /// `rev`'s tree of mods.
    fn tree(&self, rev: &str, ids: &[&str]) {
        let nodes: Vec<serde_json::Value> = ids.iter().map(|id| serde_json::json!({"type": "directory", "path": format!("mods/{id}")})).collect();
        self.set(&format!("/api/datasets/{REPO}/tree/{}/mods", rev_part(rev)), 200, serde_json::to_vec(&nodes).unwrap());
    }

    /// The open PRs, `(number, author)`, and their heads.
    fn pulls(&self, open: &[(u32, &str)]) {
        let list: Vec<serde_json::Value> = open.iter().map(|(n, a)| serde_json::json!({"num": n, "isPullRequest": true, "status": "open", "author": {"name": a}})).collect();
        self.set(&format!("/api/datasets/{REPO}/discussions?type=pull_request&status=open&p=0"), 200, serde_json::to_vec(&serde_json::json!({"discussions": list, "count": open.len()})).unwrap());
        let refs: Vec<serde_json::Value> = open.iter().map(|(n, _)| serde_json::json!({"name": n.to_string(), "targetCommit": format!("head{n}")})).collect();
        self.set(&format!("/api/datasets/{REPO}/refs?include_prs=1"), 200, serde_json::to_vec(&serde_json::json!({"pullRequests": refs})).unwrap());
    }

    fn asked(&self) -> Vec<String> {
        self.routes.lock().unwrap().asked.clone()
    }
}

/// A scratch settings folder and mods folder.
fn scratch(name: &str) -> (PathBuf, PathBuf) {
    let dir = std::env::temp_dir().join(format!("openmx-modstore-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
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

fn listing(s: &Store, fresh: bool) -> Vec<Entry> {
    s.list(fresh);
    let got = until(s, |e| matches!(e, Event::Listing(_)));
    match got.into_iter().find_map(|e| if let Event::Listing(l) = e { Some(l) } else { None }) {
        Some(Ok(l)) => l,
        other => panic!("{other:?}"),
    }
}

fn install(s: &Store, e: &Entry) -> Result<String, String> {
    s.install(e);
    let got = until(s, |ev| matches!(ev, Event::Installed(..)));
    got.into_iter().find_map(|ev| if let Event::Installed(_, r) = ev { Some(r) } else { None }).unwrap()
}

/// Everything under `dir`, by path.
fn tree_of(dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let mut todo = vec![dir.to_path_buf()];
    while let Some(d) = todo.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            out.push(e.path().strip_prefix(dir).unwrap().to_string_lossy().into_owned());
            if e.path().is_dir() {
                todo.push(e.path());
            }
        }
    }
    out.sort();
    out
}

#[test]
fn the_fake_store_lists_installs_updates_and_removes() {
    let fake = Fake::start();
    let (dir, mods) = scratch("round");
    let z1 = zip(&track("store_track", b"scene one"));
    let m1 = meta("store_track", Kind::Track, &z1);
    let music = zip(&[put("mod.json", &mod_json("store_music", "music")), put("song.ogg", b"OggS")]);
    let mm = meta("store_music", Kind::Music, &music);
    fake.tree("main", &["store_track"]);
    fake.publish("main", &m1, &z1, Some(webp(32, 18)));
    fake.tree("refs/pr/5", &["store_music"]);
    fake.publish("refs/pr/5", &mm, &music, None);
    fake.pulls(&[(5, "dj")]);

    let s = Store::new(&fake.base, &dir, &mods);
    let mut l = listing(&s, false);
    // by name: Test store_music, then Test store_track
    assert_eq!(l.iter().map(|e| (e.key(), e.verified)).collect::<Vec<_>>(), [("dj/store_music".into(), false), ("someone/store_track".into(), true)]);
    l.reverse();
    assert_eq!(Store::have(&l[0], &s.stamps(), &s.own_ids()), Have::No);

    // M116: the second Browse in the hour is the kept listing, not the store's
    let asked = fake.asked().len();
    assert_eq!(listing(&s, false).into_iter().rev().collect::<Vec<_>>(), l);
    assert_eq!(fake.asked().len(), asked);

    // B259: a preview by its zip's sha, kept
    s.preview(&l[0]);
    let got = until(&s, |e| matches!(e, Event::Preview(..)));
    assert!(got.iter().any(|e| matches!(e, Event::Preview(sha, Some(p)) if *sha == m1.sha256 && p.width == 32)), "{got:?}");

    // install, through the redirect to the CDN
    assert_eq!(install(&s, &l[0]), Ok("Test store_track".into()));
    assert!(fake.asked().iter().any(|p| p.starts_with("/cdn/") && p.ends_with("store_track/mod.zip")));
    let t = track_mods::find(&mods, "store_track").unwrap();
    assert_eq!(t.stem, "MC01");
    let stamps = s.stamps();
    assert_eq!(stamps.len(), 1);
    assert_eq!((stamps[0].sha256.as_str(), stamps[0].verified, stamps[0].rev.as_str()), (m1.sha256.as_str(), true, "main"));
    assert_eq!(Store::have(&l[0], &stamps, &s.own_ids()), Have::Installed);
    assert_eq!(tree_of(&mods), ["tracks", "tracks/store_track", "tracks/store_track/MC01.scn", "tracks/store_track/mod.json", "tracks/store_track/previews", "tracks/store_track/previews/0.png", "tracks/store_track/store.json"]);

    // the music pack off a PR, as its author's
    assert_eq!(install(&s, &l[1]), Ok("Test store_music".into()));
    assert_eq!(track_mods::music_packs(&mods).len(), 1);

    // an update: a new zip on main, the listing read again after an install
    let z2 = zip(&track("store_track", b"scene two"));
    let mut m2 = meta("store_track", Kind::Track, &z2);
    m2.version = "1.1.0".into();
    fake.publish("main", &m2, &z2, Some(webp(16, 9)));
    let l2 = listing(&s, false);
    let e2 = l2.iter().find(|e| e.meta.id == "store_track").unwrap();
    assert_eq!(Store::have(e2, &s.stamps(), &s.own_ids()), Have::Update);
    s.preview(e2);
    let got = until(&s, |e| matches!(e, Event::Preview(..)));
    assert!(got.iter().any(|e| matches!(e, Event::Preview(sha, Some(p)) if *sha == m2.sha256 && p.width == 16)), "B259: {got:?}");
    assert_eq!(install(&s, e2), Ok("Test store_track".into()));
    assert_eq!(std::fs::read(mods.join("tracks/store_track/MC01.scn")).unwrap(), b"scene two");
    assert_eq!(Store::have(e2, &s.stamps(), &s.own_ids()), Have::Installed);

    // remove
    s.remove(&e2.key(), Kind::Track, "store_track");
    let got = until(&s, |e| matches!(e, Event::Removed(..)));
    assert!(got.iter().any(|e| matches!(e, Event::Removed(_, Ok(_)))), "{got:?}");
    assert!(!mods.join("tracks/store_track").exists());
    assert_eq!(tree_of(&mods.join("tracks")), Vec::<String>::new());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn every_bad_mod_is_refused_and_leaves_nothing() {
    let fake = Fake::start();
    let (dir, mods) = scratch("bad");
    let s = Store::new(&fake.base, &dir, &mods);
    let mut cases: Vec<(String, Vec<u8>, Meta)> = bad_zips().into_iter().map(|(w, z, m)| (w.to_string(), z, m)).collect();
    // what only the download or the shell's parser catches
    let good = zip(&track("bad_mod", b"scene"));
    let mut wrong_sha = meta("bad_mod", Kind::Track, &good);
    wrong_sha.sha256 = "0".repeat(64);
    cases.push(("another sha256".into(), good.clone(), wrong_sha));
    let mut short = meta("bad_mod", Kind::Track, &good);
    short.size -= 1;
    cases.push(("more than listed".into(), good.clone(), short));
    let mut long = meta("bad_mod", Kind::Track, &good);
    long.size += 1;
    cases.push(("less than listed".into(), good.clone(), long));
    let no_scene = zip(&[put("mod.json", &mod_json("bad_mod", "track"))]);
    cases.push(("a track the shell can't read".into(), no_scene.clone(), meta("bad_mod", Kind::Track, &no_scene)));
    let bad_json = zip(&[put("mod.json", &String::from_utf8(mod_json("bad_mod", "track")).unwrap().replace("\"1.0.0\"", "\"one\"").into_bytes()), put("MC01.scn", b"x")]);
    cases.push(("a version the shell refuses".into(), bad_json.clone(), meta("bad_mod", Kind::Track, &bad_json)));

    for (what, z, m) in cases {
        fake.publish("main", &m, &z, None);
        let e = entry(m, "main", None);
        let got = install(&s, &e);
        assert!(got.is_err(), "{what}: installed");
        assert!(tree_of(&mods).iter().all(|p| p == "tracks"), "{what}: left {:?}", tree_of(&mods));
        assert!(!s.busy());
    }

    // the player's own folder of the id is never overwritten
    std::fs::create_dir_all(mods.join("tracks/bad_mod")).unwrap();
    std::fs::write(mods.join("tracks/bad_mod/mod.json"), b"mine").unwrap();
    fake.publish("main", &meta("bad_mod", Kind::Track, &good), &good, None);
    let e = entry(meta("bad_mod", Kind::Track, &good), "main", None);
    assert_eq!(Store::have(&e, &s.stamps(), &s.own_ids()), Have::Own);
    assert!(install(&s, &e).is_err());
    assert_eq!(std::fs::read(mods.join("tracks/bad_mod/mod.json")).unwrap(), b"mine");
    s.remove(&e.key(), Kind::Track, "bad_mod");
    let got = until(&s, |ev| matches!(ev, Event::Removed(..)));
    assert!(got.iter().any(|ev| matches!(ev, Event::Removed(_, Err(_)))));
    assert!(mods.join("tracks/bad_mod/mod.json").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_busy_store_leaves_the_last_listing_up() {
    let fake = Fake::start();
    let (dir, mods) = scratch("busy");
    let z = zip(&track("store_track", b"x"));
    fake.tree("main", &["store_track"]);
    fake.publish("main", &meta("store_track", Kind::Track, &z), &z, None);
    fake.pulls(&[]);
    let s = Store::new(&fake.base, &dir, &mods);
    let first = listing(&s, true);
    assert_eq!(first.len(), 1);
    fake.set(&format!("/api/datasets/{REPO}/discussions?type=pull_request&status=open&p=0"), 429, b"slow down".to_vec());
    s.list(true);
    let got = until(&s, |e| matches!(e, Event::Note(_)));
    assert!(got.contains(&Event::Listing(Ok(first))), "{got:?}");
    assert!(got.contains(&Event::Note(BUSY.into())));
    // with nothing kept, it says so
    let (dir2, mods2) = scratch("busy-none");
    let s2 = Store::new(&fake.base, &dir2, &mods2);
    s2.list(true);
    let got = until(&s2, |e| matches!(e, Event::Listing(_)));
    assert!(got.contains(&Event::Listing(Err(BUSY.into()))), "{got:?}");
    let _ = (std::fs::remove_dir_all(&dir), std::fs::remove_dir_all(&dir2));
}

#[test]
fn installs_run_a_few_at_a_time() {
    let fake = Fake::start();
    let (dir, mods) = scratch("parallel");
    let s = Store::new(&fake.base, &dir, &mods);
    let all: Vec<Entry> = (0..PARALLEL + 3)
        .map(|i| {
            let id = format!("music_{i}");
            let z = zip(&[put("mod.json", &mod_json(&id, "music"))]);
            let m = meta(&id, Kind::Music, &z);
            fake.publish("main", &m, &z, None);
            entry(m, "main", None)
        })
        .collect();
    for e in &all {
        s.install(e);
        s.install(e);
    }
    assert!(s.busy());
    let mut done = 0;
    let end = Instant::now() + Duration::from_secs(30);
    while done < all.len() && Instant::now() < end {
        assert!(s.shared.jobs.lock().unwrap().running <= PARALLEL);
        done += s.take().iter().filter(|e| matches!(e, Event::Installed(_, Ok(_)))).count();
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(done, all.len());
    assert_eq!(track_mods::music_packs(&mods).len(), all.len());
    let _ = std::fs::remove_dir_all(&dir);
}

/// A 2×2 PNG, its first pixel `r`.
fn png_of(r: u8) -> Vec<u8> {
    let at = std::env::temp_dir().join(format!("openmx-modstore-png-{r}-{}.png", std::process::id()));
    let t = game_render::pst::Texture { width: 2, height: 2, rgba: [r, 2, 3, 255].into_iter().chain([9; 12]).collect() };
    crate::textures::write_png(&at, &t).unwrap();
    let b = std::fs::read(&at).unwrap();
    let _ = std::fs::remove_file(&at);
    b
}

/// P230: a texture pack off the store: listed on its own tab, staged, read through `textures`' pack
/// reader, installed into `mods/textures/<id>/`, and then found by a ride's lookup at its keys, on top
/// until Pack Order's lines say otherwise; a pack with no key-named texture, or a picture that isn't a
/// PNG, refused and leaving nothing.
#[test]
fn a_texture_pack_round_trips() {
    assert_eq!((Kind::Textures.word(), Kind::Textures.folder(), Kind::Textures.tab().map(|t| TABS[t])), ("textures", Some("textures"), Some("Texture Packs")));
    let fake = Fake::start();
    let (dir, mods) = scratch("textures");
    let (a, b) = ("00000000000000aa", "0123456789abcdef");
    let pack = zip(&[put("mod.json", &mod_json("hd_pack", "textures")), put(&format!("textures/bikes/frame-{a}.png"), &png_of(1)), put(&format!("textures/{b}.png"), &png_of(2)), put(&format!("textures/ground/dirt-{b}.png"), &png_of(3))]);
    let m = meta("hd_pack", Kind::Textures, &pack);
    assert_eq!(meta_of(&serde_json::to_vec(&m).unwrap(), "hd_pack").map(|m| m.kind), Some(Kind::Textures));
    fake.tree("main", &["hd_pack"]);
    fake.publish("main", &m, &pack, None);
    fake.pulls(&[]);
    let s = Store::new(&fake.base, &dir, &mods);
    let l = listing(&s, false);
    assert_eq!(l.iter().map(|e| (e.meta.kind, e.meta.kind.tab())).collect::<Vec<_>>(), [(Kind::Textures, Some(3))]);
    assert_eq!(install(&s, &l[0]), Ok("Test hd_pack".into()));
    assert_eq!(s.stamps().iter().map(|st| (st.id.as_str(), st.kind)).collect::<Vec<_>>(), [("hd_pack", Kind::Textures)]);
    assert_eq!(Store::have(&l[0], &s.stamps(), &s.own_ids()), Have::Installed);
    assert!(mods.join("textures/hd_pack/store.json").is_file());

    // the ride's lookup: both keys from the pack (the bare `<key>.png` has no `-`, so isn't key-named)
    let found = crate::textures::scan(&mods, &[], &[]);
    assert_eq!((found.packs, found.from_packs), (1, 2));
    assert!(found.files[a].ends_with(format!("textures/hd_pack/textures/bikes/frame-{a}.png")));
    assert!(found.files[b].ends_with(format!("textures/hd_pack/textures/ground/dirt-{b}.png")));
    // Pack Order's listing, and its off line
    let rows = crate::textures::listed(&mods, &[], &[]);
    assert_eq!(rows, [crate::textures::Listed { folder: "textures/hd_pack".into(), name: "Test hd_pack".into(), textures: 2, on: true }]);
    let off = ["textures/hd_pack".to_string()];
    assert!(!crate::textures::listed(&mods, &off, &off)[0].on);
    assert_eq!(crate::textures::scan(&mods, &off, &off).packs, 0);

    // what the pack reader refuses, at install, leaving the installed copy alone
    let bad = [
        ("no key-named texture", zip(&[put("mod.json", &mod_json("bad_pack", "textures")), put(&format!("textures/{a}.png"), &png_of(1))])),
        ("not a png", zip(&[put("mod.json", &mod_json("bad_pack", "textures")), put(&format!("textures/x-{a}.png"), b"not a png at all")])),
        ("a track's mod.json", zip(&[put("mod.json", &mod_json("bad_pack", "track")), put(&format!("textures/x-{a}.png"), &png_of(1))])),
    ];
    for (what, z) in bad {
        let m = meta("bad_pack", Kind::Textures, &z);
        fake.publish("main", &m, &z, None);
        let got = install(&s, &entry(m, "main", None));
        assert!(got.is_err(), "{what}: installed");
        assert!(!mods.join("textures/bad_pack").exists() && !mods.join(".store-bad_pack").exists(), "{what}: left {:?}", tree_of(&mods));
    }
    assert_eq!(crate::textures::scan(&mods, &[], &[]).packs, 1);

    s.remove(&l[0].key(), Kind::Textures, "hd_pack");
    let got = until(&s, |ev| matches!(ev, Event::Removed(..)));
    assert!(got.iter().any(|ev| matches!(ev, Event::Removed(_, Ok(_)))), "{got:?}");
    assert_eq!(crate::textures::scan(&mods, &[], &[]).packs, 0);
    let _ = std::fs::remove_dir_all(&dir);
}
