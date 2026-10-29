// Offline play: the page, its worker and the game module are kept in the browser's cache and served from it when
// the network is not there. Network first, so a new build is what an online visit gets (the module by its build,
// below). The disc is in OPFS, never in this cache: nothing from the disc is hosted or fetched. The game module is
// not fetched ahead (P194: a first visit loads the page alone, the module only on Ride); the fetch below keeps it
// once a ride has loaded it.
const CACHE = 'openmx';
const SHELL = ['./', 'worker.js', 'manifest.webmanifest', 'icon.svg', 'icon-180.png', 'touch.js', 'fonts/Saira.ttf'];
// The game module (P205, opentennis's B470): its glue and wasm, kept a build to a cache named for the build
// (version.txt, tools/web.sh) and served from it first, so a ride after the first starts without fetching the
// module again — megabytes, on a phone's network — and the glue and the wasm are always one build's. A build
// version.txt does not name (a local one with uncommitted changes, `+`, or none served) goes network first as the rest.
const PKG = 'openmx-pkg ';
const PAIR = ['pkg/openmx_web.js', 'pkg/openmx_web_bg.wasm'];

self.addEventListener('install', e => {
  e.waitUntil(caches.open(CACHE).then(c => c.addAll(SHELL)).then(() => self.skipWaiting()));
});
self.addEventListener('activate', e => e.waitUntil(self.clients.claim()));
self.addEventListener('fetch', e => {
  const url = new URL(e.request.url);
  if (e.request.method !== 'GET' || url.origin !== location.origin) return;
  e.respondWith(PAIR.some(p => url.pathname.endsWith(`/${p}`)) ? module(e.request) : fresh(e.request));
});

function fresh(request) {
  return fetch(request)
    .then(r => {
      if (r.ok) {
        const copy = r.clone();
        caches.open(CACHE).then(c => c.put(request, copy));
      }
      return r;
    })
    .catch(() => kept(request));
}

// Offline: a whole build's pair first, so the glue and the wasm agree; else whatever was kept.
async function kept(request) {
  for (const name of await caches.keys()) {
    if (!name.startsWith(PKG)) continue;
    const r = await (await caches.open(name)).match(request);
    if (r) return r;
  }
  return (await caches.match(request)) ?? Response.error();
}

async function module(request) {
  const build = await fetch('version.txt', { cache: 'no-store' })
    .then(r => (r.ok ? r.text() : ''))
    .then(t => t.trim())
    .catch(() => '');
  if (!build || build.endsWith('+')) return fresh(request);
  const cache = await caches.open(PKG + build);
  const hit = await cache.match(request);
  if (hit) return hit;
  const r = await fetch(request).catch(() => null);
  if (!r) return kept(request);
  if (r.ok) {
    await cache.put(request, r.clone());
    // The pair whole: the builds before it go.
    const whole = await Promise.all(PAIR.map(p => cache.match(new URL(p, location.href))));
    if (whole.every(Boolean))
      for (const name of await caches.keys()) if (name.startsWith(PKG) && name !== PKG + build) await caches.delete(name);
  }
  return r;
}
