//! Collapsing moves paint, hit targets, hover coordinates and scroll bounds together.
use super::*;

const HEADER: f32 = 28.;
#[derive(Clone)]
pub(super) struct Section {
    pub id: u16,
    title: &'static str,
    start: f32,
    end: f32,
    skip: f32,
    output: f32,
    closed: bool,
    gaps: Vec<(f32, f32)>,
}
impl Section {
    fn removed_before(&self, y: f32) -> f32 {
        self.gaps.iter().map(|&(start, end)| (y - start).clamp(0., end - start)).sum()
    }
    fn height(&self) -> f32 { self.end - self.start - self.skip - self.removed_before(self.end) }
    fn map(&self, y: f32) -> Option<f32> {
        (!self.closed && y >= self.start + self.skip && y < self.end
            && !self.gaps.iter().any(|&(start, end)| y >= start && y < end))
            .then_some(self.output + HEADER + y - self.start - self.skip - self.removed_before(y))
    }
    fn unmap(&self, y: f32) -> Option<f32> {
        let top = self.output + HEADER;
        if self.closed || y < top || y >= top + self.height() { return None; }
        let mut source = y - top + self.start + self.skip;
        for &(start, end) in &self.gaps { if source >= start { source += end - start; } }
        Some(source)
    }
}

impl App {
    fn property_empty_ranges(&self) -> Vec<(f32, f32)> {
        // Remove only known unoccupied ranges, retaining a 12 px gap. Both
        // drawing and pointer coordinates use the same reversible mapping.
        if self.surface.tab == 0 {
            if let Some(light) = self.surface_light_index().filter(|_| self.active_light_index().is_some())
                .and_then(|index| self.editor.project.lights.get(index)) {
                let mut gaps = match light.kind {
                    poseforge_core::LightKind::Point => vec![(816., 1026.)],
                    poseforge_core::LightKind::Directional => vec![(816., 934.)],
                    poseforge_core::LightKind::Spot => vec![(896., 934.)],
                    poseforge_core::LightKind::Area => vec![],
                };
                if light.kind != poseforge_core::LightKind::Point && light.target.is_none() { gaps.push((974., 1026.)); }
                return gaps;
            }
        }
        vec![]
    }

