use super::{App, FilePurpose, Job, JobMessage};
use crate::ui::{Action, DropdownKind, SliderRange, Ui, WorldAction};
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
};
impl App {
    pub(super) fn start_environment_load(&mut self, path: PathBuf) {
        if self.editor.project.environment_library.len()
            + usize::from(self.editor.project.environment_map.is_some())
            >= 16
        {
            self.status = "В библиотеке 16 панорам. Удалите ненужную перед загрузкой новой.".into();
            return;
        }
        if self.job.is_some() {
            self.status = "Дождитесь текущей операции или отмените её.".into();
            return;
        }
        let (sender, receiver) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let c = cancel.clone();
        std::thread::spawn(move || {
            if c.load(Ordering::Relaxed) {
                return;
            }
            let result = poseforge_io::load_environment(path);
            if !c.load(Ordering::Relaxed) {
                let _ = sender.send(match result {
                    Ok(map) => JobMessage::EnvironmentLoaded(map),
                    Err(e) => JobMessage::Error(e.to_string()),
                });
            }
        });
        self.job = Some(Job {
            receiver,
            cancel,
            label: "Загрузка HDRI".into(),
        });
        self.status = "Загрузка HDRI… Esc — отмена".into();
        self.editor.playing = false;
    }
    pub(super) fn world_action(&mut self, action: WorldAction) {
        if self.environment_library_action(action) {
            return;
        }
        if action == WorldAction::Import {
            self.environments.open = false;
            self.show_files(FilePurpose::Environment);
            return;
        }
        let r = self.editor.transact("Окружение и трассировка", |project| {
            let s = &mut project.render_settings;
            match action {
                WorldAction::Enabled => s.hdri_enabled = !s.hdri_enabled,
                WorldAction::Background => s.hdri_background = !s.hdri_background,
                WorldAction::Reflections => s.ray_reflections = !s.ray_reflections,
                WorldAction::Shadows => s.ray_shadows = !s.ray_shadows,
                WorldAction::ReflectionSamples(samples) => s.reflection_samples = samples,
                WorldAction::ShadowSamples(samples) => s.area_shadow_samples = samples,
                _ => {}
            }
        });
        self.result(r);
    }
    pub(super) fn draw_world_settings(&self, u: &mut Ui, x: f32, width: f32) {
        let p = u.palette();
        let settings = self.editor.project.render_settings;
        let half = (width - 8.) * 0.5;
        u.label(x, 803., "HDRI · ОКРУЖЕНИЕ", 11., p.muted);
        let name = self
            .editor
            .project
            .environment_map
            .as_ref()
            .map_or("Панорама не загружена", |m| {
                m.name.as_str()
            });
        u.fitted_label([x, 827., width, 18.], name, 12., p.text);
        u.button(
            [x, 852., half, 30.],
            "Загрузить",
            Action::World(WorldAction::Import),
            false,
        );
        u.button(
            [x + half + 8., 852., half, 30.],
            "Каталог",
            Action::World(WorldAction::Library),
            false,
        );
        u.checkbox(
            [x, 891., width, 26.],
            "Освещение HDRI",
            Action::World(WorldAction::Enabled),
            settings.hdri_enabled,
        );
        u.number_field(
            [x, 931., width, 30.],
            "Сила HDRI",
            settings.hdri_strength as f64,
            Action::SurfaceRenderNumber(7),
            SliderRange::new(0., 8., 0.01),
        );
        u.number_field(
            [x, 971., width, 30.],
            "Поворот HDRI, °",
            settings.hdri_rotation as f64,
            Action::SurfaceRenderNumber(8),
            SliderRange::new(-180., 180., 1.),
        );
        u.checkbox(
            [x, 1011., width, 26.],
            "Панорама на фоне",
            Action::World(WorldAction::Background),
            settings.hdri_background,
        );
        u.label(x, 1054., "ТРАССИРОВКА ГЕОМЕТРИИ", 11., p.muted);
        u.checkbox(
            [x, 1077., width, 28.],
            "Отражения сцены",
            Action::World(WorldAction::Reflections),
            settings.ray_reflections,
        );
        u.label(x, 1113., "Качество отражений", 10., p.muted);
        self.draw_dropdown_button(
            u,
            [x, 1130., width, 30.],
            DropdownKind::ReflectionSamples,
            "4 луча",
        );
        u.checkbox(
            [x, 1172., width, 28.],
            "Мягкие лучевые тени",
            Action::World(WorldAction::Shadows),
            settings.ray_shadows,
        );
        u.label(x, 1211., "Выборки площадного света", 10., p.muted);
        self.draw_dropdown_button(
            u,
            [x, 1228., width, 30.],
            DropdownKind::AreaShadowSamples,
            "16 выборок",
        );
        u.fitted_label(
            [x, 1268., width, 16.],
            "Качество ↑ · скорость ↓",
            10.,
            p.muted,
        );
    }
}
