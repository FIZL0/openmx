// On-screen controls for phones and tablets (P197), and the page's other mobile bits; opentennis's web/touch.js, cut
// down to one pad and laid out for a bike. The controls are a virtual pad of the browser's standard mapping added to
// `navigator.getGamepads()`, which index.html hands the worker every frame like any controller, so the shell reads them
// through the same pad layer and bindings as a real one (crates/openmx/src/pad.rs, the new layout, the default):
//   the stick       axes 0/1    steer and lean (and the menus' cursor)
//   GAS / BRAKE     R2 / L2     the throttle and the brake, full travel while held
//   ✕ ○ □ △         0 1 2 3     OK and back in the menus; ○ △ tricks in a ride (✕ □ the trick pack's extras)
//   CLUTCH / MODIFIER  L1 / R1  the clutch, the third trick button; together the put-back
//   CAMERA / PAUSE  Select / Start
// A rider on the original's layout (Settings) has the gas on ✕ and the brake on □, both on the pad too.
//
// They show once the game has started (`body[data-state="started"]`) on a touch screen, or after any touch, while no
// real controller is connected and no hardware key was the last thing pressed. The menus take taps as clicks: a short
// tap on the stick's area that hardly moves is handed to the canvas as one, so the stick hides no menu item.
// Also here: every AudioContext resumes on the next gesture (iOS starts them suspended and suspends them again after a
// call or another app's sound), a touch screen goes fullscreen on the first tap where the browser allows it (not
// iPhone Safari: there, Add to Home Screen; the manifest and apple-mobile-web-app-capable make that start fullscreen),
// upright the game is covered by a hint to turn the phone, and while riding nothing on the page scrolls, zooms or
// selects.

