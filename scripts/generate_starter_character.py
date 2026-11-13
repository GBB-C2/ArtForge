"""Original articulated mannequin; Y up, metres, +Z forward.

Closed anatomical shells and joint balls follow the existing 17-joint rig.
Python is needed only to regenerate the embedded asset. No downloaded assets.
"""
from pathlib import Path
import json
import math

ROOT = Path(__file__).resolve().parents[1]
BONES = [
 (2,'Таз',1,[0,1.105,0]),(3,'Позвоночник',2,[0,.17,0]),(4,'Грудь',3,[0,.235,0]),
 (5,'Шея',4,[0,.18,-.008]),(6,'Голова',5,[0,.115,.008]),
 (7,'Плечо.L',4,[-.235,.07,0]),(8,'Локоть.L',7,[-.30,0,0]),(9,'Кисть.L',8,[-.27,0,0]),
 (10,'Плечо.R',4,[.235,.07,0]),(11,'Локоть.R',10,[.30,0,0]),(12,'Кисть.R',11,[.27,0,0]),
 (13,'Бедро.L',2,[-.115,-.04,0]),(14,'Колено.L',13,[0,-.49,.008]),(15,'Стопа.L',14,[0,-.46,-.008]),
 (16,'Бедро.R',2,[.115,-.04,0]),(17,'Колено.R',16,[0,-.49,.008]),(18,'Стопа.R',17,[0,-.46,-.008]),
]
MATERIALS = [dict(id=i,name=n,base_color=[.65,.595,.455,1.],metallic=0.,roughness=.58,
 emissive=[0.,0.,0.],base_color_texture=None) for i,n in
 [(200,'Манекен · корпус'),(202,'Манекен · голова'),(203,'Манекен · кисти и стопы')]]

def add(a,b): return tuple(x+y for x,y in zip(a,b))
def sub(a,b): return tuple(x-y for x,y in zip(a,b))
def mul(a,s): return tuple(x*s for x in a)
def dot(a,b): return sum(x*y for x,y in zip(a,b))
def cross(a,b): return (a[1]*b[2]-a[2]*b[1],a[2]*b[0]-a[0]*b[2],a[0]*b[1]-a[1]*b[0])
def unit(a): return mul(a,1/max(math.sqrt(dot(a,a)),1e-15))
def gaussian(x,c,s): return math.exp(-.5*((x-c)/s)**2)

def interpolate(rows,t):
 k=next((i for i in range(len(rows)-1) if rows[i+1][0]>=t),len(rows)-2)
 a,b=rows[k],rows[k+1];u=(t-a[0])/(b[0]-a[0]);h=b[0]-a[0]
 prev,nxt=rows[max(k-1,0)],rows[min(k+2,len(rows)-1)]
 return [(2*u**3-3*u*u+1)*a[c]+(u**3-2*u*u+u)*h*(b[c]-prev[c])/(b[0]-prev[0])
         +(-2*u**3+3*u*u)*b[c]+(u**3-u*u)*h*(nxt[c]-a[c])/(nxt[0]-a[0])
         for c in range(1,len(a))]

