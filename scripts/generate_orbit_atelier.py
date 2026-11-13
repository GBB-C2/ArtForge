"""Build the editable Atelier example from owned geometry, without downloads."""
from pathlib import Path
from copy import deepcopy
import json
import math
import numpy as np

ROOT=Path(__file__).resolve().parents[1]

def quaternion(axis,angle):
    axis=np.asarray(axis,float);axis/=np.linalg.norm(axis)
    return [*(axis*math.sin(angle/2)),math.cos(angle/2)]

def qmul(a,b):
    av,bv=np.array(a[:3]),np.array(b[:3])
    return [*(a[3]*bv+b[3]*av+np.cross(av,bv)),a[3]*b[3]-float(av@bv)]

def mesh_record(mid,name,points,normals,faces,material,uv):
    normals=np.asarray(normals,float);normals/=np.maximum(np.linalg.norm(normals,axis=1)[:,None],1e-12)
    vertices=[dict(position=list(map(float,p)),normal=list(map(float,n)),uv=list(map(float,t)),
                   color=[1.,1.,1.,1.],joints=[0,0,0,0],weights=[0.,0.,0.,0.]) for p,n,t in zip(points,normals,uv)]
    indices=[v for f in faces for k in range(1,len(f)-1) for v in [f[0],f[k],f[k+1]]]
    return dict(id=mid,name=name,vertices=vertices,indices=indices,material=material,morph_targets=[],
                topology=dict(faces=faces,face_materials=[],uv_seams=[],smooth_normals=True,primitive=None))

def lathe(mid,name,profile,material,sides=64):
    points=[];normals=[];uv=[];faces=[]
    for j,(y,r) in enumerate(profile):
        a=profile[max(0,j-1)];b=profile[min(len(profile)-1,j+1)]
        for i in range(sides):
            angle=2*math.pi*i/sides;c=math.cos(angle);s=math.sin(angle)
            points.append([r*c,y,r*s]);normals.append([(b[0]-a[0])*c,a[1]-b[1],(b[0]-a[0])*s]);uv.append([i/sides,j/(len(profile)-1)])
    for j in range(len(profile)-1):
        for i in range(sides):
            n=(i+1)%sides;faces.append([j*sides+i,(j+1)*sides+i,(j+1)*sides+n,j*sides+n])
    for top,(y,r) in enumerate([profile[0],profile[-1]]):
        start=len(points);points.append([0,y,0]);normals.append([0,1 if top else -1,0]);uv.append([.5,.5])
        for i in range(sides):
            angle=2*math.pi*i/sides;c=math.cos(angle);s=math.sin(angle)
            points.append([r*c,y,r*s]);normals.append([0,1 if top else -1,0]);uv.append([.5+.5*c,.5+.5*s])
        for i in range(sides):
            a=start+1+i;b=start+1+(i+1)%sides
            faces.append([start,b,a] if top else [start,a,b])
    return mesh_record(mid,name,points,normals,faces,material,uv)

def torus(mid,name,major,minor,material,sides=64,rings=8):
    points=[];normals=[];uv=[];faces=[]
    for i in range(sides):
        a=2*math.pi*i/sides
        for j in range(rings):
            b=2*math.pi*j/rings;c=math.cos(b);s=math.sin(b)
            points.append([(major+minor*c)*math.cos(a),minor*s,(major+minor*c)*math.sin(a)])
            normals.append([c*math.cos(a),s,c*math.sin(a)]);uv.append([i/sides,j/rings])
            faces.append([i*rings+j,i*rings+(j+1)%rings,((i+1)%sides)*rings+(j+1)%rings,((i+1)%sides)*rings+j])
    return mesh_record(mid,name,points,normals,faces,material,uv)

def sphere(mid,name,material,sides=32,rings=18):
    points=[[0,-1,0]];normals=[[0,-1,0]];uv=[[.5,0]];faces=[]
    for j in range(1,rings):
        a=-math.pi/2+math.pi*j/rings
        for i in range(sides):
            b=2*math.pi*i/sides;p=[math.cos(a)*math.cos(b),math.sin(a),math.cos(a)*math.sin(b)]
            points.append(p);normals.append(p);uv.append([i/sides,j/rings])
    top=len(points);points.append([0,1,0]);normals.append([0,1,0]);uv.append([.5,1])
    for i in range(sides):
        n=(i+1)%sides;faces.append([0,1+i,1+n]);faces.append([top,1+(rings-2)*sides+n,1+(rings-2)*sides+i])
    for j in range(rings-2):
        for i in range(sides):
            n=(i+1)%sides;a=1+j*sides;faces.append([a+i,a+sides+i,a+sides+n,a+n])
    return mesh_record(mid,name,points,normals,faces,material,uv)