// A block: a classic script's top-level names would be the window's (`top`, `screen`).
{
// ---- sound: every AudioContext the page makes, resumed on the next gesture ----
const contexts = [];
const AC = window.AudioContext ?? window.webkitAudioContext;
if (AC) {
  window.AudioContext = class extends AC {
    constructor(...a) {
      super(...a);
      contexts.push(this);
    }
  };
}
let primed = false;
const unlockAudio = () => {
  for (const c of contexts) {
    if (c.state !== 'running') c.resume?.().catch(() => {});
    // iOS before 15 only unlocked output for a source started inside the gesture: one silent sample, once
    if (!primed) {
      primed = true;
      try {
        const s = c.createBufferSource();
        s.buffer = c.createBuffer(1, 1, c.sampleRate);
        s.connect(c.destination);
        s.start(0);
      } catch {}
    }
  }
};

const coarse = matchMedia('(pointer: coarse)');
const started = () => document.body?.dataset.state === 'started';
const installed = () => matchMedia('(display-mode: fullscreen), (display-mode: standalone)').matches || navigator.standalone === true;
// iPhone and iPad Safari (an iPad says Macintosh, but has touch points)
const ios = /iPhone|iPad|iPod/.test(navigator.userAgent) || (/Macintosh/.test(navigator.userAgent) && navigator.maxTouchPoints > 1);
const gesture = e => {
  unlockAudio();
  if (!started() || !(coarse.matches || e.type === 'touchend') || document.fullscreenElement || document.webkitFullscreenElement || installed()) return;
  const root = document.documentElement;
  const full = root.requestFullscreen?.({ navigationUI: 'hide' }) ?? root.webkitRequestFullscreen?.();
  // landscape where the browser lets a page hold it (Android in fullscreen); iOS keeps the hint
  Promise.resolve(full).then(() => screen.orientation?.lock?.('landscape')).catch(() => {});
};
for (const e of ['pointerup', 'touchend', 'keydown']) addEventListener(e, gesture, { capture: true });

// ---- the pad ----
// the standard mapping's button indices
const B = { cross: 0, circle: 1, square: 2, triangle: 3, l1: 4, r1: 5, l2: 6, r2: 7, select: 8, start: 9 };
const realPads = navigator.getGamepads?.bind(navigator) ?? (() => []);
const anyReal = () => [...realPads()].some(g => g && g.connected);
// The page's animation frames: a button stays down until a frame after the one a poll first saw it in, so a tap
// shorter than a frame (or one whose down and up arrive together while the page is busy) still counts once.
let tick = 0;
let shown = false;

const css = `
.touch { position: fixed; inset: 0; z-index: 10; pointer-events: none; user-select: none; -webkit-user-select: none;
  -webkit-touch-callout: none; -webkit-tap-highlight-color: transparent;
  --u: min(7vw, 12.5vh); font: 800 calc(var(--u) * .3) var(--face, system-ui, sans-serif); color: #fff;
  --l: env(safe-area-inset-left, 0px); --r: env(safe-area-inset-right, 0px); --t: env(safe-area-inset-top, 0px);
  --b: env(safe-area-inset-bottom, 0px); }
.touch[hidden] { display: none; }
/* the left thumb's area: the stick centres where it lands */
.touch .zone { position: absolute; left: 0; bottom: 0; width: 42%; height: 78%; pointer-events: auto; touch-action: none; }
.touch .base, .touch .knob { position: absolute; border-radius: 50%; transform: translate(-50%, -50%); }
.touch .base { width: calc(var(--u) * 2.6); height: calc(var(--u) * 2.6); background: #fff1; border: 2px solid #fff4; }
.touch .knob { width: calc(var(--u) * 1.2); height: calc(var(--u) * 1.2); background: #fff5; }
.touch .zone:not(.held) .base { opacity: .5; }
.touch button { position: absolute; pointer-events: auto; touch-action: none; border: 2px solid #fff5; background: #0005;
  color: inherit; font: inherit; border-radius: 50%; width: calc(var(--u) * 1.15); height: calc(var(--u) * 1.15); padding: 0;
  display: flex; align-items: center; justify-content: center; line-height: 1; letter-spacing: .02em; }
.touch button.on { background: #fff6; }
/* a touch just past a button's edge still hits it */
.touch button::after { content: ''; position: absolute; inset: calc(var(--u) * -.15); border-radius: inherit; }
.touch button.face { font-size: calc(var(--u) * .5); }
.touch button.pedal { border-radius: calc(var(--u) * .35); }
.touch button.small { border-radius: calc(var(--u) * .3); width: calc(var(--u) * 1.6); height: calc(var(--u) * .7); opacity: .75; }
/* upright on a touch screen: the game is covered by the hint (a tall desktop window keeps its game) */
.turn { display: none; }
@media (orientation: portrait) and (pointer: coarse) {
  html.riding .touch { display: none; }
  html.riding .turn { display: flex; position: fixed; inset: 0; z-index: 11; flex-direction: column; align-items: center;
    justify-content: center; gap: 1em; padding: 2em; text-align: center; background: #000; color: #f5f0e8;
    font: 800 20px/1.4 var(--face, system-ui, sans-serif); }
}
.turn svg { width: 72px; height: 72px; fill: none; stroke: currentColor; stroke-width: 2; }
.turn small { font-weight: 700; font-size: 15px; color: #9c8f80; }
/* while riding nothing scrolls, bounces, zooms or selects */
html.riding, html.riding body { overscroll-behavior: none; overflow: hidden; touch-action: none; user-select: none;
  -webkit-user-select: none; -webkit-touch-callout: none; -webkit-tap-highlight-color: transparent; }
`;
// right(x, y): a control's right and bottom edges, x and y `u` in from the safe area's corner
const right = (x, y) => `right: calc(var(--r) + var(--u) * ${x}); bottom: calc(var(--b) + var(--u) * ${y})`;
// [button, where, class, text]: the pedals under the right thumb, the faces in a diamond over them, the rest along the top
const layout = [
  ['r2', `${right(0.35, 0.35)}; width: calc(var(--u) * 1.9); height: calc(var(--u) * 2.5)`, 'pedal', 'GAS'],
  ['l2', `${right(2.5, 0.35)}; width: calc(var(--u) * 1.6); height: calc(var(--u) * 1.7)`, 'pedal', 'BRAKE'],
  ['cross', right(1.6, 3.1), 'face', '✕'],
  ['circle', right(0.35, 4.2), 'face', '○'],
  ['square', right(2.85, 4.2), 'face', '□'],
  ['triangle', right(1.6, 5.3), 'face', '△'],
  // under the HUD's score bar (its foot is 45.5 of the HUD's 448 high)
  ['l1', `left: calc(var(--l) + var(--u) * .4); top: calc(var(--t) + 12vh)`, 'small', 'CLUTCH'],
  ['select', `left: calc(50% - var(--u) * 2.6); top: calc(var(--t) + var(--u) * .3)`, 'small', 'CAMERA'],
  ['start', `left: calc(50% - var(--u) * .8); top: calc(var(--t) + var(--u) * .3)`, 'small', 'PAUSE'],
  // beside PAUSE, clear of the top-right column where the camera's name comes up
  ['r1', `left: calc(50% + var(--u) * 1.0); top: calc(var(--t) + var(--u) * .3); width: calc(var(--u) * 2.1)`, 'small', 'MODIFIER'],
];
const style = document.createElement('style');
style.textContent = css;
document.head.append(style);

const buttons = Array.from({ length: 17 }, () => ({ pressed: false, touched: false, value: 0 }));
const pad = {
  // the name the shell lists it by (on the web it knows a pad by its name only, web_host.rs `pad_name`)
  id: 'Touch controls',
  index: 16, // past the browser's own slots
  connected: true,
  mapping: 'standard',
  timestamp: 0,
  axes: [0, 0, 0, 0],
  buttons,
  vibrationActuator: null,
};
const seenAt = {}; // button -> the frame a poll first saw it down
const held = Object.fromEntries(Object.keys(B).map(k => [k, new Set()])); // button -> the fingers on it
const root = document.createElement('div');
root.className = 'touch';
root.id = 'touch';
root.hidden = true;
root.innerHTML = `<div class="zone"><div class="base"></div><div class="knob"></div></div>`
  + layout.map(([k, pos, cls, text]) => `<button data-b="${k}" class="${cls}" style="${pos}" aria-label="${k}">${text}</button>`).join('');
const turn = document.createElement('div');
turn.className = 'turn';
turn.innerHTML = `<svg viewBox="0 0 48 48" aria-hidden="true"><rect x="16" y="6" width="16" height="28" rx="3"/><path d="M8 30a16 16 0 0 0 16 12m0 0-4-3m4 3-3 4"/></svg>
  <span>Turn your phone sideways to ride.</span>${ios && !installed() ? '<small>For full screen: Share, then Add to Home Screen.</small>' : ''}`;
const mount = () => document.body.append(root, turn);
if (document.body) mount();
else addEventListener('DOMContentLoaded', mount);

const el = k => root.querySelector(`[data-b="${k}"]`);
const stamp = () => (pad.timestamp = performance.now());
const set = (k, on) => {
  const b = buttons[B[k]];
  b.pressed = b.touched = on;
  b.value = on ? 1 : 0;
  el(k).classList.toggle('on', on);
  if (!on) delete seenAt[k];
  stamp();
};
// up once no finger holds it and the game has had it down for a frame
const settle = k => {
  if (!held[k].size && buttons[B[k]].pressed && seenAt[k] !== undefined && tick > seenAt[k]) set(k, false);
};
// each finger on its own: a second finger on a button neither presses it again nor lets it go when it lifts
const press = (k, id) => {
  held[k].add(id);
  if (!buttons[B[k]].pressed) set(k, true);
  else el(k).classList.add('on');
};
const lift = (k, id) => {
  if (!held[k].delete(id)) return;
  if (!held[k].size) el(k).classList.remove('on');
  settle(k);
};
// a mouse or pen (`?touch` on a desktop); fingers come through the touch events below
for (const b of root.querySelectorAll('button[data-b]')) {
  const k = b.dataset.b;
  b.addEventListener('pointerdown', e => {
    if (e.pointerType === 'touch') return;
    e.preventDefault();
    b.setPointerCapture(e.pointerId);
    press(k, `p${e.pointerId}`);
  });
  for (const t of ['pointerup', 'pointercancel', 'lostpointercapture']) b.addEventListener(t, e => lift(k, `p${e.pointerId}`));
  b.addEventListener('contextmenu', e => e.preventDefault());
}

// the stick: centred where the thumb lands, full tilt at the base's radius; at rest it waits at the zone's corner
const zone = root.querySelector('.zone');
const base = zone.querySelector('.base');
const knob = zone.querySelector('.knob');
let stick = null;
const local = (x, y) => {
  const r = zone.getBoundingClientRect();
  return { x: x - r.left, y: y - r.top };
};
const place = (e, x, y) => {
  e.style.left = `${x}px`;
  e.style.top = `${y}px`;
};
const rest = () => {
  for (const e of [base, knob]) {
    e.style.left = 'calc(var(--l) + var(--u) * 1.8)';
    e.style.top = 'calc(100% - var(--b) - var(--u) * 1.8)';
  }
};
rest();
const stickDown = (id, x, y) => {
  if (stick) return;
  const p = local(x, y);
  stick = { id, ...p, at: performance.now(), far: 0, cx: x, cy: y };
  place(base, p.x, p.y);
  place(knob, p.x, p.y);
  zone.classList.add('held');
};
const stickMove = (id, x, y) => {
  if (stick?.id !== id) return;
  const p = local(x, y);
  const R = base.offsetWidth / 2 || 1;
  let dx = p.x - stick.x, dy = p.y - stick.y;
  const d = Math.hypot(dx, dy);
  stick.far = Math.max(stick.far, d / R);
  if (d > R) (dx *= R / d), (dy *= R / d);
  place(knob, stick.x + dx, stick.y + dy);
  pad.axes[0] = dx / R;
  pad.axes[1] = dy / R;
  stamp();
};
const stickUp = (id, cancelled) => {
  if (stick?.id !== id) return;
  const s = stick;
  stick = null;
  zone.classList.remove('held');
  rest();
  pad.axes[0] = pad.axes[1] = 0;
  stamp();
  // a short tap that hardly moved is a tap on the game: the menus' click, where the finger went down
  if (!cancelled && s.far < 0.25 && performance.now() - s.at < 350) {
    document.getElementById('screen')?.dispatchEvent(new PointerEvent('pointerdown', { clientX: s.cx, clientY: s.cy, bubbles: true }));
  }
};
zone.addEventListener('pointerdown', e => {
  if (e.pointerType === 'touch') return;
  e.preventDefault();
  zone.setPointerCapture(e.pointerId);
  stickDown(`p${e.pointerId}`, e.clientX, e.clientY);
});
zone.addEventListener('pointermove', e => stickMove(`p${e.pointerId}`, e.clientX, e.clientY));
zone.addEventListener('pointerup', e => stickUp(`p${e.pointerId}`, false));
for (const t of ['pointercancel', 'lostpointercapture']) zone.addEventListener(t, e => stickUp(`p${e.pointerId}`, true));

// Fingers by their touch events, each by its own `identifier`, from `changedTouches` only (opentennis B274: phones'
// pointer events for a second finger lost lifts and re-targeted; iOS has always delivered touch events per finger).
// A finger keeps the control it landed on until it lifts, wherever it slides.
const control = new Map(); // touch identifier -> button key, or 'stick'
root.addEventListener('touchstart', e => {
  if (e.cancelable) e.preventDefault();
  for (const t of e.changedTouches) {
    const hit = t.target instanceof Element ? t.target.closest('button, .zone') : null;
    if (!hit || !root.contains(hit) || control.has(t.identifier)) continue;
    if (hit === zone) {
      if (stick) continue;
      control.set(t.identifier, 'stick');
      stickDown(t.identifier, t.clientX, t.clientY);
    } else {
      control.set(t.identifier, hit.dataset.b);
      press(hit.dataset.b, t.identifier);
    }
  }
}, { passive: false });
root.addEventListener('touchmove', e => {
  if (e.cancelable) e.preventDefault();
  for (const t of e.changedTouches) if (control.get(t.identifier) === 'stick') stickMove(t.identifier, t.clientX, t.clientY);
}, { passive: false });
const letGo = (id, cancelled) => {
  const k = control.get(id);
  control.delete(id);
  if (k === 'stick') stickUp(id, cancelled);
  else if (k) lift(k, id);
};
addEventListener('touchend', e => { for (const t of e.changedTouches) letGo(t.identifier, false); }, true);
addEventListener('touchcancel', e => { for (const t of e.changedTouches) letGo(t.identifier, true); }, true);
// every touch event lists the fingers still down: one gone without its own end (a system gesture took it) lets go too
for (const t of ['touchstart', 'touchend', 'touchcancel']) addEventListener(t, e => {
  const down = new Set([...e.touches].map(t => t.identifier));
  for (const id of [...control.keys()]) if (!down.has(id)) letGo(id, true);
}, true);
const reset = () => {
  for (const id of [...control.keys()]) letGo(id, true);
  if (stick) stickUp(stick.id, true);
  for (const k in B) held[k].clear(), buttons[B[k]].pressed && set(k, false);
};

// listed after the browser's own pads while shown; never beside a real one (`update` hides it the frame one connects)
navigator.getGamepads = () => {
  for (const k in B) settle(k);
  for (const k in B) if (buttons[B[k]].pressed) seenAt[k] ??= tick;
  const list = [...realPads()];
  if (shown && !list.some(g => g && g.connected)) list.push(pad);
  return list;
};

// Shown on a touch screen, or after any touch, until a hardware key goes down; `?touch` forces them on.
let touched = coarse.matches || new URLSearchParams(location.search).has('touch');
addEventListener('pointerdown', e => { if (e.pointerType === 'touch') touched = true; }, { capture: true });
addEventListener('touchstart', () => (touched = true), { capture: true, passive: true });
const typing = t => t instanceof Element && t.matches('input, textarea, [contenteditable]');
addEventListener('keydown', e => { if (!typing(e.target) && !e.isComposing) touched = false; }, { capture: true });
// read by tests/web/touch.mjs
window.openmxTouch = { pad, shown: () => shown };

const viewport = document.querySelector('meta[name=viewport]');
const free = viewport?.content;
let riding = false;
const update = () => {
  if (started() !== riding) {
    riding = started();
    document.documentElement.classList.toggle('riding', riding);
    // no pinch or double-tap zoom while riding; the site around it keeps its zoom
    if (viewport) viewport.content = riding ? `${free}, maximum-scale=1, user-scalable=no` : free;
  }
  const want = riding && touched && !anyReal();
  if (want !== shown) {
    shown = want;
    root.hidden = !want;
    if (!want) reset();
  }
};
const frame = () => {
  tick++;
  update();
  requestAnimationFrame(frame);
};
requestAnimationFrame(frame);
for (const e of ['gamepadconnected', 'gamepaddisconnected']) addEventListener(e, update);

// ---- no browser gestures while riding ----
// iOS starts a selection, the magnifier or the long-press callout from a drag, and zooms on a double tap or pinch, and
// both iOS and Android go back a page on an edge swipe: the touch events' preventDefault stops those where pointer
// events' does not. Only on the canvas and the pad, only while riding: the site keeps its taps, links and inputs.
const ours = t => t instanceof Element && (t.tagName === 'CANVAS' || root.contains(t) || turn.contains(t));
for (const t of ['touchstart', 'touchmove']) addEventListener(t, e => { if (riding && ours(e.target) && e.cancelable) e.preventDefault(); }, { passive: false });
for (const t of ['gesturestart', 'gesturechange', 'dblclick', 'contextmenu', 'selectstart']) {
  addEventListener(t, e => { if (riding && !typing(e.target)) e.preventDefault(); }, { passive: false, capture: true });
}
document.addEventListener('selectionchange', () => {
  const s = getSelection();
  if (riding && s && !s.isCollapsed && !typing(document.activeElement)) s.removeAllRanges();
});
// An edge swipe (iOS Safari, Android's back gesture) goes back a page and would end the ride: touches that start at the
// screen's left or right edge are swallowed, and a spare history entry, pushed on the first tap while riding (Chrome
// skips entries a page added before any user activation), is all a back that gets through pops.
addEventListener('touchstart', e => {
  const x = e.changedTouches[0]?.clientX;
  if (riding && e.cancelable && (x < 20 || x > innerWidth - 20)) e.preventDefault();
}, { passive: false, capture: true });
const guard = () => riding && history.pushState({ openmx: 1 }, '', location.href);
addEventListener('pointerdown', function once() {
  if (!riding) return;
  removeEventListener('pointerdown', once, true);
  guard();
  addEventListener('popstate', guard);
}, true);
}