class Builder:
 def __init__(self):
  self.points=[];self.faces=[];self.tags=[];self.joints=[];self.colors=[]
 def shell(self,origin,u,v,axis,profiles,joint,tag=100,sides=24,steps=3,tint=1.,sculpt=None):
  """One closed loft. Profile columns: axial distance, two radii, two offsets."""
  start=len(self.points);faces=[];centers=[]
  times=[a[0]+(b[0]-a[0])*j/steps for a,b in zip(profiles,profiles[1:]) for j in range(steps)]
  times.append(profiles[-1][0])
  for t in times:
   ru,rv,cu,cv=interpolate(profiles,t)
   center=add(add(origin,mul(axis,t)),add(mul(u,cu),mul(v,cv)));centers.append(center)
   for j in range(sides):
    angle=j*math.tau/sides
    p=add(center,add(mul(u,math.cos(angle)*max(ru,.0001)),mul(v,math.sin(angle)*max(rv,.0001))))
    self.points.append(sculpt(p,angle) if sculpt else p)
  for r in range(len(times)-1):
   a=start+r*sides;b=a+sides
   for j in range(sides):
    k=(j+1)%sides;faces.append([a+j,a+k,b+k,b+j])
  lower=len(self.points);self.points.append(centers[0])
  upper=len(self.points);self.points.append(centers[-1]);last=start+(len(times)-1)*sides
  for j in range(sides):
   k=(j+1)%sides;faces.extend([[lower,start+k,start+j],[upper,last+j,last+k]])
  volume=sum(dot(self.points[f[0]],cross(self.points[f[i]],self.points[f[i+1]]))
             for f in faces for i in range(1,len(f)-1))
  assert abs(volume)>1e-10
  if volume<0:faces=[list(reversed(f)) for f in faces]
  self.faces.extend(faces);self.tags.extend([tag]*len(faces))
  self.joints.extend([joint-2]*(len(self.points)-start))
  self.colors.extend([[tint,tint,tint,1.]]*(len(self.points)-start))
 def ball(self,center,radii,joint,tag=100,sides=20,rings=12,tint=.88):
  # Nonzero pole rings avoid collapsed faces after decimal serialization.
  rows=[]
  for i in range(rings+1):
   angle=.045+(math.pi-.09)*i/rings
   rows.append((-math.cos(angle)*radii[1],math.sin(angle)*radii[0],math.sin(angle)*radii[2],0.,0.))
  self.shell(center,(1,0,0),(0,0,1),(0,1,0),rows,joint,tag,sides,1,tint)
 def finish(self):
  normals=[(0.,0.,0.) for _ in self.points]
  for f in self.faces:
   for k in range(1,len(f)-1):
    ids=(f[0],f[k],f[k+1]);a,b,c=[self.points[i] for i in ids]
    normal=cross(sub(b,a),sub(c,a));assert dot(normal,normal)>1e-23
    for i in ids:normals[i]=add(normals[i],normal)
  vertices=[dict(position=[round(x,8) for x in p],normal=[round(x,8) for x in unit(n)],
   uv=[round((math.atan2(p[2],p[0])+math.pi)/math.tau,8),round(max(0.,min(1.,p[1]/2.02)),8)],
   color=color,joints=[joint,0,0,0],weights=[1.,0.,0.,0.])
   for p,n,color,joint in zip(self.points,normals,self.colors,self.joints)]
  result=[]
  for ident,name,material in [(100,'Манекен · корпус и шарниры',200),(102,'Манекен · голова',202),(103,'Манекен · кисти и стопы',203)]:
   faces=[f for f,t in zip(self.faces,self.tags) if t==ident]
   used=sorted({v for f in faces for v in f});remap={v:i for i,v in enumerate(used)}
   polygons=[[remap[v] for v in f] for f in faces]
   result.append(dict(id=ident,name=name,vertices=[vertices[i] for i in used],
    indices=[v for f in polygons for i in range(1,len(f)-1) for v in (f[0],f[i],f[i+1])],
    material=material,morph_targets=[],topology=dict(faces=polygons,face_materials=[],
    uv_seams=[],smooth_normals=True,primitive=None)))
  return result

