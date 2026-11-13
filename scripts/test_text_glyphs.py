"""Geometry/data regressions for the bundled glyphs; does not execute Rust."""
from pathlib import Path
from collections import Counter
import json, unittest, math
from generate_text_glyphs import area,inside,cross
ROOT=Path(__file__).resolve().parents[1]
class TextGlyphTests(unittest.TestCase):
    asset='text_glyphs.json'
    @classmethod
    def setUpClass(cls):cls.glyphs=json.loads((ROOT/'crates/core/assets'/cls.asset).read_text())['glyphs']
    def test_russian_latin_digits_and_visible_defaults_are_available(self):
        for ch in 'ТекстОбъектЁёжийABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789':self.assertIn(ch,self.glyphs)
        self.assertTrue(self.glyphs[' ']['advance']>0);self.assertFalse(self.glyphs[' ']['triangles'])
    def test_caps_match_contour_area_and_winding(self):
        for ch,g in self.glyphs.items():
            points=g['points'];rings=[[points[i] for i in r] for r in g['contours']]
            expected=sum(area(r) for r in rings);actual=0
            for t in g['triangles']:
                a,b,c=[points[i] for i in t];v=cross(a,b,c)/2;self.assertGreater(v,0,ch);actual+=v
                center=((a[0]+b[0]+c[0])/3,(a[1]+b[1]+c[1])/3)
                self.assertEqual(sum(inside(center,r) for r in rings)%2,1,ch)
            self.assertAlmostEqual(expected,actual,places=6,msg=ch)
    def test_boundary_edges_form_closed_extrusions(self):
        for ch,g in self.glyphs.items():
            edges=Counter()
            def face(ids):
                for a,b in zip(ids,ids[1:]+ids[:1]):edges[tuple(sorted((a,b)))]+=1
            n=len(g['points'])
            for a,b,c in g['triangles']:face([a,b,c]);face([c+n,b+n,a+n])
            for ring in g['contours']:
                for a,b in zip(ring,ring[1:]+ring[:1]):face([a,a+n,b+n,b])
            self.assertTrue(all(v==2 for v in edges.values()),ch)
    def test_font_data_is_finite_and_bounded(self):
        for ch,g in self.glyphs.items():
            self.assertTrue(math.isfinite(g['advance']));self.assertLessEqual(len(g['points']),1500,ch)
            self.assertTrue(all(math.isfinite(v) and abs(v)<3 for p in g['points'] for v in p),ch)
            self.assertTrue(all(0<=i<len(g['points']) for t in g['triangles'] for i in t),ch)
    def test_bevel_offsets_preserve_counters_and_winding(self):
        for ch,g in self.glyphs.items():
            self.assertEqual(len(g['points']),len(g['bevel_offsets']),ch)
            self.assertTrue(0<=g['bevel_limit']<=0.04,ch)
            for r in g['contours']:
                old=[g['points'][i] for i in r]
                new=[[g['points'][i][k]+g['bevel_offsets'][i][k]*g['bevel_limit'] for k in (0,1)] for i in r]
                self.assertGreater(area(old)*area(new),0,ch)
    def test_beveled_shell_is_closed_and_has_consistent_triangle_winding(self):
        import numpy as np
        for ch in 'ТекстОB8Ё@':
            g=self.glyphs[ch];radius=min(0.02,g['bevel_limit']);segments=3;edges=Counter()
            p=np.asarray(g['points']);o=np.asarray(g['bevel_offsets'])
            def ring(i,t,sign):return (*list(p[i]+o[i]*radius*math.sin(t)),sign*(0.04-radius*(1-math.cos(t))))
            def face(points):
                points=[tuple(round(v,9) for v in p) for p in points]
                for a,b in zip(points,points[1:]+points[:1]):edges[tuple(sorted((a,b)))]+=1
                a,b,c=map(np.asarray,points[:3]);self.assertGreater(np.linalg.norm(np.cross(b-a,c-a)),1e-14,ch)
            for t in g['triangles']:
                face([(*p[i],0.04) for i in t]);face([(*p[i],-0.04) for i in t[::-1]])
            for contour in g['contours']:
                for a,b in zip(contour,contour[1:]+contour[:1]):
                    face([ring(a,math.pi/2,-1),ring(b,math.pi/2,-1),ring(b,math.pi/2,1),ring(a,math.pi/2,1)])
                    for sign in [1,-1]:
                        for step in range(segments):
                            t0=step/segments*math.pi/2;t1=(step+1)/segments*math.pi/2
                            q=[ring(a,t1,sign),ring(b,t1,sign),ring(b,t0,sign),ring(a,t0,sign)]
                            face(q if sign>0 else q[::-1])
            self.assertTrue(all(v==2 for v in edges.values()),ch)
class SerifTextGlyphTests(TextGlyphTests):asset='text_glyphs_serif.json'
class MonoTextGlyphTests(TextGlyphTests):asset='text_glyphs_mono.json'
if __name__=='__main__':unittest.main()