    pub(super) fn property_sections(&self) -> Vec<Section> {
        let bottom = self.inspector_content_bottom();
        let (base, mut starts): (u16, Vec<(f32, &'static str, f32)>) = match self.surface.tab {
            1 => {
                let shift = self.material_controls_shift();
                let extra = super::material_controls::EMISSION_EXTRA_HEIGHT;
                let glass = if self.surface.maps { 1364. } else { 1024. } + shift + extra + self.material_map_details_height();
                let mut groups = vec![(171., "Материалы", 0.)];
                if !self.surface.material_library && self.surface_mesh_id().is_none() {
                    groups.truncate(1);
                }
                if self.selected_material_id().is_some() {
                    groups.extend([
                        (353. + shift, "Поверхность", 0.),
                        (628. + shift, "Свечение", 0.),
                        (710. + shift + extra, "Прозрачность", 0.),
                        (832. + shift + extra, "Текстуры", 0.),
                        (
                            glass,
                            if self.npr_enabled() {
                                "NPR · палитра и край"
                            } else {
                                "Стекло и пропускание"
                            },
                            24.,
                        ),
                        (
                            glass + 316.,
                            if self.npr_enabled() {
                                "NPR · контур поверхности"
                            } else {
                                "Блик и ткань"
                            },
                            24.,
                        ),
                        (glass + 576., "Положение текстур · UV", 24.),
                    ]);
                    if self.npr_enabled() {
                        groups.extend([
                            (glass + 868., "NPR · палитра света", 24.),
                            (glass + 1090., "NPR · штриховка", 24.),
                            (glass + 1340., "NPR · стиль и растр", 24.),
                        ]);
                    }
                }
                if self.artistic_enabled() {
                    groups.push((self.material_legacy_bottom(), "Краска и бумага", 0.));
                }
                (100, groups)
            }
            3 => (
                300,
                vec![
                    (171., "Качество изображения", 24.),
                    (389., "Тени", 0.),
                    (642., "Контактное затенение", 28.),
                    (803., "HDRI · окружение", 24.),
                    (1054., "Отражения и мягкие тени", 22.),
                    (1310., "Освещение и окружение", 22.),
                    (1634., "Вывод изображения", 24.),
                    (1920., "Фон", 24.),
                    (2010., "NPR · экранные контуры", 24.),
                ],
            ),
            4 | 6 if self.surface.tab == 4 || self.selected_fluid().is_some() => (
                400,
                vec![
                    (171., "Физика объекта", 0.),
                    (494. + self.collision_layout_shift(poseforge_core::CollisionKind::Fluid), "Параметры жидкости", 0.),
                ],
            ),
            6 | 11 if self.editor.selected.and_then(|id|self.editor.project.node(id)).and_then(|n|n.physics).is_some_and(|s|s.kind==poseforge_core::PhysicsKind::Cloth) =>
                (600,vec![(171.,"Физика ткани",0.),(386.,"Столкновения",0.),(386.+self.cloth_collision_shift(),"Параметры симуляции",0.)]),
            6 | 11 if bottom > 400. => (
                600,
                vec![
                    (171., "Физика объекта", 0.),
                    (378., "Параметры симуляции", 0.),
                ],
            ),
            6 | 11 => (600, vec![(171., "Физика объекта", 0.)]),
            7 if self.authoring.editing.is_some() => (
                modeling_palette::TOOLS_SECTION,
                vec![(171., "Моделирование", 0.), (self.modeling_properties_start(), "Свойства", 0.)],
            ),
            7 if self.has_morph_panel() => (700, vec![(171., "Геометрия", 0.), (self.morph_panel_start(), "Морфы", 24.)]),
            7 => (700, vec![(171., "Геометрия", 0.)]),
            13 => (1700, vec![(171., "Разрушение", 0.)]),
            12 => (1600, vec![(171., "UV · выделение", 0.), (360., "Трансформация UV", 0.), (570., "Развёртка", 0.)]),
            10 => (
                1500,
                vec![
                    (171., "Сглаживание", 0.),
                    (456., "Отображение", 0.),
                    (682., "Направление", 0.),
                ],
            ),
            9 => (
                1200,
                vec![
                    (171., "Интерфейс", 0.),
                    (310., "Просмотр", 0.),
                    (500., "Импорт Blender", 0.),
                ],
            ),
            0 if self.active_light_index().is_some() => (
                900,
                vec![
                    (171., "Источник света", 24.),
                    (528., "Положение и форма", 0.),
                    (1066., "Окружение", 24.),
                    (1234., "Объёмный свет", 0.),
                    (1396., "Среда", 24.),
                ],
            ),
            0 if self.selected_camera_id().is_some() => (
                1000,
                vec![
                    (171., "Камера и трансформация", 0.),
                    (392., "Объектив и кадр", 0.),
                    (790., "Фокус и композиция", 0.),
                    (1012., "Сохранённые ракурсы", 0.),
                    (1154., "Эффекты камеры", 0.),
                ],
            ),
            0 if self.selected_curve().is_some() => (1800, vec![
                (171., "Объект и трансформация", 0.), (526., "Кривая", 0.), (850., "Геометрия кривой", 0.),
            ]),
            0 if self.selected_fx().is_some() => (2400,self.fx_property_groups()),
            0 if self.selected_helper().is_some() => (2300,vec![(171.,"Объект",0.),(526.,if self.selected_helper().unwrap().force_field.is_some() {"Силовое поле"}else{"Пустышка"},0.)]),
            0 if self.selected_text().is_some() => (2200,vec![(171.,"Объект",0.),(526.,"Текст",0.)]),
            0 if self.rig_context() => (1900, vec![
                (171., "Объект и трансформация", 0.),
                (rig_workflow::RIG_OBJECT_START, match self.authoring.rig_stage {
                    0 => "Скелет и IK", 1 => "Привязка скелета", _ => "Поза и действия",
                }, 0.),
            ]),
            0 if self.editor.selected.and_then(|id| self.editor.project.node(id)).is_some_and(|n| n.mesh.is_some()) =>
                (0, vec![(171., "Объект и трансформация", 0.), (526., "Отображение", 0.)]),
            0 => (0, vec![(171., "Объект и трансформация", 0.)]),
            _ => (1100, vec![(171., "Настройки", 0.)]),
        };
        let transform = self.surface.tab==0 && self.active_light_index().is_none() && self.selected_camera_id().is_none()
            && self.editor.selected.and_then(|id|self.editor.project.node(id)).is_some();
        if transform {starts[0].1="Объект";starts.insert(1,(240.,"Положение",0.));}
        if let Some(start) = self.path_panel_start() { starts.push((start, "Привязка к кривой", 0.)); }
        let gaps = self.property_empty_ranges();
        let mut output = 171.;
        starts
            .iter()
            .enumerate()
            .filter(|(_, s)| s.0 < bottom)
            .map(|(i, &(start, title, skip))| {
                let end = starts.get(i + 1).map_or(bottom, |s| s.0).min(bottom);
                let id = if transform && i==1 {2100}
                    else if base==600 && starts.len()==3 {match i {1=>602,2=>601,_=>600}}
                    else if base==2400 {
                        // Optional timing controls must not change saved collapse state of other FX groups.
                        match title {"Объект"=>2400,"FX"=>2401,"Форма"=>2402,"Движение"=>2403,
                            "Освещение"=>2404,"Источник"=>2405,"Силовые поля"=>2406,"Столкновения"=>2407,
                            "Время FX"=>2409,"Расчёт и качество"=>2410,"Изменение по времени"=>2411,"Горение"=>2412,"Импульс взрыва"=>2413,_=>2408}
                    }
                    else if base==100 && title=="Краска и бумага" {182}
                    else {base + i as u16 - if transform && i>1 {1} else {0}};
                let closed = self.preferences.collapsed_sections.contains(&id)
                    || (id == modeling_palette::PROPERTIES_SECTION && self.authoring.modeling_tool.is_none());
                let section = Section {
                    id,
                    title,
                    start,
                    end,
                    skip,
                    output,
                    closed,
                    gaps: gaps.iter().copied().filter(|&(a, b)| a >= start + skip && b <= end).collect(),
                };
                output += HEADER + if closed { 4. } else { section.height() };
                section
            })
            .collect()
    }
    pub(super) fn property_y(&self, y: f32) -> Option<f32> {
        self.property_sections()
            .iter()
            .find(|s| y >= s.start && y < s.end)?
            .map(y)
    }
    pub(super) fn property_cursor_y(&self, y: f32) -> f32 {
        self.property_sections().iter().find_map(|s| s.unmap(y)).unwrap_or(-10000.)
    }
    pub(super) fn collapsed_content_bottom(&self) -> f32 {
        self.property_sections().last().map_or(171., |s| {
            s.output
                + HEADER
                + if s.closed {
                    4.
                } else {
                    s.height()
                }
        })
    }
    pub(super) fn draw_property_sections(&self, u: &mut Ui, start: (usize, usize, usize)) {
        let sections = self.property_sections();
        let map = |y: f32| {
            sections
                .iter()
                .find(|s| y >= s.start && y < s.end)
                .and_then(|s| s.map(y))
        };
        for mut r in u.rects.split_off(start.0) {
            if let Some(y) = map(r.rect[1] / u.scale) {
                let delta = y * u.scale - r.rect[1];
                r.rect[1] = y * u.scale;
                if let Some(points) = &mut r.triangle {
                    for point in points { point[1] += delta; }
                }
                if let Some(line) = &mut r.segment {
                    line[1] += delta;
                    line[3] += delta;
                }
                u.rects.push(r);
            }
        }
        for mut t in u.texts.split_off(start.1) {
            if let Some(y) = map(t.y / u.scale) {
                t.y = y * u.scale;
                u.texts.push(t);
            }
        }
        for mut h in u.hits.split_off(start.2) {
            if let Some(y) = map(h.rect[1]) {
                h.rect[1] = y;
                u.hits.push(h);
            }
        }
        let [x, _, w, _] = self.properties_content_rect();
        u.set_cursor([
            self.mouse[0],
            self.mouse[1] + self.inspector_scroll + properties::CONTENT_SHIFT,
        ]);
        let p = u.palette();
        for section in sections {
            let r = [x + 4., section.output, w - 12., 25.];
            u.menu_row(r, Action::ToggleSection(section.id), false);
            // A real chevron, with the same hit target as the complete heading.
            let cx = r[0] + 9.;
            let cy = r[1] + 12.;
            if section.closed {
                u.smooth_segment([cx - 2., cy - 4.], [cx + 2., cy], 1.8, p.accent);
                u.smooth_segment([cx + 2., cy], [cx - 2., cy + 4.], 1.8, p.accent);
            } else {
                u.smooth_segment([cx - 4., cy - 2.], [cx, cy + 2.], 1.8, p.accent);
                u.smooth_segment([cx, cy + 2.], [cx + 4., cy - 2.], 1.8, p.accent);
            }
            u.section_label([r[0] + 22., r[1] + 6., r[2] - 26., 18.], section.title, 12.);
            if section.closed {
                u.rect([r[0], r[1] + 25., r[2], 1.], p.border);
            }
        }
    }
    pub(super) fn property_section_offset(&self, id: u16) -> Option<f32> {
        self.property_sections().iter().find(|s| s.id == id).map(|s| s.output - 171.)
    }

    pub(super) fn toggle_property_section(&mut self, id: u16) {
        if !self.preferences.right_panel || !self.property_sections().iter().any(|s| s.id == id) {
            return;
        }
        if id == modeling_palette::PROPERTIES_SECTION && self.authoring.modeling_tool.is_none() {
            self.authoring.modeling_tool = Some(ui::ModelingTool::Select(self.authoring.selection_tool));
            self.preferences.collapsed_sections.remove(&id);
            return;
        }
        if !self.preferences.collapsed_sections.remove(&id) {
            self.preferences.collapsed_sections.insert(id);
        }
        self.inspector_scroll = self.inspector_scroll.min(self.inspector_max_scroll());
        self.surface.scroll_offsets[self.surface.tab.min(14) as usize] = self.inspector_scroll;
        self.persist_preferences();
    }
}

#[cfg(test)]
mod object_position_tests {
    use super::*;
    #[test]
    fn position_folds_without_hiding_name_or_object_specific_settings() {
        for kind in 0..3 {let mut a=App::new(None);a.editor=Editor::new(Project::default());a.reset_workflow_selection();a.preferences=Preferences::default();a.width=1440.;a.height=900.;
            if kind==0 {a.create_primitive(0);} else if kind==1 {a.create_curve(0);} else {a.text_action(ui::TextAction::Create);}
            a.inspector_scroll=0.;let before=a.editor.project.clone();let initial=a.collapsed_content_bottom();
            assert!(a.property_y(270.).is_some());a.action(Action::ToggleSection(2100));assert!(a.property_y(270.).is_none());assert!(a.property_y(171.).is_some());assert!(a.property_y(534.).is_some());assert!(a.collapsed_content_bottom()<initial);
            assert_eq!(a.editor.project,before);a.action(Action::ToggleSection(2100));assert!(a.property_y(270.).is_some());assert_eq!(a.collapsed_content_bottom(),initial);
        }
    }
}