def torso(b):
 def chest(p,angle):
  x,y,z=p;front=max(0.,math.sin(angle))**3;back=max(0.,-math.sin(angle))**3
  pec=sum(gaussian(x,s*.095,.062) for s in (-1,1))*gaussian(y,1.535,.045)*.022
  z+=front*(pec-gaussian(x,0,.012)*gaussian(y,1.48,.12)*.008)
  z-=back*.009*gaussian(y,1.52,.065)*sum(gaussian(x,s*.10,.052) for s in (-1,1))
  y+=.065*(abs(x)/.22)**1.3*gaussian(y,1.37,.07)
  return x,y,z
 b.shell((0,0,0),(1,0,0),(0,0,1),(0,1,0),[
  (1.335,.047,.045,0,0),(1.355,.115,.071,0,0),(1.41,.173,.093,0,0),
  (1.48,.205,.103,0,-.006),(1.55,.215,.100,0,-.009),(1.61,.190,.079,0,-.016),
  (1.65,.112,.051,0,-.016),(1.664,.052,.039,0,-.010)],4,sides=32,sculpt=chest)
 b.shell((0,0,0),(1,0,0),(0,0,1),(0,1,0),[
  (1.175,.047,.043,0,0),(1.19,.098,.070,0,0),(1.24,.124,.081,0,.002),
  (1.29,.128,.083,0,0),(1.33,.105,.073,0,-.004),(1.36,.047,.039,0,0)],3,sides=28)
 b.ball((0,1.182,0),(.074,.068,.067),3,sides=24)
 b.shell((0,0,0),(1,0,0),(0,0,1),(0,1,0),[
  (.983,.050,.043,0,.008),(1.005,.091,.070,0,.005),(1.052,.155,.096,0,-.007),
  (1.105,.173,.099,0,-.016),(1.15,.150,.084,0,-.01),(1.18,.103,.057,0,0),
  (1.186,.047,.033,0,0)],2,sides=32)
 b.ball((0,1.708,-.008),(.047,.081,.044),5,sides=24,tint=.94)
 b.shell((0,0,0),(1,0,0),(0,0,1),(0,1,0),[
  (1.745,.025,.037,0,.015),(1.767,.054,.062,0,.018),(1.80,.077,.076,0,.012),
  (1.85,.091,.088,0,.002),(1.90,.099,.090,0,-.005),(1.95,.093,.084,0,-.010),
  (1.987,.071,.067,0,-.012),(2.008,.037,.038,0,-.012),(2.014,.003,.003,0,-.012)],6,102,sides=32)

def hands(b,side,wrist):
 origin=(side*.805,1.58,0);axis=(side,0,0)
 b.shell(origin,(0,1,0),(0,0,1),axis,[
  (.014,.025,.017,0,0),(.035,.038,.020,-.004,.002),(.065,.042,.019,-.005,.003),
  (.091,.039,.016,-.005,.003),(.104,.030,.014,-.005,.003)],wrist,103,sides=20)
 for offset,length,radius in [(.026,.088,.0098),(.006,.102,.010),(-.014,.094,.0095),(-.033,.074,.0082)]:
  center=add(origin,(side*.094,offset,.002))
  rows=[(t*length,radius*r,radius*r*.92,-.006*t*t,.009*t*t) for t,r in
   [(0,.94),(.13,1.03),(.29,.90),(.39,.98),(.58,.84),(.67,.91),(.87,.73),(.96,.49),(1,.08)]]
  b.shell(center,(0,1,0),(0,0,1),axis,rows,wrist,103,sides=12,steps=2)
 direction=unit((side*.74,-.67,.04));radial=unit((side*.67,.74,0))
 b.shell(add(origin,(side*.037,-.028,.004)),radial,(0,0,1),direction,[
  (0,.015,.016,0,0),(.025,.014,.014,0,0),(.045,.012,.012,0,0),
  (.063,.010,.010,0,.002),(.073,.006,.006,0,.003),(.076,.001,.001,0,.003)],wrist,103,sides=12,steps=2)

def arms(b):
 for side in (-1,1):
  shoulder,elbow,wrist=(7,8,9) if side<0 else (10,11,12)
  b.ball((side*.235,1.58,0),(.062,.067,.064),shoulder,sides=24,tint=.98)
  axis=(side,0,0)
  b.shell((side*.235,1.58,0),(0,1,0),(0,0,1),axis,[
   (.025,.052,.052,0,0),(.067,.066,.060,.002,0),(.113,.065,.058,-.005,.005),
   (.174,.056,.052,-.007,.004),(.226,.043,.043,-.005,0),(.264,.032,.033,0,0)],shoulder,sides=24)
  b.ball((side*.535,1.58,0),(.040,.037,.036),elbow,sides=20)
  b.shell((side*.535,1.58,0),(0,1,0),(0,0,1),axis,[
   (.029,.029,.029,0,0),(.060,.045,.044,.004,0),(.108,.044,.041,.002,.003),
   (.163,.035,.033,0,.002),(.213,.025,.025,0,0),(.246,.021,.022,0,0)],elbow,sides=24)
  b.ball((side*.805,1.58,0),(.027,.023,.021),wrist,sides=16,rings=10)
  hands(b,side,wrist)

