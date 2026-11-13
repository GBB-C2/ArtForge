"""Build the bundled DejaVu Sans geometry cache. Runtime needs no font parser.
Regeneration: Python + fontTools + scipy + numpy, and the unmodified DejaVuSans.ttf font.
"""
from pathlib import Path
from fontTools.ttLib import TTFont
from fontTools.pens.basePen import BasePen
import json, math, sys
ROOT=Path(__file__).resolve().parents[1]

def cross(a,b,c):return (b[0]-a[0])*(c[1]-a[1])-(b[1]-a[1])*(c[0]-a[0])
def area(r):return sum(a[0]*b[1]-a[1]*b[0] for a,b in zip(r,r[1:]+r[:1]))/2

def inside(p,r):
    result=False
    for a,b in zip(r,r[1:]+r[:1]):
        if (a[1]>p[1])!=(b[1]>p[1]) and p[0]<(b[0]-a[0])*(p[1]-a[1])/(b[1]-a[1])+a[0]:result=not result
    return result

def crosses(a,b,c,d):return cross(a,b,c)*cross(a,b,d)<-1e-20 and cross(c,d,a)*cross(c,d,b)<-1e-20

class Pen(BasePen):
    def __init__(self,glyphs,units):super().__init__(glyphs);self.units=units;self.rings=[];self.r=[]
    def pt(self,p):return (round(p[0]/self.units,7),round(p[1]/self.units,7))
    def _moveTo(self,p):self._closePath();self.r=[self.pt(p)]
    def _lineTo(self,p):
        p=self.pt(p)
        if not self.r or p!=self.r[-1]:self.r.append(p)
    def _qCurveToOne(self,b,c):
        a=self._getCurrentPoint();steps=max(2,min(32,math.ceil(math.sqrt(math.dist(a,b)+math.dist(b,c))/2)))
        for i in range(1,steps+1):
            t=i/steps;u=1-t;self._lineTo(tuple(u*u*a[k]+2*u*t*b[k]+t*t*c[k] for k in (0,1)))
    def _curveToOne(self,b,c,d):
        a=self._getCurrentPoint()
        for i in range(1,17):
            t=i/16;u=1-t;self._lineTo(tuple(u**3*a[k]+3*u*u*t*b[k]+3*u*t*t*c[k]+t**3*d[k] for k in (0,1)))
    def _closePath(self):
        r=self.r
        if len(r)>1 and r[0]==r[-1]:r.pop()
        # Collinear points would form zero-area cap triangles.
        changed=True
        while changed and len(r)>3:
            changed=False
            for i in range(len(r)):
                if abs(cross(r[i-1],r[i],r[(i+1)%len(r)]))<1e-12:
                    r.pop(i);changed=True;break
        if len(r)>=3 and abs(area(r))>1e-10:self.rings.append(r)
        self.r=[]
    def _endPath(self):self._closePath()

def triangulate(rings):
    from scipy.spatial import Delaunay
    import numpy as np
    if not rings:return [],[],[]
    depths=[sum(inside(r[0],other) for j,other in enumerate(rings) if i!=j) for i,r in enumerate(rings)]
    rings=[r if (area(r)>0)==(d%2==0) else list(reversed(r)) for r,d in zip(rings,depths)]
    expected=sum(area(r) for r in rings)
    # Refine only absent boundary edges until the Delaunay tessellation conforms
    # to every contour. This preserves counters without artificial bridge faces.
    for _ in range(20):
        points=[];lookup={};contours=[]
        for r in rings:
            ids=[]
            for p in r:
                if p not in lookup:lookup[p]=len(points);points.append(p)
                ids.append(lookup[p])
            contours.append(ids)
        if len(points)==3:simplices=[[0,1,2]]
        else:simplices=Delaunay(np.asarray(points)).simplices.tolist()
        edges={tuple(sorted((t[i],t[(i+1)%3]))) for t in simplices for i in range(3)}
        changed=False;refined=[]
        for r,ids in zip(rings,contours):
            out=[]
            for i,a in enumerate(r):
                b=r[(i+1)%len(r)];out.append(a)
                if tuple(sorted((ids[i],ids[(i+1)%len(ids)]))) not in edges:
                    out.append(((a[0]+b[0])/2,(a[1]+b[1])/2));changed=True
            refined.append(out)
        if not changed:break
        rings=refined
    else:raise ValueError('boundary tessellation did not converge')
    tris=[]
    for t in simplices:
        center=tuple(sum(points[i][k] for i in t)/3 for k in (0,1))
        if sum(inside(center,r) for r in rings)%2==0:continue
        if cross(*(points[i] for i in t))<0:t.reverse()
        first=t.index(min(t));t=t[first:]+t[:first]
        tris.append(t)
    tris.sort()
    actual=sum(cross(points[a],points[b],points[c])/2 for a,b,c in tris)
    if abs(expected-actual)>1e-7 or any(cross(points[a],points[b],points[c])<=0 for a,b,c in tris):raise ValueError('invalid cap area/winding')
    return points,contours,tris

