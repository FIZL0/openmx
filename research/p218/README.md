# P218 — the mod store against huggingface.co itself (2026-11-02)

P198 proved the store only against a localhost fake. This is the same round trip on the live store,
FIZL0/openmx-mods, on the desktop and in the browser. The store's only content today is open PR #1
(mxunleashed, "Uploaded from openMX", 108 mods, 2026-10-30); `main` has the dataset card and no `mods/`
and no `index.json`, so nothing is Verified yet.

## How to run it again

- Desktop: `cargo test -p openmx --bin openmx live_store -- --ignored --nocapture`
  (`modstore_tests.rs`, `live_store_lists_installs_updates_and_removes`). It lists, takes the smallest track,
  fetches its preview, installs it, makes its `store.json` stale so it's an Update and installs it again,
  removes it, and prints Report's URL.
- Browser: `cargo test -p openmx --test web the_browser_uses_the_live_store -- --ignored`
  (`tests/web/store_live.mjs`; disc, node and chromium needed). The page opens on `--screen store` with no
  `--mod-store`. Cross installs the first card into OPFS, Triangle (G) opens Report, and Square (V) removes it.
  Both are ignored because they need the network, and the gate runs offline.

## What it showed (all passed)

| What | Live reply | openMX |
|---|---|---|
| `api/datasets/…/tree/main/mods` | 404 `EntryNotFound` (no `mods/` on main) | read as no mods (`ids`) ✓ |
| `tree/refs%2Fpr%2F1/mods` | 200, 108 `{"type":"directory","path":"mods/<id>"}`, no paging (`Link` absent) | `ids_in` ✓ |
| `discussions?type=pull_request&status=open&p=0` | `{discussions:[{num,author:{name},status:"open",isPullRequest:true}],count}` | `pulls_in` ✓ |
| `refs?include_prs=1` | `{pullRequests:[{name:"1",targetCommit}]}` | `heads_in` ✓ |
| `resolve/main/index.json` | 404 `EntryNotFound` | no index: every PR's metas are read ✓ |
| `resolve/refs%2Fpr%2F1/…` (encoded rev) | 307 to `/api/resolve-cache/…` (same host) for a small file | followed ✓ |
| `resolve/…/mod.zip` (LFS/xet) | 302 to `us.aws.cdn.hf.co/xet-bridge-us/…`, `x-linked-size`, `x-linked-etag` = sha256 | followed, sha256 matches `meta.json` ✓ |
| CDN with `Range: bytes=N-` | 206, `content-range`, `accept-ranges: bytes` | P241 resume path is sound |
| CORS, API and resolve | `access-control-allow-origin` echoes the page's origin (127.0.0.1, openmx.dev, beta.openmx.dev); preflight with `range` allowed | ✓ |
| CORS, CDN | `access-control-allow-origin: *`, preflight allows every header | ✓ |
| Report `discussions/new?title=…&description=…` | signed out: 302 to `/login?next=<the whole prefilled URL>` | the page's `window.open` opens it (CDP saw the tab land on the sign-in page) |

Desktop: 108 listed, 0 verified, a 2 MB track installed off `refs/pr/1`, updated and removed in 7.7 s.
Browser: 19th Hole (mxvatv_mc08, the grid's first card) went into OPFS (781 files, `store.json` sha256 =
the meta's), the download left nothing in OPFS `store/`, Free Ride's list said `mods: 1 tracks`, Report
opened its tab, and Remove emptied OPFS `mods/`. A listing took about 218 requests to huggingface.co.

## Found

- In 1 of the 5 browser runs, two `meta.json` resolves failed with `MissingAllowOriginHeader`: a reply
  from HF's edge with no CORS header, probably a 429 or 5xx. 216 curl requests didn't reproduce it.
  `Store::get_meta` reads every error except `BUSY` as "no mod", so a failure like this drops the mod from
  Browse for the hour the listing is kept. Filed as a bug.
- No `index.json` on `main` yet (M116), so every fresh Browse makes about 110 requests (218 in the browser,
  counting redirects). `--mod-store-admin index` writes it, but that needs the maintainer's token. Filed.
- What I couldn't check: whether discussions/new prefills the title and description for a signed-in user.
  That needs a signed-in browser. A signed-out player is sent to sign in, and the prefill rides along in
  `next=`.
