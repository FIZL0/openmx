// The mod store in the page (P198; crates/openmx/tests/web.rs, `the_browser_installs_from_the_store`): web/ served
// on loopback with a fake store beside it under /store (the openmx-mods dataset's API and files, its mod.zip behind a
// redirect to /cdn as Hugging Face's is), opened in headless chromium with `?software&args=--screen store --mod-store …`, the
// disc picked, then Space (Cross) on the store's one card until the mod is in OPFS `mods/`. Then (P217) the page is
// opened again with `--track <the mod>`, and rides the installed track out of OPFS.
//
//   node store.mjs <web dir> <pkg dir> <disc.iso> <scratch profile dir>
//
// Prints one JSON line: { state, files: [OPFS mods/ paths], stamp, left: [OPFS store/ names], zips, ranges, presses,
// listed, pictured, rode, ride, frames } — `stamp` the installed store.json's sha256, `left` what is still in the downloads folder,
// `zips` the mod.zip requests the store and its CDN saw, `ranges` how many asked a Range; `listed` the shell's `mods:`
// line once the install made it read the mods folder again (Free Ride's list), `pictured` its line for the track cards'
// pictures then (P261), `rode` its line for the ride's track,
// `ride` the page's state through the ride and `frames` the pictures it drew in its last seconds.
// With OPENMX_STORE_LOG set, each store request is printed to stderr; with OPENMX_STORE_SHOT a path, the page's last
// picture is written there.
import http from 'node:http';
import fs from 'node:fs';
import path from 'node:path';
import zlib from 'node:zlib';
import crypto from 'node:crypto';
import { spawn } from 'node:child_process';

const [webdir, pkg, iso, profile] = process.argv.slice(2);
const REPO = 'FIZL0/openmx-mods';
const ID = 'web_store_track';

// The mod: a track's mod.json, a scene and the terrain and wall it names — tests/track_mods.rs's flat MD2 track, so the
// page can ride it — a filler of 2 MB that does not deflate much (so the download comes in many pieces), and a preview
// still, zipped as the store's tooling zips (deflate, made on Unix). No byte of it comes off a disc.
const modJson = JSON.stringify({ standard: 1, kind: 'track', id: ID, name: 'Web Store Track', version: '1.0.0', author: 'someone',
  permission: 'test', source: { game: 'MX vs. ATV Unleashed' }, todo: [], songs: [] });
