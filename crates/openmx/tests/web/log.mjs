// The page log for bug reports (P257b; crates/openmx/tests/web.rs, `the_page_log_keeps_the_worker_and_the_boot_error`):
// web/ served on loopback, opened in headless chromium with `?fps` and OPFS refused, and a disc that is no disc picked:
// the game module starts in the worker, says what it reads, and the shell ends on the disc, which the page shows on its
// failed screen. The page is then reloaded (as a fallback's restart onto `?webgl2` does), and the new page's Save log
// button pressed; chromium's download is the log.
//
//   node log.mjs <web dir> <not a disc> <scratch profile dir> <download dir> [chromium flag...]
//
// With OPENMX_LOG_PKG set to a directory, the page's pkg/ is served from there (the test's `tools/web.sh --check`
// module) instead of web/pkg. Prints one JSON line: { state, status, button, file, log } — the page's state and
// #status on its failed screen, whether the Save log button was there after the reload, the downloaded file's name
// and its text.
import http from 'node:http';
import fs from 'node:fs';
import path from 'node:path';
import { spawn } from 'node:child_process';

const [webdir, junk, profile, downloads, ...flags] = process.argv.slice(2);
const PKG = process.env.OPENMX_LOG_PKG;
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
    // OPFS refused, as gpu.mjs: the picked file is read where it is, and nothing is copied
    if (p === '/index.html') b = Buffer.from(String(b).replace('opfs = true;', 'opfs = false;'));
    res.writeHead(200, { 'content-type': types[path.extname(f)] ?? 'application/octet-stream' });
    res.end(b);
  });
}).listen(0, '127.0.0.1');
await new Promise(r => server.on('listening', r));
const url = `http://127.0.0.1:${server.address().port}/?fps&software`;

// No extensions, as gpu.mjs (K7, K12).
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
// The browser's own target, for where downloads go, and the page's.
const connect = async address => {
  const ws = new WebSocket(address);
  await new Promise(r => ws.addEventListener('open', r));
  let id = 0;
  const pending = new Map();
  ws.addEventListener('message', ({ data }) => {
    const m = JSON.parse(data);
    if (m.id && pending.has(m.id)) {
      pending.get(m.id)(m);
      pending.delete(m.id);
    } else if (m.method === 'Runtime.exceptionThrown') console.error('page:', m.params.exceptionDetails.text);
    else if (m.method === 'Runtime.consoleAPICalled') console.error('console:', m.params.args.map(a => a.value ?? a.description).join(' '));
  });
  return (method, params = {}) => new Promise(r => {
    const i = ++id;
    pending.set(i, r);
    ws.send(JSON.stringify({ id: i, method, params }));
  });
};
const browser = await connect(wsUrl);
fs.mkdirSync(downloads, { recursive: true });
await browser('Browser.setDownloadBehavior', { behavior: 'allow', downloadPath: downloads });
const targets = await (await fetch(wsUrl.replace('ws://', 'http://').replace(/\/devtools.*/, '/json'))).json();
const send = await connect(targets.find(t => t.type === 'page').webSocketDebuggerUrl);
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

await send('Runtime.enable');
await send('Page.enable');
await send('Page.addScriptToEvaluateOnNewDocument', { source: 'navigator.getGamepads = () => [];' });
await send('Page.navigate', { url });
let d = await until(d => d.state === 'asking', 60000);
if (d.state !== 'asking') fail(`the page never asked for a disc: ${JSON.stringify(d)}`);
const doc = await send('DOM.getDocument');
const pick = await send('DOM.querySelector', { nodeId: doc.result.root.nodeId, selector: '#pick' });
await send('DOM.setFileInputFiles', { files: [junk], nodeId: pick.result.nodeId });
d = await until(d => d.state === 'failed', 120000);
const status = await evaluate("document.querySelector('#status')?.textContent");
if (d.state !== 'failed') fail(`the page did not fail on a disc that is no disc: ${JSON.stringify(d)} (${status})`);

// the restart, then Save log on the new page
await send('Page.reload');
await new Promise(r => setTimeout(r, 500));
await until(d => d.state === 'asking', 60000);
const button = await evaluate("!!document.querySelector('#savelog')");
await evaluate("document.querySelector('#savelog')?.click()");
let file = '';
for (const t = Date.now(); Date.now() - t < 20000 && !file; await new Promise(r => setTimeout(r, 100)))
  file = fs.readdirSync(downloads).find(f => !f.endsWith('.crdownload')) ?? '';
const log = file ? fs.readFileSync(path.join(downloads, file), 'utf8') : '';
console.log(JSON.stringify({ state: d.state, status, button, file, log }));
chrome.kill();
await new Promise(r => chrome.on('exit', r));
server.close();
process.exit(0);
