// The GPU in the page (P48; crates/openmx/tests/web.rs, `the_browser_rides_on_the_gpu`): web/ served on loopback,
// opened in chromium with OPFS refused (so the picked disc is read in place, never copied) and `?args=--ride`, so the
// shell rides the start line at once; then the page is watched for SECONDS while it rides.
//
//   node gpu.mjs <web dir> <disc.iso> <scratch profile dir> <out.png> [query] [chromium flag...]
//
// `query` is added to the page's address (`webgl2`, `software`). With OPENMX_GPU_PKG set to a directory, the page's
// pkg/ is served from there (the test's `tools/web.sh --check` module) instead of web/pkg. With OPENMX_GPU_PHONE set
// to `width,height,ratio` the page is a phone's (P205): that viewport in CSS pixels at that device-pixel ratio, with
// touch. Prints one JSON line:
// { state, gpu, drawn, fps: [per second...], lit, pixels, png } — `gpu` what the page says draws (worker.js's `gpu`),
// `drawn` its first GPU frame, `fps` the page's pictures a second as worker.js counts them, and `lit` how many of the
// last screenshot's `pixels` are not black. The screenshot is written to <out.png>.
import http from 'node:http';
import fs from 'node:fs';
import path from 'node:path';
import { spawn } from 'node:child_process';

const [webdir, iso, profile, out, query = '', ...flags] = process.argv.slice(2);
const SECONDS = Number(process.env.OPENMX_GPU_SECONDS ?? 20);
const PKG = process.env.OPENMX_GPU_PKG;
const PHONE = process.env.OPENMX_GPU_PHONE;
const types = { '.html': 'text/html', '.js': 'text/javascript', '.wasm': 'application/wasm', '.svg': 'image/svg+xml',
  '.webmanifest': 'application/manifest+json', '.ttf': 'font/ttf' };
const server = http.createServer((req, res) => {
  let p = decodeURIComponent(new URL(req.url, 'http://x').pathname);
  if (p.endsWith('/')) p += 'index.html';
  const f = PKG && p.startsWith('/pkg/') ? path.join(PKG, path.normalize(p.slice(5))) : path.join(webdir, path.normalize(p));
  fs.readFile(f, (e, b) => {
    if (e) {
      res.writeHead(404);
      res.end();
      return;
    }
    // OPFS refused, as chromium.mjs's --no-opfs: the picked file is read where it is, and nothing is copied
    if (p === '/index.html') b = Buffer.from(String(b).replace('opfs = true;', 'opfs = false;'));
    res.writeHead(200, { 'content-type': types[path.extname(f)] ?? 'application/octet-stream' });
    res.end(b);
  });
}).listen(0, '127.0.0.1');
await new Promise(r => server.on('listening', r));
const url = `http://127.0.0.1:${server.address().port}/?args=--ride${query ? `&${query}` : ''}`;

