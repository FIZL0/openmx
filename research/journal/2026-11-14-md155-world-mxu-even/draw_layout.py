import sys
L=open(sys.argv[1]).read().split('\n'); out=sys.argv[2]
W,H=map(float,L[0].split()[1:])
R=[(l.split()[0],*map(float,l.split()[1:])) for l in L[1:] if l.strip()]
# gaps per side, as join_widths
P=[W,H]; worst=[]
gl=[]
for n,*r in R:
    for side in range(4):
        ax=side%2; up=side>=2; o=1-ax; best=(1e9,None)
        for n2,*r2 in R:
            if n2==n or r2[o+2]<=r[o] or r2[o]>=r[o+2]: continue
            d=(r2[ax]-r[ax+2]) if up else (r[ax]-r2[ax+2])
            if d<0: d+=P[ax]
            if 0<=d<best[0]: best=(d,n2)
        gl.append((best[0],n,side,best[1],r))
sc=900/max(W,H)
o=[f'<svg xmlns="http://www.w3.org/2000/svg" width="{W*sc+20:.0f}" height="{H*sc+30:.0f}"><rect width="100%" height="100%" fill="#ddd"/><g transform="translate(10,10)">',
   f'<rect x="0" y="0" width="{W*sc}" height="{H*sc}" fill="#c8b98a" stroke="black"/>']
for n,x0,z0,x1,z1 in R:
    c='#5a9' if 'free' in n else '#69c'
    o.append(f'<rect x="{x0*sc}" y="{z0*sc}" width="{(x1-x0)*sc}" height="{(z1-z0)*sc}" fill="{c}" stroke="black"/>')
    o.append(f'<text x="{(x0+x1)/2*sc}" y="{(z0+z1)/2*sc}" font-size="12" text-anchor="middle" font-family="sans">{n[4:]}</text>')
for d,n,side,n2,r in gl:
    ax=side%2; up=side>=2
    if ax==0: x=(r[2] if up else r[0]-d); y=(r[1]+r[3])/2; tx=x+d/2
    else: y=(r[3] if up else r[1]-d); x=(r[0]+r[2])/2; tx=x
    if d<1e8 and (r[ax+2]+d<=P[ax] if up else r[ax]-d>=0):
        col='red' if d>1300 else 'black'
        if ax==0: o.append(f'<line x1="{x*sc}" y1="{y*sc}" x2="{(x+d)*sc}" y2="{y*sc}" stroke="{col}"/><text x="{tx*sc}" y="{y*sc-2}" font-size="8" fill="{col}" text-anchor="middle">{d:.0f}</text>')
        else: o.append(f'<line x1="{x*sc}" y1="{y*sc}" x2="{x*sc}" y2="{(y+d)*sc}" stroke="{col}"/><text x="{x*sc+2}" y="{(y+d/2)*sc}" font-size="8" fill="{col}">{d:.0f}</text>')
gl.sort(key=lambda g:g[0])
o.append(f'<text x="5" y="{H*sc+12}" font-size="10">{sys.argv[3] if len(sys.argv)>3 else ""} {W:.0f} x {H:.0f} ft, {len(R)} tiles; join widths (red over 1300 ft; widest {gl[-1][0]:.0f}, median {gl[len(gl)//2][0]:.0f})</text></g></svg>')
open(out,'w').write('\n'.join(o))
print('widest',[(round(g[0]),g[1],g[2],g[3]) for g in gl[-6:]],'median',gl[len(gl)//2][0])