def legs(b):
 for side in (-1,1):
  hip,knee,ankle=(13,14,15) if side<0 else (16,17,18);x=side*.115
  b.ball((x,1.065,0),(.070,.073,.071),hip,sides=24)
  b.shell((x,0,0),(1,0,0),(0,0,1),(0,1,0),[
   (.611,.039,.041,0,.009),(.67,.049,.051,0,.009),(.75,.065,.063,side*.005,.010),
   (.85,.082,.080,side*.011,.006),(.95,.086,.091,side*.004,0),
   (1.025,.074,.080,-side*.005,-.003),(1.05,.047,.058,-side*.005,0)],hip,sides=28)
  b.ball((x,.575,.008),(.046,.050,.046),knee,sides=24)
  b.shell((x,0,0),(1,0,0),(0,0,1),(0,1,0),[
   (.143,.027,.027,0,0),(.21,.030,.032,0,-.001),(.30,.039,.043,-side*.005,-.007),
   (.40,.052,.060,-side*.008,-.016),(.47,.055,.060,-side*.005,-.016),
   (.515,.042,.046,0,-.003),(.536,.032,.034,0,.004)],knee,sides=24)
  b.ball((x,.115,0),(.034,.039,.033),ankle,sides=20,rings=10)
  b.shell((x,.048,0),(1,0,0),(0,1,0),(0,0,1),[
   (-.083,.013,.020,0,.009),(-.066,.037,.038,0,.009),(-.025,.043,.041,0,.009),
   (.035,.047,.033,0,.004),(.10,.060,.028,side*.005,-.004),
   (.16,.065,.023,side*.003,-.009),(.198,.052,.018,0,-.011),(.213,.009,.005,0,-.012)],ankle,103,sides=24)

def generate():
 b=Builder();torso(b);arms(b);legs(b)
 return dict(meshes=b.finish(),bones=[dict(id=i,name=n,parent=p,translation=t) for i,n,p,t in BONES],materials=MATERIALS)

def update_example(result):
 path=ROOT/'examples/starter-character.artforge';project=json.loads(path.read_text(encoding='utf-8'))
 project['version']=30;project['title']='Шарнирный манекен'
 project['meshes']=result['meshes'];project['materials']=result['materials']
 nodes={n['id']:n for n in project['nodes']};global_positions={1:(0,0,0)}
 for ident,name,parent,translation in BONES:
  node=nodes[ident];node['name']=name;node['parent']=parent
  node['transform']=dict(translation=translation,rotation=[0,0,0,1],scale=[1,1,1])
  global_positions[ident]=add(global_positions[parent],translation)
 inverse=[]
 for ident in range(2,19):
  x,y,z=global_positions[ident];inverse.append([1,0,0,0,0,1,0,0,0,0,1,0,-x,-y,-z,1])
 for ident,mesh,name in ((20,100,'Корпус и шарниры'),(21,102,'Голова'),(22,103,'Кисти и стопы')):
  node=nodes[ident];node['name']=name;node['mesh']=mesh;node['skin']['inverse_bind_matrices']=inverse
 project['camera']['eye']=[0.,1.05,3.4];project['camera']['target']=[0.,1.02,0.]
 for channel in project['clips'][0]['channels']:
  if channel['property']=='Translation':
   for key in channel['keys']:key['value']=nodes[channel['node']]['transform']['translation']
  if channel['property']!='Rotation' or channel['node'] not in (10,11):continue
  angles=[0.,.30,.42,.30,.42,.30,0.,0.] if channel['node']==10 else [0.,1.05,.70,1.05,.70,1.05,0.,0.]
  for key,angle in zip(channel['keys'],angles):key['value']=[0,0,math.sin(angle/2),math.cos(angle/2)]
 path.write_text(json.dumps(project,ensure_ascii=False,separators=(',',':'))+'\n',encoding='utf-8')

def main():
 result=generate();path=ROOT/'crates/core/assets/starter_character.json'
 path.write_text(json.dumps(result,ensure_ascii=False,separators=(',',':'))+'\n',encoding='utf-8')
 update_example(result)
 print(json.dumps({'vertices':sum(len(m['vertices']) for m in result['meshes']),
 'triangles':sum(len(m['indices'])//3 for m in result['meshes']),
 'polygons':sum(len(m['topology']['faces']) for m in result['meshes'])}))

if __name__=='__main__':main()