let seed = 0x198;
const filler = Buffer.alloc(2 << 20);
for (let i = 0; i < filler.length; i++) filler[i] = (seed = (Math.imul(seed, 1103515245) + 12345) >>> 0) >>> 24;
const bytes = () => {
  const out = [];
  const put = {
    word: w => out.push(...new Uint8Array(new Uint32Array([w >>> 0]).buffer)),
    single: f => out.push(...new Uint8Array(new Float32Array([f]).buffer)),
    half: h => out.push(h & 0xff, h >> 8),
    fill: (n, b) => out.push(...new Array(n).fill(b)),
  };
  return { out, ...put };
};
// A flat two-level PLN v7 terrain at height 0: one root cell, one block its 16x16 leaves share, 4 feet a sample.
function flatTerrain() {
  const t = bytes();
  const node = (block, childrenAt) => {
    [1.0, 0.5, 0.0, 0.0].forEach(t.single);
    t.half(0);
    t.half(block);
    t.fill(4 + 64 + 4, 0);
    t.out.push(1);
    t.word(childrenAt);
  };
  t.out.push(...Buffer.from('PLN\x007ps2\x00', 'latin1'));
  const name = Buffer.alloc(32);
  name.write('mdflat');
  t.out.push(...name);
  t.word(0);
  t.single(4.0);
  t.word(2);
  const treeSlot = t.out.length;
  [0, 1, 1, 0, 0, 0].forEach(t.word);
  t.word(0);
  t.fill(17 * 20 * 2, 0);
  for (let i = 0; i < 289; i++) t.out.push(0, 127, 0);
  t.fill(289, 0);
  t.fill(289, 0x81);
  const treeAt = t.out.length;
  t.out.splice(treeSlot, 4, ...new Uint8Array(new Uint32Array([treeAt]).buffer));
  const rootChildren = treeAt + 97;
  const cellAt = rootChildren + 256 * 4;
  const cellChildren = cellAt + 97;
  const leafAt = cellChildren + 256 * 4;
  node(0, rootChildren);
  t.word(cellAt);
  for (let i = 1; i < 256; i++) t.word(0);
  node(0, cellChildren);
  for (let i = 0; i < 256; i++) t.word(leafAt);
  node(0, 0);
  return Buffer.from(t.out);
}
// A wall: two triangles 40 feet wide and 20 high, facing -z.
function wallCol() {
  const c = bytes();
  c.word(1234);
  c.out.push(...Buffer.from('col\x00', 'latin1'));
  [2, 0, 1].forEach(c.word);
  [1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0].forEach(c.single);
  [0, 4, 1, 2, 0].forEach(c.word);
  [-20, 0, 0, 20, 0, 0, 20, 20, 0, -20, 20, 0].forEach(c.single);
  c.word(0);
  c.single(400);
  [0, 10, 0, 20, 10, 0.5].forEach(c.single);
  c.half(0x8000);
  c.half(0x8001);
  for (const tri of [[0, 1, 2], [0, 2, 3]]) {
    [0, 0, 0, -1].forEach(c.single);
    tri.forEach(c.half);
  }
  return Buffer.from(c.out);
}
const scene = ['[SceneInfo]', 'SceneName=Web Store Track', 'DefaultStartPosition=512.00,5.00,300.00', 'DefaultStartDirection=0.00,0.00,1.00',
  '[Environment]', 'TerrainFile=MDFLAT.trn', 'TerrainWidth=1', 'TerrainBreadth=1', 'TerrainWidthScale=4.00', 'TerrainBreadthScale=4.00',
  'TerrainBorderWidth=0.00', 'MinDivisionSize=16.00', '[Fog]', 'ColorRGB=150,151,139', '[CollisionObjects]', 'NumberOfCollisionObjects=1',
  '[Object1]', 'COL=mdwall.col', 'Position=700.00,0.00,500.00', ''].join('\r\n');
const files = [['mod.json', Buffer.from(modJson)], ['Env/MDFLAT/MDFLAT.scn', Buffer.from(scene)], ['Env/MDFLAT/MDFLAT.ps2.trn', flatTerrain()],
  ['Col/mdwall.col', wallCol()], ['filler.dat', filler], ['previews/0.png', png(16, 9)]];
