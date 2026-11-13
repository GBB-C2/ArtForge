"""Software QA view of the actual shipped mesh; requires NumPy and Pillow."""
from pathlib import Path
import json,math
import numpy as np
from PIL import Image,ImageDraw,ImageFont
ROOT=Path(__file__).resolve().parents[1]
data=json.loads((ROOT/'crates/core/assets/starter_character.json').read_text())
materials={m['id']:np.array(m['base_color'][:3]) for m in data['materials']}
bones=[(b['id'],b['parent'],b['translation']) for b in data['bones']]
def pose(wave=False):
 rest={1:np.eye(4)}; posed={1:np.eye(4)}
 for j,parent,t in bones:
  m=np.eye(4);m[:3,3]=t;rest[j]=rest[parent]@m
  angle={10:.30,11:1.05}.get(j,0) if wave else 0
  r=np.eye(4);r[:2,:2]=[[math.cos(angle),-math.sin(angle)],[math.sin(angle),math.cos(angle)]]
  posed[j]=posed[parent]@m@r
 return np.array([posed[j]@np.linalg.inv(rest[j]) for j in range(2,19)])
def scene(wave=False):
 palettes=pose(wave);result=[]
 for mesh in data['meshes']:
  p=np.array([v['position'] for v in mesh['vertices']]);n=np.array([v['normal'] for v in mesh['vertices']]);col=np.array([v['color'][:3] for v in mesh['vertices']]);js=np.array([v['joints'] for v in mesh['vertices']]);ws=np.array([v['weights'] for v in mesh['vertices']]);t=(palettes[js]*ws[:,:,None,None]).sum(1)
  p=np.einsum('nij,nj->ni',t,np.c_[p,np.ones(len(p))])[:,:3]
  n=np.einsum('nij,nj->ni',t[:,:3,:3],n);n/=np.maximum(np.linalg.norm(n,axis=1)[:,None],1e-12)
  for face in mesh['topology']['faces']:
   slot=None
   base=materials[slot or mesh['material']]
   for k in range(1,len(face)-1):
    ids=[face[0],face[k],face[k+1]];result.append((p[ids],n[ids],col[ids]*base))
 return result

def render(tris,w,h,eye,target,span,background=None):
 eye=np.array(eye,dtype=float);target=np.array(target,dtype=float)
 back=eye-target;back/=np.linalg.norm(back);right=np.cross([0,1,0],back);right/=np.linalg.norm(right);up=np.cross(back,right)
 basis=np.array([right,up,back]);scale=h/span
 rgb=np.broadcast_to(np.array([.22,.27,.31] if background is None else background),(h,w,3)).copy();rgb*=np.linspace(1.03,.97,h)[:,None,None];depth=np.full((h,w),-1e20)
 light=np.array([.4,.75,1]);light/=np.linalg.norm(light);fill=np.array([-.8,.2,.4]);fill/=np.linalg.norm(fill)
 for p,n,color in tris:
  projected=(p-target)@basis.T;screen=projected[:,:2]*[scale,-scale]+[w/2,h/2]
  xmin=max(0,int(np.floor(screen[:,0].min())));xmax=min(w-1,int(np.ceil(screen[:,0].max())))
  ymin=max(0,int(np.floor(screen[:,1].min())));ymax=min(h-1,int(np.ceil(screen[:,1].max())))
  if xmin>xmax or ymin>ymax: continue
  a,b,c=screen;den=(b[1]-c[1])*(a[0]-c[0])+(c[0]-b[0])*(a[1]-c[1])
  if abs(den)<1e-9: continue
  yy,xx=np.mgrid[ymin:ymax+1,xmin:xmax+1];xx=xx+.5;yy=yy+.5
  u=((b[1]-c[1])*(xx-c[0])+(c[0]-b[0])*(yy-c[1]))/den
  v=((c[1]-a[1])*(xx-c[0])+(a[0]-c[0])*(yy-c[1]))/den;z=1-u-v
  mask=(u>=-1e-6)&(v>=-1e-6)&(z>=-1e-6)
  ds=u*projected[0,2]+v*projected[1,2]+z*projected[2,2]
  tile=depth[ymin:ymax+1,xmin:xmax+1];mask &= ds>tile
  if not mask.any():continue
  illum=.25+.62*np.clip(n@light,0,1)+.22*np.clip(n@fill,0,1)
  colors=np.clip(color*illum[:,None],0,1)
  vals=u[:,:,None]*colors[0]+v[:,:,None]*colors[1]+z[:,:,None]*colors[2]
  rgb[ymin:ymax+1,xmin:xmax+1][mask]=vals[mask];tile[mask]=ds[mask]
 return Image.fromarray((np.clip(rgb,0,1)**(1/2.2)*255).astype('uint8'))

def main():
    root=ROOT/'docs'
    font=ImageFont.load_default()
    for candidate in ('DejaVuSans.ttf','C:/Windows/Fonts/segoeui.ttf','/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf'):
        try:
            font=ImageFont.truetype(candidate,19)
            break
        except OSError:
            pass
    neutral=scene();wave=scene(True)
    sheet=Image.new('RGB',(1800,990),'#edf0f3');draw=ImageDraw.Draw(sheet)
    panels=[(0,40,800,900,[0,1.1,5],[0,1.04,0],2.35,'T-поза · спереди',neutral),
            (810,40,660,900,[3,1.6,5],[0,1.04,0],2.35,'Форма · 3/4',neutral),
            (1480,40,320,420,[0,1.85,5],[0,1.87,0],.40,'Голова',neutral),
            (1480,510,320,430,[1.3,1.8,5],[.18,1.40,0],1.65,'Приветствие',wave)]
    for x,y,w,h,eye,target,span,label,tris in panels:
        sheet.paste(render(tris,w,h,eye,target,span),(x,y));draw.text((x+12,y-28),label,fill='#243447',font=font)
    draw.text((18,954),'ArtForge 0.47.7 · программный просмотр фактической сетки, не снимок приложения',fill='#243447',font=font)
    sheet.save(root/'starter-character.png')
    diagnostic=Image.new('RGB',(1660,990),'#edf0f3');draw=ImageDraw.Draw(diagnostic)
    panels=[(0,40,800,900,[0,1.1,-5],[0,1.04,0],2.35,'Сзади',neutral),
            (810,40,400,900,[5,1.10,.3],[0,1.04,0],2.35,'Профиль',neutral),
            (1220,40,440,420,[.95,1.8,5],[.91,1.56,0],.29,'Кисть · пять пальцев',neutral),
            (1220,510,440,430,[3,1.2,5],[0,.16,.06],.48,'Стопы и голеностоп',neutral)]
    for x,y,w,h,eye,target,span,label,tris in panels:
        diagnostic.paste(render(tris,w,h,eye,target,span),(x,y));draw.text((x+12,y-28),label,fill='#243447',font=font)
    draw.text((18,954),'Закрытые детали с шарнирами · 17 костей · базовая проекция UV',fill='#243447',font=font)
    diagnostic.save(root/'starter-character-back.png')
    print(root/'starter-character.png')
    print(root/'starter-character-back.png')
if __name__=='__main__':main()
