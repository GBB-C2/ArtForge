"""Validate the shipped mesh/skin data without a GPU or Rust toolchain."""
from collections import Counter
import json
import math
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[1]

def sub(a, b): return [x-y for x,y in zip(a,b)]
def dot(a, b): return sum(x*y for x,y in zip(a,b))
def cross(a, b): return [a[1]*b[2]-a[2]*b[1], a[2]*b[0]-a[0]*b[2], a[0]*b[1]-a[1]*b[0]]
def identity(): return [[float(i == j) for j in range(4)] for i in range(4)]
def multiply(a, b): return [[sum(a[i][k]*b[k][j] for k in range(4)) for j in range(4)] for i in range(4)]
def point(m, p): return [sum(m[i][k]*p[k] for k in range(3))+m[i][3] for i in range(3)]

def transform(data, angle=0.):
    x,y,z,w=data['rotation']
    sx,sy,sz=data['scale']
    m=[[sx*(1-2*(y*y+z*z)),sy*2*(x*y-z*w),sz*2*(x*z+y*w),data['translation'][0]],
       [sx*2*(x*y+z*w),sy*(1-2*(x*x+z*z)),sz*2*(y*z-x*w),data['translation'][1]],
       [sx*2*(x*z-y*w),sy*2*(y*z+x*w),sz*(1-2*(x*x+y*y)),data['translation'][2]],
       [0.,0.,0.,1.]]
    r=identity();r[0][0]=r[1][1]=math.cos(angle);r[1][0]=math.sin(angle);r[0][1]=-math.sin(angle)
    return multiply(m,r)


class StarterCharacterTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.asset=json.loads((ROOT/'crates/core/assets/starter_character.json').read_text(encoding='utf-8'))
        cls.meshes=cls.asset['meshes']
        cls.project=json.loads((ROOT/'examples/starter-character.artforge').read_text(encoding='utf-8'))

    def test_finite_vertices_normals_weights_and_budget(self):
        count=sum(len(m['vertices']) for m in self.meshes)
        triangles=sum(len(m['indices'])//3 for m in self.meshes)
        self.assertTrue(5000 < count < 16000)
        self.assertLess(triangles,32000)
        for mesh in self.meshes:
            for vertex in mesh['vertices']:
                self.assertTrue(all(math.isfinite(x) for key in ('position','normal','uv','color','weights') for x in vertex[key]))
                self.assertAlmostEqual(dot(vertex['normal'],vertex['normal']),1.,places=5)
                self.assertAlmostEqual(sum(vertex['weights']),1.,places=6)
                self.assertTrue(all(0 <= x <= 1 for x in vertex['uv']+vertex['color']+vertex['weights']))
                self.assertTrue(all(0 <= j < 17 for j in vertex['joints']))
        points=[v['position'] for m in self.meshes for v in m['vertices']]
        self.assertGreater(min(p[1] for p in points),0.)
        self.assertTrue(1.9 < max(p[1] for p in points) < 2.2)

    def test_faces_indices_and_nondegenerate_triangles(self):
        for mesh in self.meshes:
            positions=[v['position'] for v in mesh['vertices']]
            expected=[]
            for face in mesh['topology']['faces']:
                self.assertIn(len(face),(3,4))
                self.assertEqual(len(face),len(set(face)))
                self.assertTrue(all(0 <= i < len(positions) for i in face))
                for i in range(1,len(face)-1):
                    triangle=[face[0],face[i],face[i+1]]
                    expected.extend(triangle)
                    a,b,c=[positions[j] for j in triangle]
                    normal=cross(sub(b,a),sub(c,a))
                    self.assertGreater(dot(normal,normal),1e-23)
            self.assertEqual(mesh['indices'],expected)

    def test_articulated_parts_are_closed_consistently_oriented_and_rigidly_bound(self):
        components=0
        for mesh in self.meshes:
            edges=Counter(); directions=Counter(); graph={}
            for face in mesh['topology']['faces']:
                for a,b in zip(face,face[1:]+face[:1]):
                    key=tuple(sorted((a,b)))
                    edges[key]+=1; directions[key]+=1 if a < b else -1
                    graph.setdefault(a,set()).add(b);graph.setdefault(b,set()).add(a)
            self.assertTrue(all(count==2 for count in edges.values()))
            self.assertTrue(all(total==0 for total in directions.values()))
            unseen=set(graph)
            while unseen:
                first=min(unseen); seen={first}; stack=[first]
                while stack:
                    for neighbour in graph[stack.pop()]-seen:
                        seen.add(neighbour);stack.append(neighbour)
                unseen-=seen; components+=1
                # Articulated shells retain shape instead of blending across the gaps.
                joints={mesh['vertices'][i]['joints'][0] for i in seen}
                self.assertEqual(len(joints),1)
                self.assertTrue(all(mesh['vertices'][i]['weights']==[1.,0.,0.,0.] for i in seen))
                volume=0.
                for face in mesh['topology']['faces']:
                    if face[0] not in seen:continue
                    a=mesh['vertices'][face[0]]['position']
                    for i in range(1,len(face)-1):
                        b=mesh['vertices'][face[i]]['position']; c=mesh['vertices'][face[i+1]]['position']
                        volume+=dot(a,cross(b,c))/6
                self.assertGreater(volume,1e-8)
        self.assertEqual(components,40)

    def test_native_example_has_materials_skin_and_current_meshes(self):
        self.assertEqual(self.project['meshes'],self.meshes)
        materials={m['id']:m for m in self.project['materials']}
        for material in materials.values():
            self.assertEqual(len(material['emissive']),3)
        for mesh in self.meshes:
            self.assertIn(mesh['material'],materials)
            slots=mesh['topology']['face_materials']
            self.assertIn(len(slots),(0,len(mesh['topology']['faces'])))
            self.assertTrue(all(slot is None or slot in materials for slot in slots))
        nodes={n['id']:n for n in self.project['nodes']}
        for node in nodes.values():
            self.assertTrue(node['parent'] is None or node['parent'] in nodes)
        for ident in (20,21,22):
            skin=nodes[ident]['skin']
            self.assertEqual(len(skin['joints']),17)
            self.assertEqual(len(skin['inverse_bind_matrices']),17)
            self.assertTrue(all(j in nodes for j in skin['joints']))

    def test_start_pose_is_t_and_animation_keeps_bind_translations(self):
        nodes={n['id']:n for n in self.project['nodes']}
        self.assertEqual(self.asset['materials'],self.project['materials'])
        for bone in self.asset['bones']:
            self.assertEqual(nodes[bone['id']]['transform']['translation'],bone['translation'])
            self.assertEqual(nodes[bone['id']]['parent'],bone['parent'])
        # Arms extend horizontally from the shoulders in the authored T pose.
        for ident in (8,9,11,12):
            self.assertAlmostEqual(nodes[ident]['transform']['translation'][1],0.)
            self.assertGreater(abs(nodes[ident]['transform']['translation'][0]),.2)
        for channel in self.project['clips'][0]['channels']:
            expected=nodes[channel['node']]['transform'][channel['property'].lower()]
            self.assertEqual(channel['keys'][0]['value'],expected)
            if channel['property']=='Translation':
                self.assertTrue(all(k['value']==expected for k in channel['keys']))
        for material in self.project['materials']:
            rgb=material['base_color'][:3]
            self.assertGreater(rgb[0],rgb[1]); self.assertGreater(rgb[1],rgb[2])

    def palette(self, angles):
        nodes={n['id']:n for n in self.project['nodes']}
        globals_={}
        def evaluate(ident):
            if ident in globals_:return globals_[ident]
            node=nodes[ident]
            parent=identity() if node['parent'] is None else evaluate(node['parent'])
            result=multiply(parent,transform(node['transform'],angles.get(ident,0.)))
            globals_[ident]=result
            return result
        skin=nodes[20]['skin'];result=[]
        for joint,flat in zip(skin['joints'],skin['inverse_bind_matrices']):
            inverse=[[flat[c*4+r] for c in range(4)] for r in range(4)]
            result.append(multiply(evaluate(joint),inverse))
        return result

    def test_bind_pose_preserves_geometry(self):
        for matrix in self.palette({}):
            for a,b in zip(sum(matrix,[]),sum(identity(),[])):
                self.assertAlmostEqual(a,b,places=5)

    def test_bent_pose_stays_finite_and_does_not_explode(self):
        for angles in ({10:.30,11:1.05},{7:1.45,8:-.6,10:-1.45,11:.6,14:-.7,17:-.9}):
            palette=self.palette(angles)
            for mesh in self.meshes:
                posed=[]
                for vertex in mesh['vertices']:
                    p=[0.,0.,0.]
                    for joint,weight in zip(vertex['joints'],vertex['weights']):
                        if weight:
                            q=point(palette[joint],vertex['position'])
                            p=[a+weight*b for a,b in zip(p,q)]
                    self.assertTrue(all(math.isfinite(x) and abs(x) < 3. for x in p))
                    posed.append(p)
                edges={tuple(sorted((a,b))) for face in mesh['topology']['faces'] for a,b in zip(face,face[1:]+face[:1])}
                for a,b in edges:
                    original=sub(mesh['vertices'][a]['position'],mesh['vertices'][b]['position'])
                    changed=sub(posed[a],posed[b])
                    # A rigidly weighted shell preserves every edge under joint motion.
                    self.assertAlmostEqual(dot(changed,changed),dot(original,original),places=9)


if __name__ == '__main__':unittest.main()