// A 16x9 RGB PNG of a gradient: a store zip's picture by another name than previews/track.png, which the browser's
// pick shows on the track's card (P261).
function png(w, h) {
  const chunk = (kind, body) => {
    const len = Buffer.alloc(4), crc = Buffer.alloc(4), k = Buffer.from(kind);
    len.writeUInt32BE(body.length);
    crc.writeUInt32BE(zlib.crc32(Buffer.concat([k, body])));
    return Buffer.concat([len, k, body, crc]);
  };
  const head = Buffer.alloc(13);
  head.writeUInt32BE(w, 0);
  head.writeUInt32BE(h, 4);
  head[8] = 8;
  head[9] = 2;
  const rows = [];
  for (let y = 0; y < h; y++) {
    rows.push(0);
    for (let x = 0; x < w; x++) rows.push(x * 16, y * 28, 128);
  }
  return Buffer.concat([Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]), chunk('IHDR', head),
    chunk('IDAT', zlib.deflateSync(Buffer.from(rows))), chunk('IEND', Buffer.alloc(0))]);
}
function zip(entries) {
  const out = [];
  const dir = [];
  let at = 0;
  for (const [name, data] of entries) {
    const packed = zlib.deflateRawSync(data);
    const crc = zlib.crc32(data);
    const n = Buffer.from(name);
    const local = Buffer.alloc(30);
    local.writeUInt32LE(0x04034b50, 0);
    local.writeUInt16LE(20, 4);
    local.writeUInt16LE(8, 8);
    local.writeUInt32LE(crc, 14);
    local.writeUInt32LE(packed.length, 18);
    local.writeUInt32LE(data.length, 22);
    local.writeUInt16LE(n.length, 26);
    const central = Buffer.alloc(46);
    central.writeUInt32LE(0x02014b50, 0);
    central.writeUInt16LE((3 << 8) | 20, 4);
    central.writeUInt16LE(20, 6);
    central.writeUInt16LE(8, 10);
    central.writeUInt32LE(crc, 16);
    central.writeUInt32LE(packed.length, 20);
    central.writeUInt32LE(data.length, 24);
    central.writeUInt16LE(n.length, 28);
    central.writeUInt32LE((0o100644 << 16) >>> 0, 38);
    central.writeUInt32LE(at, 42);
    out.push(local, n, packed);
    dir.push(central, n);
    at += local.length + n.length + packed.length;
  }
  const d = Buffer.concat(dir);
  const end = Buffer.alloc(22);
  end.writeUInt32LE(0x06054b50, 0);
  end.writeUInt16LE(entries.length, 8);
  end.writeUInt16LE(entries.length, 10);
  end.writeUInt32LE(d.length, 12);
  end.writeUInt32LE(at, 16);
  return Buffer.concat([...out, d, end]);
}
const modZip = zip(files);
const sha = crypto.createHash('sha256').update(modZip).digest('hex');
const meta = { id: ID, kind: 'track', name: 'Web Store Track', author: 'someone', source: 'MX vs. ATV Unleashed', version: '1.0.0',
  size: modZip.length, sha256: sha, standard: 1 };
const store = new Map([
  [`/api/datasets/${REPO}/tree/main/mods`, JSON.stringify([{ type: 'directory', path: `mods/${ID}` }])],
  [`/api/datasets/${REPO}/discussions?type=pull_request&status=open&p=0`, JSON.stringify({ discussions: [], count: 0 })],
  [`/api/datasets/${REPO}/refs?include_prs=1`, JSON.stringify({ pullRequests: [] })],
  [`/datasets/${REPO}/resolve/main/mods/${ID}/meta.json`, JSON.stringify(meta)],
  [`/cdn/datasets/${REPO}/resolve/main/mods/${ID}/mod.zip`, modZip],
]);
let zips = 0;
let ranges = 0;

const types = { '.html': 'text/html', '.js': 'text/javascript', '.wasm': 'application/wasm', '.svg': 'image/svg+xml',
  '.webmanifest': 'application/manifest+json', '.ttf': 'font/ttf' };
const server = http.createServer((req, res) => {
  const u = new URL(req.url, 'http://x');
  if (u.pathname.startsWith('/store/')) {
    const p = req.url.slice('/store'.length);
    if (process.env.OPENMX_STORE_LOG) console.error('store:', p);
    if (p.endsWith('/mod.zip')) {
      zips++;
      if (req.headers.range) ranges++;
    }
    if (p.endsWith('/mod.zip') && !p.startsWith('/cdn/')) {
      res.writeHead(302, { location: `/store/cdn${p}` });
      res.end();
      return;
    }
    const b = store.get(p);
    res.writeHead(b ? 200 : 404, { 'content-type': 'application/octet-stream' });
    res.end(b ?? '{"error":"not found"}');
    return;
  }
  let p = decodeURIComponent(u.pathname);
  if (p.endsWith('/')) p += 'index.html';
  const f = p.startsWith('/pkg/') ? path.join(pkg, path.normalize(p.slice(5))) : path.join(webdir, path.normalize(p));
  fs.readFile(f, (e, b) => {
    if (e) {
      res.writeHead(404);
      res.end();
      return;
    }
    // the picked disc read where it is, as gpu.mjs's: OPFS is still the store's, but the disc is not copied into it
    if (p === '/index.html') b = Buffer.from(String(b).replace('opfs = true;', 'opfs = false;'));
    res.writeHead(200, { 'content-type': types[path.extname(f)] ?? 'application/octet-stream' });
    res.end(b);
  });
}).listen(0, '127.0.0.1');
await new Promise(r => server.on('listening', r));
const origin = `http://127.0.0.1:${server.address().port}`;
// the software picture: the menu is all this needs, and no GPU fallback reloads the page to ask for the disc again
const url = `${origin}/?software&args=${encodeURIComponent(`--screen store --mod-store ${origin}/store`)}`;

