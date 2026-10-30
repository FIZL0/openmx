// The launcher's two looks in headless chromium (P181; crates/openmx/tests/web.rs, `the_launcher_fits_a_phone_and_a_desktop`):
// web/ served on loopback and opened at a desktop's width and a phone's, first with nothing stored (the landing page,
// `asking`), then with a stand-in `disc.iso` written into OPFS (Play, `menu`). Needs no disc and no game
// module: neither look starts the game, and the stand-in is never read.
//
//   node shots.mjs <web dir> <out dir> <scratch profile dir> [width...]
//
// Writes <state>-<width>.png (the whole page) into <out dir> and prints one JSON line:
// { shots: [{ state, width, scroll, fonts, lit, file }...] }, `scroll` the page's scrollWidth (wider than `width` is a
// page that pans sideways), `fonts` whether Saira loaded, `lit` the Play button's text while it shows (the menu only).
import http from 'node:http';
import fs from 'node:fs';
import path from 'node:path';
import { spawn } from 'node:child_process';

const [webdir, out, profile, ...given] = process.argv.slice(2);
const widths = given.length ? given.map(Number) : [1440, 390];
const types = { '.html': 'text/html', '.js': 'text/javascript', '.wasm': 'application/wasm', '.svg': 'image/svg+xml',
  '.webmanifest': 'application/manifest+json', '.ttf': 'font/ttf' };
const server = http.createServer((req, res) => {
  let p = decodeURIComponent(new URL(req.url, 'http://x').pathname);
  if (p.endsWith('/')) p += 'index.html';
  fs.readFile(path.join(webdir, path.normalize(p)), (e, b) => {
    if (e) {
      res.writeHead(404);
      res.end();
      return;
    }
    res.writeHead(200, { 'content-type': types[path.extname(p)] ?? 'application/octet-stream' });
    res.end(b);
  });
}).listen(0, '127.0.0.1');
await new Promise(r => server.on('listening', r));
const url = `http://127.0.0.1:${server.address().port}/`;
fs.mkdirSync(out, { recursive: true });

const chrome = spawn('chromium', ['--headless=new', '--remote-debugging-port=0', `--user-data-dir=${profile}`,
  '--no-first-run', '--no-default-browser-check', '--hide-scrollbars', 'about:blank'], { stdio: ['ignore', 'ignore', 'pipe'] });
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
ws.addEventListener('message', ({ data }) => {
  const m = JSON.parse(data);
  if (m.id && pending.has(m.id)) {
    pending.get(m.id)(m);
    pending.delete(m.id);
  } else if (m.method === 'Runtime.exceptionThrown') console.error('page:', JSON.stringify(m.params.exceptionDetails));
});
const send = (method, params = {}) => new Promise(r => {
  const i = ++id;
  pending.set(i, r);
  ws.send(JSON.stringify({ id: i, method, params }));
});
const evaluate = async expression =>
  (await send('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true })).result?.result?.value;
const state = () => evaluate('document.body.dataset.state ?? ""');
const until = async (want, ms) => {
  const t = Date.now();
  for (;;) {
    const s = await state();
    if (want.includes(s) || Date.now() - t > ms) return s;
    await new Promise(r => setTimeout(r, 100));
  }
};
const visit = async want => {
  await send('Page.navigate', { url });
  const s = await until([want, 'failed', 'started'], 20000);
  if (s !== want) fail(`wanted ${want}, the page says ${s}`);
  await evaluate('document.fonts.ready.then(() => new Promise(r => setTimeout(r, 300)))');
};

await send('Runtime.enable');
await send('Page.enable');
// No pad of the machine's reaches the page: another slot's `./kit pad` (or a pad in a hand) pressing a button shows
// itself to every chromium here, and the page then hides the touch pad and reads its presses as the rider's (K13).
await send('Page.addScriptToEvaluateOnNewDocument', { source: 'navigator.getGamepads = () => [];' });
const shots = [];
const clearOpfs = `navigator.storage.getDirectory().then(async d => { for await (const k of d.keys()) await d.removeEntry(k); })`;
for (const width of widths) {
  const phone = width < 700;
  await send('Emulation.setDeviceMetricsOverride', { width, height: phone ? 844 : 900, deviceScaleFactor: 1, mobile: phone });
  for (const want of ['asking', 'menu']) {
    await send('Page.navigate', { url: 'about:blank' });
    await send('Page.navigate', { url });
    await until(['asking', 'menu'], 20000);
    await evaluate(want === 'menu'
      ? `${clearOpfs}.then(() => navigator.storage.getDirectory()).then(d => d.getFileHandle('disc.iso', { create: true }))
          .then(h => h.createWritable()).then(async w => { await w.write(new Uint8Array(1 << 16)); await w.close(); })`
      : clearOpfs);
    await visit(want);
    const look = await evaluate(`({ scroll: document.scrollingElement.scrollWidth, fonts: document.fonts.check('800 16px OMX'),
      lit: (b => (b?.offsetParent ? b.textContent : null))(document.querySelector('#ride')), height: document.scrollingElement.scrollHeight })`);
    const png = await send('Page.captureScreenshot', { format: 'png', captureBeyondViewport: true,
      clip: { x: 0, y: 0, width, height: look.height, scale: 1 } });
    const file = path.join(out, `${want}-${width}.png`);
    fs.writeFileSync(file, Buffer.from(png.result.data, 'base64'));
    shots.push({ state: want, width, scroll: look.scroll, fonts: look.fonts, lit: look.lit, file });
  }
}
await evaluate(clearOpfs);
console.log(JSON.stringify({ shots }));
chrome.kill();
await new Promise(r => chrome.on('exit', r));
server.close();
process.exit(0);
