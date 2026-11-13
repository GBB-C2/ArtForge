//! A depth-tested floor guide; no helper geometry enters the saved project.
use super::*;
use glam::Vec4;

impl App {
    pub(super) fn draw_destruction_floor(&mut self, u: &mut Ui) {
        if !self.editor.project.nodes.iter().any(|n|n.destruction.is_some_and(|d|d.floor_enabled && d.show_floor)) { return; }
        let Ok(scene)=self.evaluated_viewport() else { return; };
        let matrix=self.view().view_projection();let viewport=self.viewport();
        let mut fill=u.palette().accent;fill[3]=0.14;
        let mut edge=u.palette().accent;edge[3]=0.65;
        for node in &self.editor.project.nodes {
            let Some(d)=node.destruction.filter(|d|d.floor_enabled && d.show_floor && d.visible) else {continue;};
            if d.result.is_some_and(|id|!self.editor.project.node_visible(id,false)) {continue;}
            let Some(model)=scene.global.get(&d.result.unwrap_or(node.id)) else {continue;};
            let center=model.w_axis.truncate();let span=4.;let y=d.values[5] as f32;
            let points=[Vec3::new(center.x-span,y,center.z-span),Vec3::new(center.x+span,y,center.z-span),
                Vec3::new(center.x+span,y,center.z+span),Vec3::new(center.x-span,y,center.z+span)];
            let screen=|p:Vec4| {let p=p.truncate()/p.w;[viewport[0]+(p.x+1.)*0.5*viewport[2],viewport[1]+(1.-p.y)*0.5*viewport[3],p.z]};
            for triangle in [[0,1,2],[0,2,3]] {
                let polygon=super::face_overlay::clip_triangle(triangle.map(|i|matrix*points[i].extend(1.)));
                for i in 1..polygon.len().saturating_sub(1) {
                    u.mesh_triangle([screen(polygon[0]),screen(polygon[i]),screen(polygon[i+1])],fill,true);
                }
            }
            for i in 0..4 {
                let a=matrix*points[i].extend(1.);let b=matrix*points[(i+1)%4].extend(1.);
                if a.w>0. && b.w>0. && a.z>=0. && b.z>=0. {
                    let a=screen(a);let b=screen(b);
                    u.smooth_segment([a[0],a[1]],[b[0],b[1]],1.,edge);
                }
            }
        }
    }
}