const chrome = spawn('chromium', ['--headless=new', '--remote-debugging-port=0', `--user-data-dir=${profile}`,
  '--no-first-run', '--no-default-browser-check', '--window-size=1280,720', 'about:blank'],
  { stdio: ['ignore', 'ignore', 'pipe'] });
const fail = why => {
  console.error(why);
  chrome.kill();
  process.exit(1);
};
const wsUrl = await new Promise(resolve => {
  let buf = '';
  const late = setTimeout(() => fail(`chromium gave no DevTools address:\n${buf}`), 20000);
  chrome.stderr.on('data', d => {
    buf += d;
    const m = buf.match(/DevTools listening on (ws:\S+)/);
    if (m) {
      clearTimeout(late);
      resolve(m[1]);
    }
  });
});
const targets = await (await fetch(wsUrl.replace('ws://', 'http://').replace(/\/devtools.*/, '/json'))).json();
const ws = new WebSocket(targets.find(t => t.type === 'page').webSocketDebuggerUrl);
await new Promise(r => ws.addEventListener('open', r));
let id = 0;
const pending = new Map();
// what the page and its worker said, in order
const logs = [];
ws.addEventListener('message', ({ data }) => {
  const m = JSON.parse(data);
  if (m.id && pending.has(m.id)) {
    pending.get(m.id)(m);
    pending.delete(m.id);
  } else if (m.method === 'Runtime.exceptionThrown') console.error('page:', m.params.exceptionDetails.text);
  else if (m.method === 'Target.attachedToTarget') send('Runtime.enable', {}, m.params.sessionId);
  else if (m.method === 'Runtime.consoleAPICalled') {
    const line = m.params.args.map(a => a.value ?? a.description).join(' ');
    logs.push(line);
    console.error('console:', line);
  }
});
const send = (method, params = {}, sessionId) => new Promise(r => {
  const i = ++id;
  pending.set(i, r);
  ws.send(JSON.stringify({ id: i, method, params, ...(sessionId ? { sessionId } : {}) }));
});
const evaluate = async expression =>
  (await send('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true })).result?.result?.value;
const data = async () => (await evaluate('({ ...document.body.dataset })')) ?? {};
const until = async (ok, ms) => {
  const t = Date.now();
  for (;;) {
    const d = await data();
    if (ok(d) || Date.now() - t > ms) return d;
    await new Promise(r => setTimeout(r, 100));
  }
};
// OPFS read from the page, the worker's origin: every file under `top`, by path, and the store.json's sha256
const opfs = top => evaluate(`(async () => {
  const out = [];
  let stamp = null;
  const walk = async (dir, at) => {
    for await (const [name, h] of dir.entries()) {
      const p = at ? at + '/' + name : name;
      if (h.kind === 'directory') await walk(h, p);
      else {
        out.push(p);
        if (name === 'store.json') stamp = JSON.parse(await (await h.getFile()).text()).sha256 ?? null;
      }
    }
  };
  try {
    await walk(await (await navigator.storage.getDirectory()).getDirectoryHandle(${JSON.stringify(top)}), '');
  } catch {}
  return { files: out.sort(), stamp };
})()`);

await send('Runtime.enable');
await send('Page.enable');
// No pad of the machine's reaches the page: another slot's `./kit pad` (or a pad in a hand) pressing a button shows
// itself to every chromium here, and the page then hides the touch pad and reads its presses as the rider's (K13).
await send('Page.addScriptToEvaluateOnNewDocument', { source: 'navigator.getGamepads = () => [];' });
await send('Target.setAutoAttach', { autoAttach: true, waitForDebuggerOnStart: false, flatten: true });
// The page at `to`, the disc picked (it is read in place, so every visit asks for it), until the game starts.
async function open(to) {
  await send('Page.navigate', { url: to });
  let d = await until(d => ['asking', 'failed'].includes(d.state), 120000);
  if (d.state !== 'asking') fail(`the page did not ask for the disc: ${JSON.stringify(d)}`);
  const doc = await send('DOM.getDocument');
  const pick = await send('DOM.querySelector', { nodeId: doc.result.root.nodeId, selector: '#pick' });
  await send('DOM.setFileInputFiles', { files: [iso], nodeId: pick.result.nodeId });
  d = await until(d => d.state === 'started' || d.state === 'failed', 120000);
  if (d.state !== 'started') fail(`the page did not start: ${JSON.stringify(d)}`);
  return d;
}
let d = await open(url);

// Space (the keyboard's Cross; Return is Start, Refresh) on the lit card, the store's only one, until the mod is in: a press before the listing is in lights
// nothing, and one while the install runs (or after) does nothing more, so pressing again is safe
const key = type => send('Input.dispatchKeyEvent', { type, key: ' ', code: 'Space', windowsVirtualKeyCode: 32 });
let presses = 0;
let got = { files: [], stamp: null };
for (const end = Date.now() + 120000; Date.now() < end && !got.stamp; ) {
  if (presses < 40) {
    await key('keyDown');
    await new Promise(r => setTimeout(r, 80));
    await key('keyUp');
    presses++;
  }
  await new Promise(r => setTimeout(r, 2000));
  got = await opfs('mods');
}
// the download's file closes and goes once the install is done
let left = [];
for (const end = Date.now() + 10000; Date.now() < end; ) {
  left = (await opfs('store')).files;
  if (!left.length) break;
  await new Promise(r => setTimeout(r, 200));
}
d = await data();
// The install read the mods folder again (P217): Free Ride's list has the track, out of OPFS. Since P293 an install
// waits for the player's word, the Reload / Later popup with Reload lit, so Space answers it; pressed before the popup
// is up it lands on the store's installed card, which does nothing more.
const said = prefix => logs.find(l => l.startsWith(prefix)) ?? null;
for (const end = Date.now() + 20000; Date.now() < end && !said('mods: 1 tracks'); ) {
  await key('keyDown');
  await new Promise(r => setTimeout(r, 80));
  await key('keyUp');
  await new Promise(r => setTimeout(r, 1000));
}
const listed = said('mods: 1 tracks');
// and its card's picture, the zip's previews/0.png out of OPFS (P261)
for (const end = Date.now() + 10000; Date.now() < end && !said('mods: pictures of'); ) await new Promise(r => setTimeout(r, 200));
const pictured = said('mods: pictures of');

// And the page rides it: opened again on the installed track, which only OPFS `mods/` has. A ride that does not load
// is the shell's error, and the page's state `failed`; one that does draws its frames.
const ridden = await open(`${origin}/?software&args=${encodeURIComponent(`--track ${ID}`)}`);
let ride = ridden;
for (const end = Date.now() + 60000; Date.now() < end && !said(`mods: ${ID} `) && ride.state === 'started'; ) {
  await new Promise(r => setTimeout(r, 200));
  ride = await data();
}
const rode = said(`mods: ${ID} `);
// its pictures over the next seconds, as the worker counts them
let frames = 0;
for (let i = 0; i < 5 && ride.state === 'started'; i++) {
  await new Promise(r => setTimeout(r, 1000));
  ride = await data();
  frames += Number(ride.fps ?? 0);
}
if (process.env.OPENMX_STORE_SHOT)
  fs.writeFileSync(process.env.OPENMX_STORE_SHOT, Buffer.from((await send('Page.captureScreenshot', { format: 'png' })).result.data, 'base64'));
console.log(JSON.stringify({ state: d.state, files: got.files, stamp: got.stamp, sha, left, zips, ranges, presses, listed, pictured, rode,
  ride: ride.state, frames: Math.round(frames) }));
chrome.kill();
await new Promise(r => chrome.on('exit', r));
server.close();
process.exit(0);