def rounded_box(mid,name,half,radius,material):
    half=np.asarray(half,float);points=[];normals=[];faces=[];uv=[]
    frames=[([1,0,0],[0,0,-1],[0,1,0]),([-1,0,0],[0,0,1],[0,1,0]),
            ([0,1,0],[1,0,0],[0,0,-1]),([0,-1,0],[1,0,0],[0,0,1]),
            ([0,0,1],[1,0,0],[0,1,0]),([0,0,-1],[-1,0,0],[0,1,0])]
    for normal,u,v in frames:
        normal=np.array(normal);u=np.array(u);v=np.array(v);hu=float(abs(u)@half);hv=float(abs(v)@half)
        xs=[-hu,-hu+radius,hu-radius,hu];ys=[-hv,-hv+radius,hv-radius,hv];start=len(points)
        for j,y in enumerate(ys):
            for i,x in enumerate(xs):
                p=normal*half+u*x+v*y;q=np.clip(p,-half+radius,half-radius);n=p-q;n/=np.linalg.norm(n)
                points.append(q+n*radius);normals.append(n);uv.append([i/3,j/3])
        for j in range(3):
            for i in range(3):
                a=start+j*4+i;faces.append([a,a+1,a+5,a+4])
    return mesh_record(mid,name,points,normals,faces,material,uv)

def cloth(mid,material):
    points=[];normals=[];faces=[];uv=[];w=24;h=32
    for j in range(h+1):
        t=j/h
        for i in range(w+1):
            u=i/w;angle=6*math.pi*u;points.append([.78*(u-.5),.97*(1-t),.07*math.sin(angle)*t])
            du=np.array([.78,0,.42*math.pi*math.cos(angle)*t]);dt=np.array([0,-.97,.07*math.sin(angle)])
            normals.append(np.cross(dt,du));uv.append([u,t])
    for j in range(h):
        for i in range(w):
            a=j*(w+1)+i;faces.append([a,a+w+1,a+w+2,a+1])
    return mesh_record(mid,'Драпировка · статические складки',points,normals,faces,material,uv)

