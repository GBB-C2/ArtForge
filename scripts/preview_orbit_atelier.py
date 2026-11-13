"""CPU geometry inspection of the shipped scene; not an ArtForge GPU render."""
from pathlib import Path
import json
import numpy as np
from PIL import ImageDraw,ImageFont
from preview_starter_character import render

ROOT=Path(__file__).resolve().parents[1]

def unit(v):
    v=np.asarray(v,float);return v/max(np.linalg.norm(v),1e-12)

def matrix(t):
    x,y,z,w=t['rotation'];m=np.eye(4)
    m[:3,:3]=np.array([[1-2*(y*y+z*z),2*(x*y-z*w),2*(x*z+y*w)],
                      [2*(x*y+z*w),1-2*(x*x+z*z),2*(y*z-x*w)],
                      [2*(x*z-y*w),2*(y*z+x*w),1-2*(x*x+y*y)]])@np.diag(t['scale'])
    m[:3,3]=t['translation'];return m

def path_frame(curve,model,binding,time,negative_z):
    control=np.array(curve['points'],float);points=[];steps=curve['resolution']
    for i in range(0,len(control)-3,3):
        a,b,c,d=control[i:i+4]
        for j in range(steps):
            t=j/steps;points.append((1-t)**3*a+3*(1-t)**2*t*b+3*(1-t)*t*t*c+t**3*d)
    points.append(control[-1]);points=(np.c_[points,np.ones(len(points))]@model.T)[:,:3]
    delta=np.diff(points,axis=0);length=np.linalg.norm(delta,axis=1);segments=delta/length[:,None]
    distance=np.r_[0,np.cumsum(length)];tangents=[]
    for i in range(len(points)):
        before=segments[i-1] if i>0 else segments[-1];after=segments[i] if i<len(segments) else segments[0]
        tangents.append(unit(before+after))
    phase=binding['position']+binding['speed']*time
    phase=phase%1 if binding['repeat'] else np.clip(phase,0,1);d=phase*distance[-1]
    end=int(np.clip(np.searchsorted(distance,d),1,len(points)-1));start=end-1;t=(d-distance[start])/(distance[end]-distance[start])
    m=np.eye(4);m[:3,3]=points[start]*(1-t)+points[end]*t
    if binding['follow']:
        forward=unit(tangents[start]*(1-t)+tangents[end]*t);z=-forward if negative_z else forward
        preferred=model[:3,1];up=unit(preferred-z*(preferred@z));x=unit(np.cross(up,z));up=np.cross(z,x)
        m[:3,:3]=np.column_stack([x,up,z])
    return m

def globals_at(project,time):
    nodes={n['id']:n for n in project['nodes']};result={};active=set()
    def calculate(id):
        if id in result:return result[id]
        if id in active:raise ValueError('Cyclic node dependencies')
        active.add(id);n=nodes[id]
        if n.get('curve_binding'):
            b=n['curve_binding'];source=nodes[b['curve']]
            frame=path_frame(source['curve'],calculate(source['id']),b,time,n.get('camera') is not None)
        else:frame=calculate(n['parent']) if n.get('parent') is not None else np.eye(4)
        result[id]=frame@matrix(n['transform']);active.remove(id);return result[id]
    for id in nodes:calculate(id)
    return result

def triangles(project,time=0):
    world=globals_at(project,time);meshes={m['id']:m for m in project['meshes']};materials={m['id']:m for m in project['materials']};triangles=[]
    for node in project['nodes']:
        if node.get('mesh') is None or not node.get('visible',True):continue
        mesh=meshes[node['mesh']];vertices=mesh['vertices'];positions=np.array([v['position'] for v in vertices]);normals=np.array([v['normal'] for v in vertices]);colors=np.array([v.get('color',[1]*4)[:3] for v in vertices])
        if node.get('skin'):
            skin=node['skin'];palette=np.array([world[j]@np.array(ibm).reshape(4,4,order='F') for j,ibm in zip(skin['joints'],skin['inverse_bind_matrices'])])
            joints=np.array([v['joints'] for v in vertices]);weights=np.array([v['weights'] for v in vertices]);transforms=(palette[joints]*weights[:,:,None,None]).sum(1)
            positions=np.einsum('nij,nj->ni',transforms,np.c_[positions,np.ones(len(positions))])[:,:3]
            normals=np.einsum('nij,nj->ni',transforms[:,:3,:3],normals)
        else:
            model=world[node['id']];positions=(np.c_[positions,np.ones(len(positions))]@model.T)[:,:3]
            normals=normals@np.linalg.inv(model[:3,:3])
        normals/=np.maximum(np.linalg.norm(normals,axis=1)[:,None],1e-12)
        assigned=mesh['topology']['face_materials']
        for i,face in enumerate(mesh['topology']['faces']):
            painted=assigned[i] if assigned else None;material=materials[painted or mesh['material']];color=np.array(material['base_color'][:3])
            if painted is None:
                for layer in node.get('material_layers',[]):
                    m=materials[layer['material']]
                    if layer.get('visible',True) and m.get('visible',True):
                        a=m['base_color'][3];color=color*(1-a)+np.array(m['base_color'][:3])*a
            for k in range(1,len(face)-1):
                ids=[face[0],face[k],face[k+1]];triangles.append((positions[ids],normals[ids],colors[ids]*color))
    return triangles

if __name__=='__main__':
    project=json.loads((ROOT/'examples/orbit-atelier.artforge').read_text())
    image=render(triangles(project),1400,1050,project['camera']['eye'],project['camera']['target'],5.4,background=project['render_settings']['background_color'])
    draw=ImageDraw.Draw(image);title=ImageFont.truetype('DejaVuSans.ttf',24);font=ImageFont.truetype('DejaVuSans.ttf',15)
    draw.rectangle((0,0,1400,62),fill='#172a33');draw.text((28,20),'АТЕЛЬЕ ОРБИТ  /  ArtForge',fill='#e5eeee',font=title)
    draw.rectangle((0,994,1400,1050),fill='#172a33')
    draw.text((28,1008),'Программный просмотр геометрии · не снимок ArtForge. Свет и стекло в приложении не проверены.',fill='#bccbd0',font=font)
    path=ROOT/'docs/orbit-atelier-preview.png';image.save(path);print(path)
