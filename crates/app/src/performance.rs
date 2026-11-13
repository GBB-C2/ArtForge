//! Interaction quality affects only the preview, never authored render settings.
use super::*;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum QualityStage {
    Moving,
    Settling,
    Final,
}
pub(super) struct Performance {
    changed: Instant,
    pub cpu_ms: f64,
    pub fps: f64,
    last_frame: Option<Instant>,
    pub stage: QualityStage,
}
impl Default for Performance {
    fn default() -> Self {
        Self {
            changed: Instant::now() - Duration::from_secs(2),
            cpu_ms: 0.,
            fps: 0.,
            last_frame: None,
            stage: QualityStage::Final,
        }
    }
}
impl Performance {
    pub fn changed(&mut self) {
        self.changed = Instant::now();
    }
    pub fn stage_at(elapsed: Duration, active: bool) -> QualityStage {
        if active || elapsed < Duration::from_millis(140) {
            QualityStage::Moving
        } else if elapsed < Duration::from_millis(400) {
            QualityStage::Settling
        } else {
            QualityStage::Final
        }
    }
    pub fn quality(
        &mut self,
        authored: poseforge_core::RenderSettings,
        enabled: bool,
        active: bool,
    ) -> poseforge_core::RenderSettings {
        if active {
            self.changed();
        }
        self.stage = if enabled {
            Self::stage_at(self.changed.elapsed(), active)
        } else {
            QualityStage::Final
        };
        let mut q = authored;
        match self.stage {
            QualityStage::Moving => {
                q.reflection_samples = 1;
                q.ray_reflections = false;
                q.ray_shadows = false;
                q.area_shadow_samples = 4;
                q.antialiasing = 1;
                q.shadow_resolution = q.shadow_resolution.min(512);
            }
            QualityStage::Settling => {
                q.reflection_samples = q.reflection_samples.min(4);
                q.area_shadow_samples = q.area_shadow_samples.min(16);
            }
            QualityStage::Final => {}
        }
        q
    }
    pub fn needs_refinement(&self) -> bool {
        self.stage != QualityStage::Final
    }
    pub fn finish(&mut self, start: Instant) {
        let now = Instant::now();
        let ms = now.duration_since(start).as_secs_f64() * 1000.;
        self.cpu_ms = if self.cpu_ms == 0. {
            ms
        } else {
            self.cpu_ms * 0.8 + ms * 0.2
        };
        if let Some(last) = self.last_frame {
            let dt = start.duration_since(last).as_secs_f64();
            if dt > 0. && dt < 0.5 {
                self.fps = if self.fps == 0. {
                    1. / dt
                } else {
                    0.8 * self.fps + 0.2 / dt
                };
            } else {
                self.fps = 0.;
            }
        }
        self.last_frame = Some(start);
    }
}
impl App {
    pub(super) fn draw_performance(&self, u: &mut Ui) {
        if !self.preferences.performance_overlay {
            return;
        }
        let p = u.palette();
        let x = self.left_panel_rect()[0] + self.left_panel_rect()[2] + 12.;
        let gpu = self
            .renderer
            .as_ref()
            .and_then(|r| r.gpu_milliseconds())
            .map_or("—".into(), |n| format!("{n:.1}"));
        let bytes = self
            .editor
            .project
            .textures
            .iter()
            .map(|t| t.rgba8.len())
            .sum::<usize>()
            + self
                .editor
                .project
                .meshes
                .iter()
                .map(|m| {
                    m.vertices.len() * std::mem::size_of::<poseforge_core::Vertex>()
                        + m.indices.len() * 4
                })
                .sum::<usize>();
        let stage = match self.performance.stage {
            QualityStage::Moving => "Движение",
            QualityStage::Settling => "Уточнение",
            QualityStage::Final => "Полное качество",
        };
        let y = self.height - 89.;
        u.surface([x, y, 304., 54.], p.panel);
        u.label(
            x + 8.,
            y + 7.,
            format!(
                "FPS {:.0} · CPU {:.1} мс · GPU {} мс",
                self.performance.fps, self.performance.cpu_ms, gpu
            ),
            11.,
            p.text,
        );
        u.label(
            x + 8.,
            y + 29.,
            format!("Ресурсы {:.1} МиБ · {}", bytes as f64 / 1048576., stage),
            10.,
            p.muted,
        );
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preview_refines_without_changing_authored_settings() {
        let q = poseforge_core::RenderSettings {
            reflection_samples: 16,
            ray_reflections: true,
            ray_shadows: true,
            area_shadow_samples: 64,
            antialiasing: 4,
            shadow_resolution: 2048,
            ..Default::default()
        };
        let mut p = Performance::default();
        let fast = p.quality(q, true, true);
        assert_eq!(fast.reflection_samples, 1);
        assert!(!fast.ray_reflections && !fast.ray_shadows);
        assert_eq!(fast.area_shadow_samples, 4);
        assert_eq!(q.reflection_samples, 16);
        assert_eq!(q.shadow_resolution, 2048);
        p.changed = Instant::now() - Duration::from_millis(220);
        assert_eq!(p.quality(q, true, false).area_shadow_samples, 16);
        p.changed = Instant::now() - Duration::from_secs(1);
        assert_eq!(p.quality(q, true, false), q);
        assert!(!p.needs_refinement());
        assert_eq!(p.quality(q, false, true), q);
    }
}
