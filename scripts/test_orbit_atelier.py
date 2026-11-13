"""Data/geometry checks; these do not execute Rust or the native editor."""
import json
import unittest
from pathlib import Path
import numpy as np
from generate_orbit_atelier import build
from preview_orbit_atelier import globals_at

ROOT=Path(__file__).resolve().parents[1]

class AtelierTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):cls.scene=json.loads((ROOT/'examples/orbit-atelier.artforge').read_text())

    def test_generator_matches_saved_example(self):self.assertEqual(build(),self.scene)

    def test_links_ids_skin_and_asset_budget(self):
        p=self.scene;nodes={n['id']:n for n in p['nodes']};meshes={m['id']:m for m in p['meshes']};materials={m['id']:m for m in p['materials']}
        self.assertEqual(len(nodes),len(p['nodes']));self.assertEqual(len(meshes),len(p['meshes']));self.assertEqual(len(materials),len(p['materials']))
        self.assertLess(sum(len(m['vertices']) for m in p['meshes']),25000);self.assertLess(sum(len(m['indices'])//3 for m in p['meshes']),45000)
        self.assertEqual(sum(n.get('is_joint',False) for n in nodes.values()),17)
        for n in nodes.values():
            if n.get('parent') is not None:self.assertIn(n['parent'],nodes)
            if n.get('mesh') is not None:self.assertIn(n['mesh'],meshes)
            if n.get('curve_binding'):self.assertIsNotNone(nodes[n['curve_binding']['curve']].get('curve'))
            for layer in n.get('material_layers',[]):self.assertIn(layer['material'],materials)
            if n.get('skin'):
                skin=n['skin'];self.assertEqual(len(skin['joints']),len(skin['inverse_bind_matrices']))
                for j,ibm in zip(skin['joints'],skin['inverse_bind_matrices']):
                    self.assertTrue(nodes[j]['is_joint']);self.assertTrue(np.isfinite(ibm).all());self.assertGreater(abs(np.linalg.det(np.array(ibm).reshape(4,4))),1e-8)
        for light in p['lights']:
            if light.get('target'):self.assertIn(light['target']['node'],nodes)
            if light.get('curve_binding'):self.assertIn(light['curve_binding']['curve'],nodes)
        collections=p['collections'];self.assertEqual(len({c['id'] for c in collections}),len(collections))
        for c in collections:
            for node in c['members']:self.assertIn(node,nodes)
            for light in c['light_members']:self.assertTrue(0<=light<len(p['lights']))
        self.assertEqual(set(range(len(p['lights']))),{i for c in collections for i in c['light_members']})

    def test_meshes_have_valid_finite_attributes_and_exact_triangulation(self):
        materials={m['id'] for m in self.scene['materials']}
        for mesh in self.scene['meshes']:
            vertices=mesh['vertices'];positions=np.array([v['position'] for v in vertices]);normals=np.array([v['normal'] for v in vertices])
            self.assertTrue(np.isfinite(positions).all());self.assertTrue(np.isfinite(normals).all());self.assertTrue(np.allclose(np.linalg.norm(normals,axis=1),1,atol=1e-5))
            self.assertTrue(np.isfinite([v['uv'] for v in vertices]).all());self.assertIn(mesh['material'],materials)
            faces=mesh['topology']['faces'];assign=mesh['topology']['face_materials'];self.assertIn(len(assign),[0,len(faces)])
            self.assertTrue(all(m is None or m in materials for m in assign))
            expected=[v for f in faces for k in range(1,len(f)-1) for v in [f[0],f[k],f[k+1]]];self.assertEqual(expected,mesh['indices'])
            self.assertTrue(all(0<=v<len(vertices) for v in expected))
            tris=positions[np.array(expected).reshape(-1,3)];cross=np.cross(tris[:,1]-tris[:,0],tris[:,2]-tris[:,0])
            self.assertGreater(float(np.linalg.norm(cross,axis=1).min()),1e-10,mesh['name'])

    def test_new_geometry_winding_matches_vertex_normals(self):
        for mesh in self.scene['meshes']:
            if mesh['id']<5000:continue
            positions=np.array([v['position'] for v in mesh['vertices']]);normals=np.array([v['normal'] for v in mesh['vertices']]);ids=np.array(mesh['indices']).reshape(-1,3)
            p=positions[ids];cross=np.cross(p[:,1]-p[:,0],p[:,2]-p[:,0]);alignment=np.einsum('ij,ij->i',cross,normals[ids].mean(axis=1))
            self.assertTrue((alignment>0).all(),mesh['name'])

    def test_orbit_closes_and_camera_faces_inward(self):
        p=self.scene;camera=next(n for n in p['nodes'] if n.get('camera'));follower=next(n for n in p['nodes'] if n.get('mesh') and n.get('curve_binding'))
        first=globals_at(p,0);last=globals_at(p,12)
        self.assertTrue(np.allclose(first[camera['id']],last[camera['id']],atol=1e-5));self.assertTrue(np.allclose(first[follower['id']],last[follower['id']],atol=1e-5))
        for time in [0,3,6,9]:
            world=globals_at(p,time);m=world[camera['id']];forward=-m[:3,2];direction=np.array([0,1.4,0])-m[:3,3];direction/=np.linalg.norm(direction)
            self.assertGreater(float(forward@direction),.98);self.assertTrue(all(np.isfinite(x).all() for x in world.values()))

if __name__=='__main__':unittest.main()
