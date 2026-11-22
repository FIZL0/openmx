#!/usr/bin/env python3
"""Decode PS2 .pst and Xbox .dxt textures to PNG and lay each pair side by side.
xbox_textures.py PS2_DIR XBOX_DIR OUT_DIR  (dirs hold the unpacked .res trees; pairs matched by name).
xbox_textures.py pack TEXTURES_SRC XBOX_DIR OUT [--keep NAME ...]: the Xbox pictures with more pixels as a
texture pack keyed to the PS2 textures (P383, `pack` below)."""
import os, struct, sys, zlib
import numpy as np

def u32(b, o): return struct.unpack_from("<I", b, o)[0]

# ---- PS2 .pst (after crates/game-render/src/pst.rs) ----
def pst(b):
    if u32(b, 4) == 0x2E:                         # 16 colours, two pixels a byte, low nibble first
        pal = np.frombuffer(b, np.uint8, 64, 8).reshape(16, 4).copy(); mips_at, bpp = 72, 4
    else:                                         # 256 colours, GS palette order
        raw = np.frombuffer(b, np.uint8, 1024, 8).reshape(256, 4)
        idx = [(i & 0xE7) | ((i & 8) << 1) | ((i & 16) >> 1) for i in range(256)]
        pal = raw[idx].copy(); mips_at, bpp = 1032, 8
    pal[:, 3] = np.minimum(pal[:, 3].astype(int) * 2, 255)
    n, o = u32(b, mips_at), mips_at + 4
    for _ in range(n):                            # smallest first: keep the last
        h, w, c, _z = struct.unpack_from("<4I", b, o); px = b[o + 16:o + 16 + c]; o += 16 + c
    a = np.frombuffer(px, np.uint8)
    if bpp == 4: a = np.stack([a & 15, a >> 4], 1).ravel()
    return pal[a[:w * h]].reshape(h, w, 4)

# ---- Xbox .dxt ----
def unswizzle(a, w, h):                           # Xbox Morton order -> linear
    # index of each linear (x, y) into the swizzled buffer: x and y bits interleaved, x first
    sx = np.zeros(w, np.int64); sy = np.zeros(h, np.int64)
    bit = 1; xb = yb = 1
    while xb < w or yb < h:
        if xb < w: sx += np.where(np.arange(w) & xb, bit, 0); bit <<= 1; xb <<= 1
        if yb < h: sy += np.where(np.arange(h) & yb, bit, 0); bit <<= 1; yb <<= 1
    return a[(sy[:, None] + sx[None, :]).ravel()].reshape(h, w, *a.shape[1:])