// No extensions, as page.mjs (K7, K12): this machine's chromium brings its own into every profile, the policy's
// force-installed one fetched from the web store a few seconds in, and one arriving mid-load can abort the worker's
// module fetch; under a busy gate the WebGPU ride failed before it chose a GPU ('{"disc":"no","state":"failed"}').
const chrome = spawn('chromium', ['--headless=new', '--remote-debugging-port=0', `--user-data-dir=${profile}`,
  '--no-first-run', '--no-default-browser-check', '--disable-extensions', '--disable-component-extensions-with-background-pages',
  '--disable-background-networking', '--window-size=1280,720', ...flags, 'about:blank'],
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
ws.addEventListener('message', ({ data }) => {
  const m = JSON.parse(data);
  if (m.id && pending.has(m.id)) {
    pending.get(m.id)(m);
    pending.delete(m.id);
  } else if (m.method === 'Runtime.exceptionThrown') console.error('page:', m.params.exceptionDetails.text);
  else if (m.method === 'Target.attachedToTarget') send('Runtime.enable', {}, m.params.sessionId);
  else if (m.method === 'Runtime.consoleAPICalled') console.error('console:', m.params.args.map(a => a.value ?? a.description).join(' '));
});
const send = (method, params = {}, sessionId) => new Promise(r => {
  const i = ++id;
  pending.set(i, r);
  ws.send(JSON.stringify({ id: i, method, params, ...(sessionId ? { sessionId } : {}) }));
});
const evaluate = async expression =>
  (await send('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true })).result?.result?.value;
// mid-navigation (the first load, a fallback's reload) there is no page to ask: nothing is known yet
const data = async () => (await evaluate('({ ...document.body.dataset })')) ?? {};
const until = async (ok, ms) => {
  const t = Date.now();
  for (;;) {
    const d = await data();
    if (ok(d) || Date.now() - t > ms) return d;
    await new Promise(r => setTimeout(r, 100));
  }
};

await send('Runtime.enable');
await send('Page.enable');
// No pad of the machine's reaches the page: another slot's `./kit pad` (or a pad in a hand) pressing a button shows
// itself to every chromium here, and the page then hides the touch pad and reads its presses as the rider's (K13).
await send('Page.addScriptToEvaluateOnNewDocument', { source: 'navigator.getGamepads = () => [];' });
// the worker's console too: what the module says (its renderer, its adapter) is how a failure explains itself
await send('Target.setAutoAttach', { autoAttach: true, waitForDebuggerOnStart: false, flatten: true });
if (PHONE) {
  const [width, height, deviceScaleFactor] = PHONE.split(',').map(Number);
  await send('Emulation.setDeviceMetricsOverride', { width, height, deviceScaleFactor, mobile: true });
  await send('Emulation.setTouchEmulationEnabled', { enabled: true });
}
await send('Page.navigate', { url });
// a fallback reloads the page with a new query, which asks for the disc again: pick it each time it asks
let d;
for (let picks = 0; picks < 3; picks++) {
  d = await until(d => ['asking', 'failed'].includes(d.state) || d.drawn || (d.state === 'started' && d.gpu === 'software'), 120000);
  if (d.state !== 'asking') break;
  const doc = await send('DOM.getDocument');
  const pick = await send('DOM.querySelector', { nodeId: doc.result.root.nodeId, selector: '#pick' });
  await send('DOM.setFileInputFiles', { files: [iso], nodeId: pick.result.nodeId });
  await until(d => d.state !== 'asking', 20000);
}
// with what the page said (its #status holds the worker's error), so a red run says why
if (d.state === 'failed' || !(d.drawn || d.gpu === 'software'))
  fail(`the page did not start drawing: ${JSON.stringify(d)} (${await evaluate("document.querySelector('#status')?.textContent")})`);

// the ride: its pictures a second, as the worker counts them, each second for SECONDS
const fps = [];
for (let s = 0; s < SECONDS; s++) {
  await new Promise(r => setTimeout(r, 1000));
  d = await data();
  fps.push(Number(d.fps ?? 0));
}
const png = (await send('Page.captureScreenshot', { format: 'png' })).result.data;
fs.writeFileSync(out, Buffer.from(png, 'base64'));
// the screenshot's pixels read back in the page: how many are not black
const [lit, pixels] = await evaluate(`(async () => {
  const b = await (await fetch('data:image/png;base64,${png}')).blob();
  const i = await createImageBitmap(b);
  const c = new OffscreenCanvas(i.width, i.height);
  const g = c.getContext('2d');
  g.drawImage(i, 0, 0);
  const p = g.getImageData(0, 0, i.width, i.height).data;
  let lit = 0;
  for (let k = 0; k < p.length; k += 4) if (p[k] + p[k + 1] + p[k + 2] > 24) lit++;
  return [lit, i.width * i.height];
})()`);
console.log(JSON.stringify({ state: d.state, gpu: d.gpu, drawn: d.drawn, fps, lit, pixels, png: out }));
chrome.kill();
await new Promise(r => chrome.on('exit', r));
server.close();
process.exit(0);
