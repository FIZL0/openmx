// Offline play: the page, its worker and the game module are kept in the browser's cache and served from it when
// the network is not there. Network first, so a new build is what an online visit gets. The disc is in OPFS, never in
// this cache: nothing from the disc is hosted or fetched. The game module is not fetched ahead (P194: a first visit
// loads the page alone, the module only on Ride); the fetch below keeps it once a ride has loaded it.
const CACHE = 'openmx';
const SHELL = ['./', 'worker.js', 'manifest.webmanifest', 'icon.svg', 'icon-180.png', 'touch.js', 'fonts/MPLUSRounded1c-Bold.ttf',
  'fonts/MPLUSRounded1c-ExtraBold.ttf'];

self.addEventListener('install', e => {
  e.waitUntil(caches.open(CACHE).then(c => c.addAll(SHELL)).then(() => self.skipWaiting()));
});
self.addEventListener('activate', e => e.waitUntil(self.clients.claim()));
self.addEventListener('fetch', e => {
  if (e.request.method !== 'GET' || new URL(e.request.url).origin !== location.origin) return;
  e.respondWith(
    fetch(e.request)
      .then(r => {
        if (r.ok) {
          const copy = r.clone();
          caches.open(CACHE).then(c => c.put(e.request, copy));
        }
        return r;
      })
      .catch(() => caches.match(e.request).then(r => r ?? Response.error())),
  );
});