def bevel_data(points,contours):
    """Bound the outline expansion before counters or separate strokes meet."""
    import numpy as np
    if not points:return [],0.
    p=np.asarray(points);offset=np.zeros_like(p);edges=[]
    for ids in contours:
        for i,b in enumerate(ids):
            a=ids[i-1];c=ids[(i+1)%len(ids)]
            prev=p[b]-p[a];after=p[c]-p[b]
            prev/=np.linalg.norm(prev);after/=np.linalg.norm(after)
            n1=np.array([prev[1],-prev[0]]);n2=np.array([after[1],-after[0]])
            m=n1+n2;den=np.dot(m,n2)
            offset[b]=m/max(den,0.25)
            edges.append((b,c))
    edges=np.asarray(edges);a,b=edges.T
    separate=(a[:,None]!=a)&(a[:,None]!=b)&(b[:,None]!=a)&(b[:,None]!=b)
    def safe(radius):
        q=p+offset*radius;start=q[a];end=q[b];v=end-start
        d1=start[None,:,:]-start[:,None,:];d2=end[None,:,:]-start[:,None,:]
        c1=v[:,None,0]*d1[:,:,1]-v[:,None,1]*d1[:,:,0]
        c2=v[:,None,0]*d2[:,:,1]-v[:,None,1]*d2[:,:,0]
        low=np.minimum(start,end);high=np.maximum(start,end)
        overlap=np.all((low[:,None,:]<=high[None,:,:])&(low[None,:,:]<=high[:,None,:]),axis=2)
        if np.any(separate&overlap&(c1*c2<=0)&(c1.T*c2.T<=0)):return False
        return all(area([tuple(q[i]) for i in ids])*area([points[i] for i in ids])>0 for ids in contours)
    lo,hi=0.,0.04
    for _ in range(12):
        mid=(lo+hi)/2
        if safe(mid):lo=mid
        else:hi=mid
    return offset.tolist(),round(lo*0.8,8)

def build(path):
    font=TTFont(path);glyphs=font.getGlyphSet();units=font['head'].unitsPerEm;cmap=font.getBestCmap();out={}
    chars=set(range(32,127))|{160,171,176,187,215,247}|set(range(0x400,0x460))|{0x2013,0x2014,0x2018,0x2019,0x201c,0x201d,0x2022,0x2026,0x2116,0x20bd}
    for code in sorted(chars):
        if code not in cmap:continue
        glyph=glyphs[cmap[code]];pen=Pen(glyphs,units);glyph.draw(pen);pen._closePath()
        try:points,contours,tris=triangulate(pen.rings)
        except ValueError as e:raise ValueError(f'U+{code:04X} {chr(code)}: {e}') from e
        offset,limit=bevel_data(points,contours)
        out[chr(code)]={'advance':round(glyph.width/units,7),'points':points,'contours':contours,'triangles':tris,'bevel_offsets':offset,'bevel_limit':limit}
    return {'font':font['name'].getDebugName(1),'glyphs':out}

if __name__=='__main__':
    font=Path(sys.argv[1]) if len(sys.argv)>1 else Path('/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf')
    result=build(font);path=Path(sys.argv[2]) if len(sys.argv)>2 else ROOT/'crates/core/assets/text_glyphs.json';path.write_text(json.dumps(result,ensure_ascii=False,separators=(',',':'))+'\n')
    print(len(result['glyphs']),'glyphs;',path.stat().st_size,'bytes')