def rgb565(c):
    return np.stack([((c >> 11) & 31) * 255 // 31, ((c >> 5) & 63) * 255 // 63, (c & 31) * 255 // 31], -1)

def dxt_blocks(px, w, h, alpha5, alpha4=False):   # all blocks at once (the same sums as one block at a time)
    bs = 16 if alpha5 or alpha4 else 8           # alpha4: DXT3, sixteen 4-bit alphas before the colours
    bw, bh = max(w // 4, 1), max(h // 4, 1); n = bw * bh
    raw = np.frombuffer(px, np.uint8)[: n * bs]                         # short data: the missing blocks black
    blk = np.concatenate([raw, np.zeros(n * bs - len(raw), np.uint8)]).reshape(-1, bs)
    c = blk[:, bs - 8:].astype(np.int64)
    c0, c1 = c[:, 0] | c[:, 1] << 8, c[:, 2] | c[:, 3] << 8
    bits = c[:, 4] | c[:, 5] << 8 | c[:, 6] << 16 | c[:, 7] << 24
    p0, p1 = rgb565(c0), rgb565(c1)                                    # (n, 3)
    four = (c0 > c1) | alpha5 | alpha4
    cols = np.stack([p0, p1, np.where(four[:, None], (2 * p0 + p1) // 3, (p0 + p1) // 2),
                     np.where(four[:, None], (p0 + 2 * p1) // 3, 0)], 1)  # (n, 4, 3)
    al = np.where(four[:, None], 255, np.array([255, 255, 255, 0]))     # (n, 4)
    s = (bits[:, None] >> (2 * np.arange(16))) & 3                      # (n, 16)
    tile = np.zeros((n, 16, 4), np.uint8)
    tile[..., :3] = np.take_along_axis(cols, s[..., None], 1)
    tile[..., 3] = np.take_along_axis(al, s, 1)
    if alpha5:
        a0, a1 = blk[:, 0].astype(np.int64), blk[:, 1].astype(np.int64)
        ab = sum(blk[:, 2 + j].astype(np.int64) << (8 * j) for j in range(6))
        j6, j4 = np.arange(6), np.arange(4)
        eight = np.concatenate([a0[:, None], a1[:, None], ((6 - j6) * a0[:, None] + (j6 + 1) * a1[:, None]) // 7], 1)
        six = np.concatenate([a0[:, None], a1[:, None], ((4 - j4) * a0[:, None] + (j4 + 1) * a1[:, None]) // 5,
                              np.zeros((n, 1), np.int64), np.full((n, 1), 255)], 1)
        av = np.where((a0 > a1)[:, None], eight, six)                   # (n, 8)
        tile[..., 3] = np.take_along_axis(av, (ab[:, None] >> (3 * np.arange(16))) & 7, 1)
    if alpha4:                                    # low nibble first, row by row
        tile[..., 3] = np.stack([blk[:, :8] & 15, blk[:, :8] >> 4], -1).reshape(n, 16) * 17
    out = np.zeros((max(h, 4), max(w, 4), 4), np.uint8)
    out[:bh * 4, :bw * 4] = tile.reshape(bh, bw, 4, 4, 4).transpose(0, 2, 1, 3, 4).reshape(bh * 4, bw * 4, 4)
    return out[:h, :w]

def dxt_size(b):                                  # (w, h) of the mip dxt() decodes (the last), from the headers alone
    fmt = u32(b, 4)
    if fmt in (0x37, 0x38): o = 12 + u32(b, 8) * 4
    elif fmt in (0x26, 0x32, 0x36): o = 12
    else: raise ValueError(f"fmt {fmt:#x}")
    n = u32(b, o); o += 12
    for _ in range(n):
        w, h, c = struct.unpack_from("<3I", b, o); o += 12 + c
    return w, h

def dxt(b):
    fmt = u32(b, 4)
    if fmt in (0x37, 0x38):                       # 8-bit palette, BGRA entries; 0x37's alpha is real too (P383a:
        # 1,061 of 1,067 same-size pairs match the PS2 picture's alpha within 4, the other 6 within 14)
        npal = u32(b, 8); pal = np.frombuffer(b, np.uint8, npal * 4, 12).reshape(-1, 4)[:, [2, 1, 0, 3]].copy()
        o = 12 + npal * 4
    elif fmt in (0x26, 0x32, 0x36):               # DXT1, DXT3, DXT5
        o = 8 + 4
    else:
        raise ValueError(f"fmt {fmt:#x}")
    n = u32(b, o); o += 12                        # mip count, W, H
    for _ in range(n):
        w, h, c = struct.unpack_from("<3I", b, o); px = b[o + 12:o + 12 + c]; o += 12 + c
    if fmt in (0x37, 0x38):
        a = np.frombuffer(px, np.uint8)[:w * h]
        return pal[unswizzle(a, w, h)]
    return dxt_blocks(px, w, h, alpha5=(fmt == 0x36), alpha4=(fmt == 0x32))

def main(pd, xd, out):
    from PIL import Image, ImageDraw              # the side-by-side sheets only; `pack` needs no PIL
    os.makedirs(out, exist_ok=True); rows = []
    for root, _, fs in os.walk(pd):
        for f in sorted(fs):
            if not f.lower().endswith(".pst"): continue
            rel = os.path.relpath(os.path.join(root, f), pd)
            xp = os.path.join(xd, rel[:-4] + ".dxt")
            if not os.path.exists(xp): continue
            try:
                a = pst(open(os.path.join(root, f), "rb").read()); b = dxt(open(xp, "rb").read())
            except Exception as e:
                print("skip", rel, e); continue
            name = rel[:-4].replace(os.sep, "__")
            A, B = Image.fromarray(a, "RGBA"), Image.fromarray(b, "RGBA")
            A.save(f"{out}/{name}.ps2.png"); B.save(f"{out}/{name}.xbox.png")
            H = max(A.height, B.height) + 20
            s = Image.new("RGBA", (A.width + B.width + 10, H), (40, 40, 40, 255))
            s.paste(A, (0, 20)); s.paste(B, (A.width + 10, 20))
            d = ImageDraw.Draw(s); d.text((2, 3), f"PS2 {A.width}x{A.height}", fill="white")
            d.text((A.width + 12, 3), f"Xbox {B.width}x{B.height}", fill="white")
            os.makedirs(f"{out}/side_by_side", exist_ok=True); s.save(f"{out}/side_by_side/{name}.png")
            rows.append(f"{name}\tPS2 {A.width}x{A.height}\tXbox {B.width}x{B.height}")
    open(f"{out}/sizes.tsv", "w").write("\n".join(rows) + "\n"); print(len(rows), "pairs")

# ---- P383: the Xbox pictures as a texture pack, keyed to the PS2 textures they replace ----
# Textures whose Xbox picture the user judged better at the same size (file names without "-<key>.png",
# e.g. "free06_grass"): kept like the larger ones. Add to it, or pass --keep.
KEEP = ()

def png_size(p):                                  # (w, h) from the IHDR
    return struct.unpack(">II", open(p, "rb").read(24)[16:24])

def write_png(p, a):                              # RGBA8, unfiltered rows
    h, w = a.shape[:2]
    rows = b"".join(b"\0" + a[y].tobytes() for y in range(h))
    chunk = lambda t, d: struct.pack(">I", len(d)) + t + d + struct.pack(">I", zlib.crc32(t + d))
    open(p, "wb").write(b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 6, 0, 0, 0))
                        + chunk(b"IDAT", zlib.compress(rows, 9)) + chunk(b"IEND", b""))

def pack(src, xd, out, keep=()):
    """For every PS2 texture `openmx --dump-textures` wrote to `src` (`<archive>/<member>-<key>.png`), the
    Xbox `.dxt` of the same archive and member under `xd` (each .res unpacked by xbox_res.py to
    `<archive>/`, as `xbox_xiso.py` names it less `.res`; matched case-blind). Where the Xbox one has
    more pixels, or its name is in `keep`, write it as `out/textures/<member>-<PS2 key>.png`: a texture
    pack (`out/mod.json`, "kind": "textures"). Every pair goes in `out/survey.tsv`. How many were kept."""
    xbox = {}
    for root, _, fs in os.walk(xd):
        for f in fs:
            if f.lower().endswith(".dxt"):
                p = os.path.join(root, f)
                xbox[os.path.relpath(p, xd)[:-4].replace(os.sep, "/").lower()] = p
    keep = {k.lower() for k in (*KEEP, *keep)}
    rows, kept, alone, wrote = [], 0, 0, set()
    for root, _, fs in os.walk(src):
        for f in sorted(fs):
            stem, _, k = f[:-4].rpartition("-")
            if not f.lower().endswith(".png") or len(k) != 16 or not all(c in "0123456789abcdef" for c in k):
                continue
            rel = os.path.relpath(os.path.join(root, stem), src).replace(os.sep, "/")
            xp = xbox.get(rel.lower())
            if xp is None:
                alone += 1; continue
            try:
                xb = open(xp, "rb").read(); xw, xh = dxt_size(xb)
            except Exception as e:
                print("skip", rel, e); continue
            w, h = png_size(os.path.join(root, f))
            why = "more pixels" if xw * xh > w * h else "kept" if stem.lower() in keep else ""
            if why:                                   # only the kept ones are decoded
                try:
                    b = dxt(xb)
                except Exception as e:
                    print("skip", rel, e); continue
                p = os.path.join(out, "textures", f"{rel}-{k}.png")
                os.makedirs(os.path.dirname(p), exist_ok=True)
                write_png(p, np.ascontiguousarray(b, np.uint8)); kept += 1; wrote.add(os.path.abspath(p))
            rows.append(f"{rel}\t{k}\tPS2 {w}x{h}\tXbox {xw}x{xh}\t{why or 'PS2'}")
    # a picture an earlier run kept that this one did not (KEEP shrank, a texture changed) goes
    for root, _, fs in os.walk(os.path.join(out, "textures")):
        for f in fs:
            p = os.path.abspath(os.path.join(root, f))
            if f.lower().endswith(".png") and p not in wrote:
                os.remove(p); print("dropped", os.path.relpath(p, out))
    os.makedirs(out, exist_ok=True)
    mj = os.path.join(out, "mod.json")
    if not os.path.exists(mj):
        open(mj, "w").write('{"standard": 1, "kind": "textures", "id": "xbox-textures", "name": "Xbox textures"}\n')
    open(os.path.join(out, "survey.tsv"), "w").write("\n".join(rows) + "\n")
    print(f"{len(rows)} pairs, {kept} Xbox pictures kept, {alone} PS2 textures with no Xbox twin")
    return kept

# ---- P383a: which same-size pairs look different, as a page to pick from ----
def pairs(src, xd):
    """(rel, PS2 key, PS2 png path, Xbox .dxt path) of every PS2 dump PNG under `src` with an Xbox twin
    under `xd`, matched as `pack` matches them."""
    xbox = {}
    for root, _, fs in os.walk(xd):
        for f in fs:
            if f.lower().endswith(".dxt"):
                p = os.path.join(root, f)
                xbox[os.path.relpath(p, xd)[:-4].replace(os.sep, "/").lower()] = p
    for root, _, fs in os.walk(src):
        for f in sorted(fs):
            stem, _, k = f[:-4].rpartition("-")
            if not f.lower().endswith(".png") or len(k) != 16 or not all(c in "0123456789abcdef" for c in k):
                continue
            rel = os.path.relpath(os.path.join(root, stem), src).replace(os.sep, "/")
            if rel.lower() in xbox: yield rel, k, os.path.join(root, f), xbox[rel.lower()]

def look_apart(a, b):
    """How different two same-size RGBA pictures look, 0-255: each laid on grey by its alpha and averaged
    over 4x4 cells (so DXT's block noise and palette dither count little), the mean colour distance plus
    the alpha's, taken as the worst of the picture's 16 regions (a changed corner counts)."""
    def flat(p):
        p = p.astype(np.float32); al = p[..., 3:] / 255
        c = np.concatenate([p[..., :3] * al + 128 * (1 - al), p[..., 3:]], -1)
        h, w = (c.shape[0] // 4) * 4 or c.shape[0], (c.shape[1] // 4) * 4 or c.shape[1]
        c = c[:h, :w]
        if h >= 4 and w >= 4: c = c.reshape(h // 4, 4, w // 4, 4, 4).mean((1, 3))
        return c
    d = np.abs(flat(a) - flat(b)); d = d[..., :3].mean(-1) + d[..., 3] / 2
    return float(max(r.mean() for band in np.array_split(d, min(4, d.shape[0]), 0)
                     for r in np.array_split(band, min(4, d.shape[1]), 1)))

PAGE = """<title>Xbox Texture Picks</title>
<meta name="claude:viewport-fit-sides" content="cover">
<style>
:root{--bg:#f6f6f4;--fg:#1d1d1b;--mute:#6b6b66;--card:#fff;--line:#ddd;--on:#2f7d32;--chk:#e8e8e4}
@media (prefers-color-scheme:dark){:root:not([data-theme="light"]){--bg:#161615;--fg:#ececea;--mute:#9a9a94;--card:#222220;--line:#3a3a37;--on:#6cc070;--chk:#30302d;color-scheme:dark}}
:root[data-theme="dark"]{--bg:#161615;--fg:#ececea;--mute:#9a9a94;--card:#222220;--line:#3a3a37;--on:#6cc070;--chk:#30302d;color-scheme:dark}
body{background:var(--bg);color:var(--fg);padding-left:max(16px,env(safe-area-inset-left,0px));padding-right:max(16px,env(safe-area-inset-right,0px));padding-block:12px 90px}
h1{font-size:1.3em;margin:.2em 0}p{color:var(--mute);max-width:46em}
.grid{display:grid;grid-template-columns:repeat(auto-fill,minmax(min(100%,330px),1fr));gap:12px}
.card{background:var(--card);border:2px solid var(--line);border-radius:8px;padding:8px;cursor:pointer;min-width:0}
.card.on{border-color:var(--on)}
.pics{display:flex;gap:6px}.pics figure{margin:0;flex:1;min-width:0;text-align:center}
.pics img{width:100%;aspect-ratio:1;object-fit:contain;image-rendering:pixelated;background:repeating-conic-gradient(var(--chk) 0 25%,transparent 0 50%) 0 0/16px 16px;border-radius:4px}
figcaption{font-size:.8em;color:var(--mute)}
.name{font-weight:600;overflow-wrap:anywhere}.meta{font-size:.8em;color:var(--mute);overflow-wrap:anywhere}
.pick{display:flex;align-items:center;gap:6px;margin-top:4px}
.bar{position:fixed;left:0;right:0;bottom:0;background:var(--card);border-top:1px solid var(--line);padding:10px max(16px,env(safe-area-inset-right,0px)) calc(10px + env(safe-area-inset-bottom,0px)) max(16px,env(safe-area-inset-left,0px));display:flex;gap:10px;align-items:center;flex-wrap:wrap}
button{font:inherit;padding:6px 14px;border-radius:6px;border:1px solid var(--line);background:var(--bg);color:var(--fg);cursor:pointer}
button.main{background:var(--on);color:#fff;border-color:var(--on)}
#out{font-family:monospace;font-size:.8em;overflow-wrap:anywhere;user-select:all;flex-basis:100%}
</style>
<h1>Xbox texture picks</h1>
<p>__N__ textures where the Xbox picture is the PS2's size but looks different. Tick the ones where the Xbox
picture (right) looks better; the texture pack then uses it. Most different first. Shown at up to 256 px;
the checkerboard shows through where a picture's alpha is low, which on models is often a shine mask, not a
hole. Button pictures on the Xbox show its own buttons.</p>
<div class="grid" id="g"></div>
<div class="bar"><button class="main" id="send">Send picks</button><span id="said">0 picked</span><div id="out" hidden></div></div>
<script>
const TID="P383a", ROWS=__ROWS__;
const pick=new Set();
const g=document.getElementById('g'), said=document.getElementById('said');
ROWS.forEach(r=>{
  const c=document.createElement('div'); c.className='card';
  c.innerHTML=`<div class="pics"><figure><img loading="lazy" src="img/${r.f}.ps2.png" alt=""><figcaption>PS2</figcaption></figure>`+
    `<figure><img loading="lazy" src="img/${r.f}.xbox.png" alt=""><figcaption>Xbox</figcaption></figure></div>`+
    `<div class="pick"><input type="checkbox"><span class="name"></span></div><div class="meta"></div>`;
  c.querySelector('.name').textContent=r.stem;
  c.querySelector('.meta').textContent=`${r.size} · ${r.rel}${r.n>1?` · and ${r.n-1} more`:''} · apart ${r.d}`;
  const box=c.querySelector('input');
  const set=on=>{box.checked=on;c.classList.toggle('on',on);on?pick.add(r.stem):pick.delete(r.stem);said.textContent=pick.size+' picked'};
  c.onclick=e=>{if(e.target!==box)set(!box.checked);else set(box.checked)};
  g.appendChild(c);
});
async function store(txt){
  try{
    const db=window.claude&&window.claude.use?await window.claude.use('db'):null;
    if(!db)return false;
    await db.doc('picks/'+TID).set({id:TID,picks:txt,at:new Date().toISOString()});
    return true;
  }catch(e){return false}
}
document.getElementById('send').onclick=async()=>{
  const txt=pick.size?TID+' picks: keep '+[...pick].join(' '):TID+' picks: none';
  const out=document.getElementById('out'); out.textContent=txt; out.hidden=false;
  said.textContent='Sending…';
  if(await store(txt)){said.textContent='Sent: saved on this page. Say "sent" in the chat with the run.'}
  else{const done=ok=>{said.textContent=(ok?'Copied. ':'Copy this line. ')+'Paste it in the chat with the run.'};
    try{navigator.clipboard.writeText(txt).then(()=>done(true),()=>done(false))}catch(e){done(false)}}
};
</script>
"""

def differ(src, xd, out, least=12.0):             # 12: below it the pairs looked the same by eye (P383a)
    """Score every same-size pair (`look_apart`) into `out/differ.tsv`, most apart first; the stems whose
    worst pair is at least `least` apart go on `out/index.html`, a page to tick the Xbox pictures to keep
    (pictures in `out/img/`, at most 256 px). One card a stem, as `KEEP` names them. How many on the page."""
    import json
    from PIL import Image
    best = {}
    for rel, k, pp, xp in pairs(src, xd):
        try:
            xb = open(xp, "rb").read()
            if dxt_size(xb) != png_size(pp): continue
            a = np.asarray(Image.open(pp).convert("RGBA")); b = dxt(xb)
        except Exception as e:
            print("skip", rel, e); continue
        d = look_apart(a, b); stem = rel.rpartition("/")[2].lower()
        n = best[stem][4] + 1 if stem in best else 1
        if stem not in best or d > best[stem][0]: best[stem] = (d, rel, k, (pp, b), n)
        else: best[stem] = best[stem][:4] + (n,)
    rows = sorted(best.items(), key=lambda kv: -kv[1][0])
    os.makedirs(os.path.join(out, "img"), exist_ok=True)
    open(os.path.join(out, "differ.tsv"), "w").write("".join(
        f"{d:.1f}\t{s}\t{rel}\t{k}\t{n}\n" for s, (d, rel, k, _, n) in rows))
    page = []
    for s, (d, rel, k, (pp, b), n) in rows:
        if d < least: break
        f = f"{len(page):04d}"; A = Image.open(pp).convert("RGBA"); B = Image.fromarray(np.ascontiguousarray(b), "RGBA")
        size = f"{A.width}x{A.height}"
        for im, side in ((A, "ps2"), (B, "xbox")):
            if max(im.size) > 256: im = im.resize((max(im.width * 256 // max(im.size), 1), max(im.height * 256 // max(im.size), 1)), Image.LANCZOS)
            im.save(os.path.join(out, "img", f"{f}.{side}.png"), optimize=True)
        page.append({"f": f, "stem": s, "rel": rel, "size": size, "n": n, "d": round(d, 1)})
    open(os.path.join(out, "index.html"), "w").write(
        PAGE.replace("__N__", str(len(page))).replace("__ROWS__", json.dumps(page)))
    print(f"{len(rows)} same-size textures, {len(page)} at least {least} apart")
    return len(page)

if __name__ == "__main__":
    if sys.argv[1:2] == ["differ"]:
        # xbox_textures.py differ TEXTURES_SRC XBOX_DIR OUT [--least D]  (needs PIL: uv run --with pillow,numpy)
        a = sys.argv[2:]; least = float(a[a.index("--least") + 1]) if "--least" in a else 12.0
        differ(*a[:3], least=least)
    elif sys.argv[1:2] == ["pack"]:
        # xbox_textures.py pack TEXTURES_SRC XBOX_DIR OUT [--keep NAME ...]  (OUT: mods/xbox-textures)
        a = sys.argv[2:]; ks = a[a.index("--keep") + 1:] if "--keep" in a else []
        pack(*a[:3], keep=ks)
    else:
        main(*sys.argv[1:4])