def build():
    p=json.loads((ROOT/'examples/starter-character.artforge').read_text())
    p.update(version=42,title='Ателье орбит',fps=30,duration=12,clips=[])
    nodes={n['id']:n for n in p['nodes']};nodes[1]['name']='Манекен · центр экспозиции'
    nodes[1]['transform']['translation']=[0,.48,0]
    for joint,angle in [(7,-.28),(8,-.36),(10,.30),(11,1.05)]:nodes[joint]['transform']['rotation']=quaternion([0,0,1],angle)
    next_id=5000
    def ident():
        nonlocal next_id
        next_id+=1;return next_id
    def material(name,color,**values):
        m=dict(id=ident(),name=name,base_color=color,metallic=0.,roughness=.45,emissive=[0.,0.,0.],base_color_texture=None)
        m.update(values);p['materials'].append(m);return m['id']
    ivory=material('Подиумы · тёплый камень',[.65,.62,.52,1],roughness=.62)
    stage=material('Сцена · тёмный бирюзовый',[.025,.072,.085,1],roughness=.7)
    teal=material('Керамика · бирюза',[.035,.30,.32,1],roughness=.27)
    gold=material('Латунь',[.68,.37,.09,1],metallic=.85,roughness=.3)
    glass=material('Янтарное стекло',[.92,.38,.055,1],transmission=.86,ior=1.47,roughness=.13)
    fabric=material('Синяя ткань',[.045,.12,.26,1],roughness=.83,double_sided=True)
    neon=material('Бирюзовый световой кант',[.02,.65,.55,1],emissive=[.04,1.7,1.3])
    orange=material('Спутник · коралл',[.8,.13,.035,1],roughness=.4)
    lacquer=material('Прозрачный лак',[.7,.9,1,.12],roughness=.08,alpha_mode='Blend')
    def put(mesh):p['meshes'].append(mesh);return mesh['id']
    def node(name,mesh=None,position=(0,0,0),scale=(1,1,1),rotation=(0,0,0,1),**values):
        n=dict(id=ident(),name=name,parent=None,transform=dict(translation=list(position),rotation=list(rotation),scale=list(scale)),
               rest_transform=None,mesh=mesh,skin=None,morph_weights=[],visible=True,render_visible=True,locked=False,is_joint=False)
        n.update(values);p['nodes'].append(n);return n
    node('Сцена · круглая платформа',put(lathe(ident(),'Платформа',[(0,3.25),(.03,3.3),(.2,3.3),(.23,3.25)],stage)))
    node('Центральный подиум',put(lathe(ident(),'Подиум манекена',[(0,.96),(.03,1.0),(.2,1.0),(.24,.98),(.25,.96)],ivory)),(0,.23,0))
    side=put(lathe(ident(),'Общий боковой постамент',[(0,.43),(.03,.47),(.7,.47),(.74,.45),(.75,.43)],ivory))
    node('Постамент керамики',side,(-1.85,.23,0));node('Постамент стекла',side,(1.65,.23,0))
    node('Световой кант',put(torus(ident(),'Кант платформы',3.26,.018,neon)),(0,.23,0))
    node('Золотая арка',put(torus(ident(),'Арка',1.62,.045,gold,96,10)),(0,1.93,-.62),rotation=quaternion([1,0,0],math.pi/2))
    cube=rounded_box(ident(),'Керамический куб',[.43,.44,.43],.065,teal)
    cube['topology']['face_materials']=[gold if 36<=i<45 else None for i in range(54)]
    node('Керамика · полигоны и слой лака',put(cube),(-1.85,1.42,0),material_layers=[dict(material=lacquer,visible=True)])
    node('Янтарная сфера',put(sphere(ident(),'Стеклянная сфера',glass)),(1.65,1.48,0),(.5,.5,.5))
    rings=put(torus(ident(),'Кольца сферы',.61,.016,gold))
    node('Кольцо сферы · наклон',rings,(1.65,1.48,0),rotation=quaternion([1,0,0],.55))
    node('Кольцо сферы · меридиан',rings,(1.65,1.48,0),rotation=quaternion([0,0,1],1.25))
    node('Драпировка',put(cloth(ident(),fabric)),(-1.45,.42,1.18))
    posts=put(lathe(ident(),'Общие стойки ткани',[(0,.012),(1.16,.012)],gold,12))
    node('Стойка ткани · левая',posts,(-1.84,.23,1.18));node('Стойка ткани · правая',posts,(-1.06,.23,1.18))
    node('Перекладина ткани',put(lathe(ident(),'Перекладина',[(0,.018),(.78,.018)],gold,12)),(-1.84,1.39,1.18),rotation=quaternion([0,0,1],-math.pi/2))
    curve=deepcopy(json.loads((ROOT/'examples/curve-follow.artforge').read_text())['nodes'][0]['curve'])
    curve['points']=[[v*3.5/3 for v in point] for point in curve['points']]
    path=node('Общая орбита · 12 секунд',position=(0,1.65,0),curve=curve)
    def binding(phase):return dict(curve=path['id'],position=phase,follow=True,speed=1/12,repeat=True)
    moon=node('Спутник на орбите',put(sphere(ident(),'Сфера спутника',orange,24,12)),scale=(.12,.12,.12),curve_binding=binding(.125))
    camera=node('Орбитальная камера',position=(0,.6,0),rotation=qmul(quaternion([0,1,0],-math.pi/2),quaternion([1,0,0],-.21)),
                camera=dict(fov_y_radians=math.radians(55)),curve_binding=binding(.125))
    def light(name,kind,position,direction,color,intensity,**values):
        a=dict(name=name,kind=kind,position=position,direction=direction,color=color,intensity=intensity,range=0.,enabled=True,casts_shadow=True);a.update(values);return a
    p['lights']=[light('Тёплый софтбокс','Area',[2.5,4,3],[-.45,-.7,-.55],[1,.85,.65],65,area_size=[3,3]),
                 light('Холодный заполняющий','Directional',[0,3,0],[.6,-.6,-.5],[.55,.72,1],1),
                 light('Прожектор на орбите','Spot',[0,1.2,0],[0,0,-1],[.3,.8,1],28,inner_cone_angle=.45,outer_cone_angle=.8,
                       curve_binding=binding(.6),target=dict(node=1,local_center=[0,1.0,0]))]
    p['camera'].update(eye=[5.4,3.9,7.2],target=[0,1.3,0],fov_y_radians=math.radians(40))
    p['active_camera']=None
    p['render_settings']=dict(background_mode='Solid',background_color=[.025,.035,.043],ambient_intensity=.3,shadows=True,
                              shadow_resolution=1024,antialiasing=4,ray_reflections=False,ray_shadows=False,output_width=1400,output_height=1050)
    orbit=[path['id'],moon['id'],camera['id']]
    def collection(name,members,lights):return dict(id=ident(),name=name,members=members,light_members=lights,visible=True,render_visible=True,locked=False)
    p['collections']=[collection('Манекен',[1],[]),collection('Экспозиция',[n['id'] for n in p['nodes'] if n['id']>5000 and n['id'] not in orbit],[]),
                      collection('Орбита',orbit,[2]),collection('Свет',[],[0,1])]
    return p

if __name__=='__main__':
    p=build();path=ROOT/'examples/orbit-atelier.artforge';path.write_text(json.dumps(p,ensure_ascii=False,indent=2)+'\n')
    print(path);print(len(p['nodes']),'nodes;',len(p['meshes']),'meshes;',sum(len(m['vertices']) for m in p['meshes']),'vertices')
