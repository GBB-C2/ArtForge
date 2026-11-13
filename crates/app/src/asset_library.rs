//! Anchored asset browser. Browsing is transient; a card dispatches one existing command.
use super::*;
use crate::ui::{DropdownKind, Hit};
use poseforge_core::{FxKind, FxSettings, Material, MaterialPreset, Node, PrimitiveKind, PrimitiveSettings};
use super::asset_thumbnails::AssetImage;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Focus { #[default] Search, Categories, Cards }
#[derive(Default)]
pub(super) struct AssetLibrary {
    pub category: u8,
    pub query: String,
    pub anchor: [f32; 4],
    candidate: usize,
    offset: usize,
    category_offset: usize,
    filtered_query: String,
    focus: Focus,
    dragging: Option<u8>,
}
pub(super) const FX_CATEGORY:u8=7;
const CATEGORIES: [(&str, Icon); 8] = [
    ("Примитивы", Icon::Object), ("Материалы", Icon::Material), ("Кривые", Icon::Curve),
    ("Свет", Icon::Light), ("Сцены", Icon::Scene), ("Камеры", Icon::Camera), ("Скелеты", Icon::Bone),
    ("FX", Icon::Fx),
];
const GAP: f32 = 8.;
const CARD_HEIGHT: f32 = 132.;
#[derive(Clone)]
enum Thumbnail { Image(AssetImage), Material(u64), Preset(usize), Open }
#[derive(Clone)]
struct Entry { name: String, note: &'static str, icon: Icon, action: Action, thumbnail: Thumbnail }
#[derive(Clone, Copy)]
struct Layout { panel: [f32; 4], sidebar: [f32; 4], grid: [f32; 4], cols: usize, rows: usize, card_width: f32 }
impl Layout {
    fn new(size: [f32; 2], anchor: [f32; 4]) -> Self {
        let width = 752_f32.min((size[0] - 16.).max(240.));
        let height = 560_f32.min((size[1] - 16.).max(288.));
        let x = (anchor[0] - width - 8.).clamp(8., (size[0] - width - 8.).max(8.));
        let y = anchor[1].clamp(8., (size[1] - height - 8.).max(8.));
        let side = (width * 0.23).clamp(104., 148.);
        let sidebar = [x + 8., y + 100., side - 16., height - 128.];
        let grid = [x + side + 12., y + 100., width - side - 36., height - 128.];
        let cols = ((grid[2] + GAP) / 134.).floor().max(1.) as usize;
        let rows = ((grid[3] + GAP) / (CARD_HEIGHT + GAP)).floor().max(1.) as usize;
        Self { panel: [x,y,width,height], sidebar, grid, cols, rows, card_width: (grid[2] - GAP * (cols - 1) as f32) / cols as f32 }
    }
    fn page(self) -> usize { self.cols * self.rows }
    fn category_page(self) -> usize { (self.sidebar[3] / 34.).floor().max(1.) as usize }
    fn max_offset(self, count: usize) -> usize { count.div_ceil(self.cols).saturating_sub(self.rows) }
    fn card(self, slot: usize) -> [f32; 4] {
        [self.grid[0] + (slot % self.cols) as f32 * (self.card_width + GAP),
         self.grid[1] + (slot / self.cols) as f32 * (CARD_HEIGHT + GAP), self.card_width, CARD_HEIGHT]
    }
    fn track(self) -> [f32; 4] { [self.grid[0] + self.grid[2] + 5., self.grid[1], 12., self.rows as f32 * (CARD_HEIGHT + GAP) - GAP] }
}
impl App {
    fn asset_query(&self) -> String {
        self.input.as_ref().filter(|i| i.purpose == Action::AssetSearch).map_or(self.assets.query.as_str(), |i| i.value.as_str()).trim().to_lowercase()
    }
    fn asset_entries(&self) -> Vec<Entry> {
        let mut entries = Vec::new(); let query = self.asset_query();
        let mut push = |name: String, note, icon, action, thumbnail, keywords: &str| {
            if format!("{name} {keywords}").to_lowercase().contains(&query) { entries.push(Entry { name, note, icon, action, thumbnail }); }
        };
        match self.assets.category {
            0 => for (index, kind) in PrimitiveKind::MESH_MENU {
                let name=PrimitiveSettings::new(kind).label();let icon=super::primitive_controls::primitive_icon(kind);
                push(name.into(), "Добавить", icon, Action::Studio(ui::StudioAction::Mesh(index)), Thumbnail::Image(AssetImage::Primitive(index)), "меш примитив");
            },
            1 => for (i, preset) in MaterialPreset::ALL.into_iter().enumerate() {
                push(preset.label().into(), "Создать материал", Icon::Material, Action::AssetPreset(i), Thumbnail::Preset(i), "шаблон материал");
            },
            2 => {
                for (i, preset) in poseforge_core::CurvePreset::BASIC.into_iter().enumerate() {
                    push(preset.label().into(), "Добавить кривую", Icon::Curve, Action::Curve(ui::CurveAction::Create(i as u8)), Thumbnail::Image(AssetImage::Curve(preset.curve())), "кривая");
                }
                push("Нарисовать кривую".into(), "Рисовать в сцене", Icon::Curve, Action::Curve(ui::CurveAction::Draw),
                    Thumbnail::Image(AssetImage::Curve(poseforge_core::CurvePreset::Bezier.curve())), "от руки");
            }
            3 => for (i, (name, icon)) in [("Солнце",Icon::Sun),("Точка",Icon::Light),("Прожектор",Icon::Spot),("Площадной",Icon::Area)].into_iter().enumerate() {
                push(name.into(), "Добавить свет", icon, Action::SurfaceAddLight(i), Thumbnail::Image(AssetImage::Light(super::asset_thumbnails::example_light(i))), "освещение источник");
            },
            4 => {
                push("Студия примитивов".into(), "Добавить сцену", Icon::Scene, Action::AssetScene(0), Thumbnail::Image(AssetImage::Scene(0)), "сцена");
                push("Шарнирный манекен".into(), "Добавить сцену", Icon::Scene, Action::AssetScene(1), Thumbnail::Image(AssetImage::Scene(1)), "сцена персонаж");
                push("Ателье орбит".into(), "Добавить сцену", Icon::Scene, Action::AssetScene(2), Thumbnail::Image(AssetImage::Scene(2)), "сцена свет материалы кривые манекен");
                push("Открыть сцену…".into(), "Выбрать файл", Icon::Folder, Action::Open, Thumbnail::Open, "файл импорт");
            }
            5 => push("Камера".into(), "Добавить", Icon::Camera, Action::CreateCamera, Thumbnail::Image(AssetImage::Camera(Default::default())), "объектив"),
            6 => for (humanoid, name) in [(false,"Цепь · 3 кости"),(true,"Человек · 67 костей")] {
                push(name.into(), "Добавить скелет", Icon::Bone, Action::Author(ui::AuthorAction::CreateSkeleton(humanoid)), Thumbnail::Image(AssetImage::Skeleton(humanoid)), "кости риг");
            },
            FX_CATEGORY => for kind in FxKind::ALL {
                push(kind.label().into(),"Базовый эффект",super::fx_controls::fx_icon(kind),
                    Action::Fx(ui::FxAction::Create(kind.index() as u8)),
                    Thumbnail::Image(AssetImage::Fx(FxSettings::modern(kind))),"fx эффект базовый");
                for (preset,label) in FxSettings::preset_labels(kind).into_iter().enumerate() {
                    let settings=FxSettings::modern(kind).styled(preset as u8).unwrap();
                    push(label.into(), "Добавить в сцену", super::fx_controls::fx_icon(kind),
                        Action::Fx(ui::FxAction::CreatePreset(kind.index() as u8,preset as u8)),
                        Thumbnail::Image(AssetImage::Fx(settings)),&format!("fx эффект {}",kind.label()));
                }
            },
            _ => {}
        }
        for node in &self.editor.project.nodes {
            let image = match self.assets.category {
                0 if node.mesh.is_some() && node.curve.is_none() => Some((Icon::Object, AssetImage::Object(node.id))),
                2 if node.curve.is_some() => Some((Icon::Curve, AssetImage::Curve(node.curve.clone().unwrap()))),
                5 if node.camera.is_some() => Some((Icon::Camera, AssetImage::Camera(node.camera.unwrap()))),
                6 if node.is_joint && node.parent.and_then(|p| self.editor.project.node(p)).is_none_or(|n| !n.is_joint) => Some((Icon::Bone, AssetImage::Rig(node.id))),
                FX_CATEGORY if node.fx.is_some() => Some((super::fx_controls::fx_icon(node.fx.unwrap().kind),AssetImage::Fx(node.fx.unwrap()))),
                _ => None,
            };
            if let Some((icon, image)) = image { push(node.name.clone(), "Выделить в сцене", icon, Action::Select(node.id), Thumbnail::Image(image), "в сцене"); }
        }
        if self.assets.category == 3 { for (index, light) in self.editor.project.lights.iter().enumerate() {
            push(light.name.clone(), "Выделить в сцене", Icon::Light, Action::SceneSelectLight(index, Box::new(light.clone())), Thumbnail::Image(AssetImage::Light(light.clone())), "в сцене");
        } }
        if self.assets.category == 1 { for m in self.editor.project.materials.iter().filter(|m| m.in_library) {
            push(m.name.clone(), "Открыть материал", Icon::Material, Action::AssetMaterial(m.id), Thumbnail::Material(m.id), &format!("{} {}",m.category,m.tags.join(" ")));
        } }
        entries
    }
    fn asset_layout(&self) -> Layout { Layout::new([self.width,self.height], self.assets.anchor) }
    fn refresh_assets(&mut self) {
        self.assets.category = self.assets.category.min(CATEGORIES.len() as u8-1);
        let query = self.asset_query();
        if self.assets.filtered_query != query { self.assets.filtered_query = query; self.assets.offset = 0; self.assets.candidate = 0; }
        let count = self.asset_entries().len(); let layout = self.asset_layout();
        self.assets.candidate = self.assets.candidate.min(count.saturating_sub(1));
        self.assets.offset = self.assets.offset.min(layout.max_offset(count));
        self.assets.category_offset = self.assets.category_offset.min(CATEGORIES.len().saturating_sub(layout.category_page()));
    }
    pub(super) fn begin_asset_popup(&mut self, anchor: [f32; 4]) {
        self.assets.anchor = anchor; self.assets.focus = Focus::Search; self.assets.dragging = None;
        self.input(Action::AssetSearch, self.assets.query.clone()); self.refresh_assets();
        let page=self.asset_layout().category_page();let category=self.assets.category as usize;
        if category<self.assets.category_offset {self.assets.category_offset=category;}
        if category>=self.assets.category_offset+page {self.assets.category_offset=category+1-page;}
    }
    pub(super) fn close_asset_popup(&mut self) {
        if !self.dropdown_is_open(DropdownKind::AssetLibrary) { return; }
        if let Some(input) = self.input.as_ref().filter(|i| i.purpose == Action::AssetSearch) { self.assets.query = input.value.clone(); }
        if self.input.as_ref().is_some_and(|i| i.purpose == Action::AssetSearch) { self.end_input(); }
        self.assets.dragging = None; self.dropdown = None; self.dropdown_focus = Some(DropdownKind::AssetLibrary);
    }
    fn asset_focus(&mut self, focus: Focus) {
        if focus == Focus::Search { self.input(Action::AssetSearch, self.assets.query.clone()); }
        else if let Some(input) = self.input.as_ref().filter(|i| i.purpose == Action::AssetSearch) {
            self.assets.query = input.value.clone(); self.end_input();
        }
        self.assets.focus = focus;
    }
    pub(super) fn choose_asset(&mut self, index: usize) {
        let action = self.asset_entries().get(index).map(|e| e.action.clone());
        self.close_asset_popup();
        if self.job.is_some() || self.native_picker.is_some() || self.editor.has_staged() { return; }
        if let Some(action) = action { self.action(action); }
    }
    pub(super) fn draw_asset_popup(&mut self, u: &mut Ui) {
        if self.job.is_some() || self.native_picker.is_some() { self.close_asset_popup(); return; }
        self.refresh_assets(); let layout = self.asset_layout(); let [x,y,w,h] = layout.panel; let p = u.palette();
        let physical = layout.panel.map(|v| v * u.scale);
        u.texts.retain(|t| t.x >= physical[0]+physical[2] || t.x+t.text.chars().count() as f32*t.size <= physical[0]
            || t.y >= physical[1]+physical[3] || t.y+t.size*1.5 <= physical[1]);
        u.hits.clear(); u.hits.push(Hit { rect: [0.,0.,self.width,self.height], action: Action::DropdownDismiss });
        u.surface([x+3.,y+4.,w,h],[0.,0.,0.,0.2]); u.outlined_surface(layout.panel,p.panel,p.border);
        Icon::Library.draw(u,[x+14.,y+12.,20.,20.],p.text);
        u.label(x+42.,y+14.,"Библиотека",15.,p.text);
        u.scene_close_button([x+w-36.,y+7.,28.,28.],Action::DropdownDismiss);
        Icon::Search.draw(u,[x+16.,y+48.,18.,18.],p.muted);
        let query = self.input.as_ref().filter(|i| i.purpose == Action::AssetSearch).map_or(self.assets.query.as_str(),|i| i.value.as_str());
        u.field([x+40.,y+40.,w-52.,32.],if query.is_empty() { "Поиск в выбранной категории…" } else { query },Action::AssetSearch);
        u.label(x+16.,y+81.,"Категории",10.,p.muted);
        u.label(layout.grid[0],y+81.,CATEGORIES[self.assets.category as usize].0,10.,p.muted);
        u.rect([layout.grid[0]-12.,y+80.,1.,h-106.],p.border);
        let cat_page = layout.category_page();
        for (i,(name,icon)) in CATEGORIES.iter().enumerate().skip(self.assets.category_offset).take(cat_page) {
            let rect = [layout.sidebar[0],layout.sidebar[1]+(i-self.assets.category_offset) as f32*34.,layout.sidebar[2],30.];
            u.quiet_icon_button(rect,name,Action::AssetCategory(i as u8),self.assets.category == i as u8,*icon);
        }
        if cat_page < CATEGORIES.len() {
            u.vertical_scrollbar([layout.sidebar[0]+layout.sidebar[2]-3.,layout.sidebar[1],5.,layout.sidebar[3]],
                self.assets.category_offset as f32/(CATEGORIES.len()-cat_page) as f32,cat_page as f32/CATEGORIES.len() as f32,Action::ScrollPanel(3));
        }
        let entries = self.asset_entries(); let start = self.assets.offset*layout.cols;
        for (slot,(index,entry)) in entries.iter().enumerate().skip(start).take(layout.page()).enumerate() {
            let r = layout.card(slot); let highlighted = index == self.assets.candidate;
            u.outlined_surface(r,p.elevated,if highlighted { p.accent } else { p.border });
            u.menu_row(r,Action::DropdownChoose(index),highlighted);
            let size = 86_f32.min(r[2]-12.); let preview = [r[0]+(r[2]-size)*0.5,r[1]+5.,size,size];
            match &entry.thumbnail {
                Thumbnail::Material(id) => if let Some(m) = self.editor.project.material(*id) { self.material_previews.draw(u,preview,m,&self.editor.project.textures); },
                Thumbnail::Preset(index) => if let Some(preset) = MaterialPreset::ALL.get(*index) { self.material_previews.draw(u,preview,&preset.create(),&[]); },
                Thumbnail::Image(source) => self.material_previews.draw_asset(u,preview,source,&self.editor.project),
                Thumbnail::Open => Icon::Folder.draw(u,[preview[0]+22.,preview[1]+22.,42.,42.],p.muted),
            }
            entry.icon.draw(u,[r[0]+7.,r[1]+98.,13.,13.],p.muted);
            u.fitted_label([r[0]+24.,r[1]+98.,r[2]-30.,16.],&entry.name,11.,p.text);
            u.fitted_label([r[0]+7.,r[1]+116.,r[2]-14.,13.],entry.note,10.,p.muted);
        }
        if entries.is_empty() {
            u.fitted_label([layout.grid[0]+12.,layout.grid[1]+42.,layout.grid[2]-24.,22.],"Ничего не найдено",14.,p.text);
            u.fitted_label([layout.grid[0]+12.,layout.grid[1]+70.,layout.grid[2]-24.,34.],"Измените запрос или выберите другую категорию",11.,p.muted);
        }
        let max_offset = layout.max_offset(entries.len());
        if max_offset > 0 { u.vertical_scrollbar(layout.track(),self.assets.offset as f32/max_offset as f32,layout.rows as f32/entries.len().div_ceil(layout.cols) as f32,Action::ScrollPanel(2)); }
        u.fitted_label([x+16.,y+h-21.,w-32.,16.],&format!("{} ресурсов  ·  Enter — выбрать  ·  Tab — поиск / категории / карточки  ·  Esc — закрыть",entries.len()),10.,p.muted);
    }
    pub(super) fn prepare_asset_previews(&mut self) {
        if !self.dropdown_is_open(DropdownKind::AssetLibrary) { return; }
        self.refresh_assets(); let layout = self.asset_layout();
        for entry in self.asset_entries().into_iter().skip(self.assets.offset*layout.cols).take(layout.page()) {
            match entry.thumbnail {
                Thumbnail::Material(id) => if let Some(m) = self.editor.project.material(id) { self.material_previews.request(m,&self.editor.project.textures); },
                Thumbnail::Preset(i) => if let Some(preset) = MaterialPreset::ALL.get(i) { self.material_previews.request(&preset.create(),&[]); },
                Thumbnail::Image(source) => self.material_previews.request_asset(&source,&self.editor.project),
                Thumbnail::Open => {}
            }
        }
    }
    pub(super) fn asset_popup_pointer_down(&mut self, button: MouseButton) -> bool {
        if button != MouseButton::Left { self.close_asset_popup(); self.redraw(); return true; }
        match self.ui.hit(self.mouse) {
            Some(Action::DropdownChoose(i)) => {
                let action = self.asset_entries().get(i).map(|entry|entry.action.clone());
                if let Some(action @ (Action::AssetMaterial(_) | Action::AssetPreset(_))) = action {
                    self.close_asset_popup(); self.begin_material_drag(&action);
                } else { self.choose_asset(i); }
            }
            Some(Action::AssetCategory(c)) => { self.asset_focus(Focus::Categories); self.asset_action(Action::AssetCategory(c)); }
            Some(Action::AssetSearch) => {
                if self.assets.focus != Focus::Search { self.asset_focus(Focus::Search); }
                if let Some(rect) = self.active_field_rect() { if let Some(input) = &mut self.input {
                    input.click(rect,self.mouse,self.modifiers.shift_key()); self.text_selecting = true;
                } }
            }
            Some(Action::AssetSearchClear) => { self.assets.query.clear(); self.asset_focus(Focus::Search); }
            Some(Action::ScrollPanel(panel @ (2 | 3))) => { self.assets.dragging = Some(panel); self.asset_popup_pointer_move(); }
            Some(Action::DropdownDismiss) => {
                let [x,y,w,_] = self.asset_layout().panel;
                if !ui::contains(self.asset_layout().panel,self.mouse) || ui::contains([x+w-36.,y+7.,28.,28.],self.mouse) { self.close_asset_popup(); }
            }
            _ => {}
        }
        self.refresh_assets(); self.redraw(); true
    }
    pub(super) fn asset_popup_pointer_move(&mut self) {
        let layout = self.asset_layout(); let count = self.asset_entries().len();
        if let Some(panel) = self.assets.dragging {
            let (track,page,total) = if panel == 3 { (layout.sidebar,layout.category_page(),CATEGORIES.len()) }
                else { (layout.track(),layout.rows,count.div_ceil(layout.cols)) };
            let thumb = (track[3]*page as f32/total.max(1) as f32).clamp(28.,track[3].max(28.));
            let ratio = ((self.mouse[1]-track[1]-thumb*0.5)/(track[3]-thumb).max(1.)).clamp(0.,1.);
            let offset = (ratio*total.saturating_sub(page) as f32).round() as usize;
            if panel == 3 { self.assets.category_offset = offset; } else { self.assets.offset = offset; }
        } else if let Some(Action::DropdownChoose(i)) = self.ui.hit(self.mouse) { self.assets.candidate = i; }
    }
    pub(super) fn asset_popup_pointer_up(&mut self) { self.assets.dragging = None; }
    pub(super) fn asset_popup_scroll(&mut self, delta: f32) -> bool {
        let layout = self.asset_layout();
        if delta.is_finite() && delta != 0. && ui::contains(layout.panel,self.mouse) {
            let step = if delta.abs() < 1. { delta.signum() as isize } else { delta.round() as isize };
            if self.mouse[0] < layout.grid[0] && self.mouse[1] >= layout.sidebar[1] {
                self.assets.category_offset = (self.assets.category_offset as isize-step).clamp(0,CATEGORIES.len().saturating_sub(layout.category_page()) as isize) as usize;
            } else {
                self.assets.offset = (self.assets.offset as isize-step).clamp(0,layout.max_offset(self.asset_entries().len()) as isize) as usize;
            }
        }
        self.redraw(); true
    }
    pub(super) fn asset_popup_key(&mut self, key: &Key, shift: bool) -> bool {
        if self.input.as_ref().is_some_and(|i| !i.preedit.is_empty()) { return false; }
        self.refresh_assets(); let layout = self.asset_layout(); let count = self.asset_entries().len();
        match key {
            Key::Named(NamedKey::Escape | NamedKey::F4) => self.close_asset_popup(),
            Key::Named(NamedKey::Tab) => {
                let next = match (self.assets.focus,shift) {
                    (Focus::Search,false)|(Focus::Cards,true) => Focus::Categories,
                    (Focus::Categories,false)|(Focus::Search,true) => Focus::Cards,
                    _ => Focus::Search,
                }; self.asset_focus(next);
            }
            Key::Named(NamedKey::Enter) if self.assets.focus != Focus::Categories => { if count>0 { self.choose_asset(self.assets.candidate); } }
            Key::Named(NamedKey::ArrowDown | NamedKey::ArrowUp | NamedKey::ArrowLeft | NamedKey::ArrowRight | NamedKey::Home | NamedKey::End | NamedKey::PageDown | NamedKey::PageUp) => {
                if self.assets.focus == Focus::Search && !matches!(key,Key::Named(NamedKey::ArrowUp|NamedKey::ArrowDown|NamedKey::PageUp|NamedKey::PageDown)) { return false; }
                let forward = matches!(key,Key::Named(NamedKey::ArrowDown|NamedKey::ArrowRight|NamedKey::PageDown|NamedKey::End));
                if self.assets.focus == Focus::Categories {
                    let c = if forward { (self.assets.category+1).min(CATEGORIES.len() as u8-1) } else { self.assets.category.saturating_sub(1) };
                    self.asset_action(Action::AssetCategory(c));
                    if (c as usize) < self.assets.category_offset { self.assets.category_offset = c as usize; }
                    if c as usize >= self.assets.category_offset+layout.category_page() { self.assets.category_offset = c as usize+1-layout.category_page(); }
                } else if count>0 {
                    let step = if matches!(key,Key::Named(NamedKey::PageUp|NamedKey::PageDown)) { layout.page() }
                        else if matches!(key,Key::Named(NamedKey::ArrowLeft|NamedKey::ArrowRight)) { 1 } else { layout.cols };
                    self.assets.candidate = if *key == Key::Named(NamedKey::Home) { 0 } else if *key == Key::Named(NamedKey::End) { count-1 }
                        else if forward { (self.assets.candidate+step).min(count-1) } else { self.assets.candidate.saturating_sub(step) };
                    let row = self.assets.candidate/layout.cols;
                    if row < self.assets.offset { self.assets.offset = row; }
                    if row >= self.assets.offset+layout.rows { self.assets.offset = row+1-layout.rows; }
                }
            }
            _ => return self.assets.focus != Focus::Search,
        }
        self.redraw(); true
    }
    pub(super) fn asset_action(&mut self, action: Action) {
        match action {
            Action::AssetCategory(category) => { self.assets.category = category.min(CATEGORIES.len() as u8-1); self.assets.offset = 0; self.assets.candidate = 0; }
            Action::AssetSearch => self.input(action, self.assets.query.clone()),
            Action::AssetSearchClear => { self.assets.query.clear(); self.assets.offset = 0; self.assets.candidate = 0; }
            Action::AssetMaterial(id) => {
                if self.editor.project.material(id).is_none() { return; }
                if self.selected_camera_id().is_some() { self.clear_scene_selection(); }
                self.surface.material_library = true; self.surface.library_material = Some(id);
                self.clear_light_selection(); self.set_properties_tab(1);
                self.select_material_click(id);
            }
            Action::AssetPreset(index) => {
                if self.job.is_some() || self.editor.playing || self.editor.has_staged() { return; }
                if let Some(&preset) = MaterialPreset::ALL.get(index) {
                    let result = self.editor.create_library_material(preset);
                    if let Some(id) = self.result(result) { self.asset_action(Action::AssetMaterial(id)); self.status = "Материал добавлен в библиотеку".into(); }
                }
            }
            Action::AssetScene(index) => self.start_library_scene(index),
            _ => {}
        }
    }
}
pub(super) fn library_scene(index: u8) -> Result<Project, String> {
    match index {
        0 => primitive_studio(),
        1 => Ok(Project::demo()),
        2 => poseforge_io::load_project_bytes(include_bytes!("../../../examples/orbit-atelier.artforge"))
            .map_err(|e| format!("Сцена «Ателье орбит»: {e}")),
        _ => Err("Сцена отсутствует в библиотеке".into()),
    }
}
impl App {
    fn start_library_scene(&mut self, index: u8) {
        if self.job.is_some() || self.editor.playing || self.editor.has_staged() || self.editor.has_active_edit() { return; }
        let before = self.editor.project.clone();
        let (tx, rx) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false)); let stop = cancel.clone();
        let spawned = std::thread::Builder::new().name("library-scene".into()).spawn(move || {
            let result = (|| -> Result<_, String> {
                if stop.load(Ordering::Relaxed) { return Err("Добавление сцены отменено".into()); }
                let source = library_scene(index)?; let title = source.title.clone();
                let mut after = before.clone();
                let roots = poseforge_io::merge_project(&mut after, source).map_err(|e| e.to_string())?;
                if stop.load(Ordering::Relaxed) { return Err("Добавление сцены отменено".into()); }
                let edit = poseforge_core::PreparedProjectEdit::new(before, after).map_err(|e| e.to_string())?;
                Ok((edit, roots, title))
            })();
            if stop.load(Ordering::Relaxed) { return; }
            let _ = tx.send(match result {
                Ok((edit, roots, title)) => JobMessage::LibrarySceneReady(Box::new(edit), roots, title),
                Err(error) => JobMessage::Error(error),
            });
        });
        if let Err(error) = spawned { self.status = format!("Не удалось подготовить сцену: {error}"); return; }
        self.job = Some(Job { receiver: rx, cancel, label: "Добавление сцены".into() });
        self.status = "Подготовка сцены… · Esc — отмена".into(); self.redraw();
    }
}
pub(super) fn primitive_studio() -> Result<Project, String> {
    let mut project = Project { title: "Студия примитивов".into(), ..Default::default() };
    project.materials.push(Material { id: 1, name: "Студийный материал".into(), ..Default::default() });
    for (i, (kind, location)) in [(PrimitiveKind::Cube, [-2.5,1.,0.]), (PrimitiveKind::Sphere,[0.,1.,0.]), (PrimitiveKind::Fox,[2.5,1.,0.])].into_iter().enumerate() {
        let id = 10 + i as u64 * 2; let settings = PrimitiveSettings::new(kind);
        let mesh = settings.mesh(id + 1, Some(1)).map_err(|e| e.to_string())?;
        let mut node = Node::new(id, settings.label()); node.mesh = Some(mesh.id); node.transform.translation = Vec3::from_array(location);
        project.nodes.push(node); project.meshes.push(mesh);
    }
    let mut camera = Node::new(20, "Камера студии"); camera.camera = Some(Default::default()); camera.transform = poseforge_core::camera_transform(&project.camera);
    project.nodes.push(camera); project.validate().map_err(|e| e.to_string())?; Ok(project)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fx_toolbar_opens_searchable_library_and_one_click_creates_one_independent_effect() {
        let mut a=app();a.close_asset_popup();a.action(Action::CreateCamera);
        let before=a.editor.project.clone();let undo=a.editor.undo_len();
        a.ui=a.build_ui();a.open_dropdown(DropdownKind::FxCreate);
        assert!(a.dropdown_is_open(DropdownKind::AssetLibrary));
        assert_eq!(a.assets.category,FX_CATEGORY);assert_eq!(a.asset_entries().len(),17);
        assert_eq!(a.editor.project,before);assert_eq!(a.editor.undo_len(),undo);
        a.input.as_mut().unwrap().value="кинематографичный".into();a.refresh_assets();
        assert_eq!(a.asset_entries().len(),1);
        a.ui=a.build_ui();a.mouse=a.ui.action_point(&Action::DropdownChoose(0)).unwrap();
        assert!(a.asset_popup_pointer_down(MouseButton::Left));
        assert!(a.dropdown.is_none() && a.input.is_none());
        let (id,settings)=a.selected_fx().unwrap();
        assert_eq!(a.editor.project.node(id).unwrap().name,"FX · Кинематографичный взрыв");
        assert!(settings.pyro.unwrap().cinematic_blast);
        assert!(a.editor.project.node(id).unwrap().parent.is_none());
        assert_eq!(a.editor.project.nodes[0],before.nodes[0]);
        assert_eq!(a.editor.project.active_camera,before.active_camera);
        assert_eq!(a.editor.undo_len(),undo+1);
        let bytes=serde_json::to_vec(&a.editor.project).unwrap();
        let loaded:Project=serde_json::from_slice(&bytes).unwrap();
        assert_eq!(loaded,a.editor.project);loaded.validate().unwrap();
        assert!(a.editor.undo());assert_eq!(a.editor.project,before);
    }
    #[test]
    fn fx_library_keyboard_cancel_and_existing_effect_selection_do_not_create_copies() {
        let mut a=app();a.asset_action(Action::AssetCategory(FX_CATEGORY));
        let before=a.editor.project.clone();a.input.as_mut().unwrap().value="огонь".into();a.refresh_assets();
        assert_eq!(a.asset_entries().len(),5);
        a.asset_popup_key(&Key::Named(NamedKey::Escape),false);
        assert_eq!(a.editor.project,before);assert_eq!(a.editor.undo_len(),0);
        a.assets.query.clear();a.ui=a.build_ui();a.open_dropdown(DropdownKind::FxCreate);
        a.input.as_mut().unwrap().value="мягкий пар".into();a.refresh_assets();
        a.asset_popup_key(&Key::Named(NamedKey::Enter),false);
        let id=a.selected_fx().unwrap().0;let created=a.editor.project.clone();let undo=a.editor.undo_len();
        a.ui=a.build_ui();a.open_dropdown(DropdownKind::FxCreate);
        let index=a.asset_entries().iter().position(|e|e.action==Action::Select(id)).unwrap();
        a.choose_asset(index);
        assert_eq!(a.editor.selected,Some(id));assert_eq!(a.editor.project,created);assert_eq!(a.editor.undo_len(),undo);
    }
    #[test]
    fn fx_library_category_and_cards_are_reachable_in_both_themes_and_small_windows() {
        for theme in [ui::Theme::Dark,ui::Theme::Light] {for scale in [1.,2.] {for size in [[640.,480.],[1200.,820.]] {
            let mut a=app();a.close_asset_popup();a.preferences.theme=theme;a.scale=scale;
            a.width=size[0];a.height=size[1];a.ui=a.build_ui();a.open_dropdown(DropdownKind::FxCreate);
            assert_eq!(a.assets.category,FX_CATEGORY);a.ui=a.build_ui();
            assert!(a.ui.hits.iter().any(|h|h.action==Action::AssetCategory(FX_CATEGORY)));
            let before=a.editor.project.clone();
            a.asset_popup_key(&Key::Named(NamedKey::Tab),false);
            a.asset_popup_key(&Key::Named(NamedKey::Tab),false);
            a.asset_popup_key(&Key::Named(NamedKey::End),false);
            assert_eq!(a.assets.candidate,16);
            let layout=a.asset_layout();assert!(a.assets.offset<=layout.max_offset(17));
            a.asset_popup_key(&Key::Named(NamedKey::Escape),false);
            assert_eq!(a.editor.project,before);
        }}}
    }
    fn app() -> App {
        let mut a = App::new(None); a.preferences = Preferences::default(); a.preferences.right_panel = true;
        a.preferences.animation_visible = false; a.width = 1200.; a.height = 820.;
        a.editor = Editor::new(Project::default()); a.editor.mode = EditorMode::Scene;
        a.saved_snapshot = a.editor.project.clone(); a.ui = a.build_ui();
        a.open_dropdown(DropdownKind::AssetLibrary); a.ui = a.build_ui(); a
    }
    #[test]
    fn categories_stay_left_search_stays_above_cards_and_popup_fits_window() {
        for size in [[480.,360.],[640.,480.],[960.,640.],[1200.,820.]] {
            for anchor in [[8.,8.,32.,32.],[size[0]-40.,size[1]-40.,32.,32.]] {
                let l = Layout::new(size,anchor);
                assert!(l.panel[0]>=0. && l.panel[1]>=0.);
                assert!(l.panel[0]+l.panel[2]<=size[0] && l.panel[1]+l.panel[3]<=size[1]);
                assert!(l.sidebar[0]+l.sidebar[2]<l.grid[0]);
                for slot in 0..l.page() { let r = l.card(slot);
                    assert!(r[0]>=l.grid[0] && r[0]+r[2]<=l.grid[0]+l.grid[2]+0.01);
                    assert!(r[1]+r[3]<=l.panel[1]+l.panel[3]-24.);
                }
            }
        }
        for scale in [1.,1.5,2.] { for theme in [ui::Theme::Dark,ui::Theme::Light] {
            let mut a = app(); a.scale = scale; a.preferences.theme = theme; a.ui = a.build_ui();
            let search = a.ui.hits.iter().find(|h|h.action==Action::AssetSearch).unwrap().rect;
            let card = a.ui.hits.iter().find(|h|h.action==Action::DropdownChoose(0)).unwrap().rect;
            assert!(search[1]+search[3]<card[1]);
            assert!(a.ui.hits.iter().all(|h|matches!(h.action,Action::AssetSearch|Action::AssetSearchClear|Action::AssetCategory(_)|Action::DropdownChoose(_)|Action::DropdownDismiss|Action::ScrollPanel(_))));
        } }
    }
    #[test]
    fn search_filters_live_and_cancel_preserves_selection_document_and_undo() {
        let mut a = app(); let before = a.editor.project.clone(); let selected = a.editor.selected; let undo = a.editor.undo_len();
        a.asset_action(Action::AssetCategory(2)); a.input.as_mut().unwrap().value = "окружность".into(); a.refresh_assets();
        let entries = a.asset_entries(); assert_eq!(entries.len(),1);
        assert_eq!(entries[0].action,Action::Curve(ui::CurveAction::Create(1)));
        a.input.as_mut().unwrap().value = "не существующий ресурс".into(); a.refresh_assets();
        assert!(a.asset_entries().is_empty()); assert!(a.asset_popup_key(&Key::Named(NamedKey::Enter),false));
        assert!(a.dropdown_is_open(DropdownKind::AssetLibrary));
        a.asset_popup_key(&Key::Named(NamedKey::Escape),false);
        assert!(a.input.is_none()); assert!(a.dropdown.is_none());
        assert_eq!(a.editor.project,before); assert_eq!(a.editor.selected,selected); assert_eq!(a.editor.undo_len(),undo);
    }
    #[test]
    fn one_card_creates_one_undo_step_and_outside_click_cannot_select_underneath() {
        let mut a = app(); let before = a.editor.project.clone();
        a.choose_asset(0); assert_eq!(a.editor.project.meshes.len(),1); assert_eq!(a.editor.undo_len(),1);
        assert!(a.dropdown.is_none() && a.input.is_none()); assert!(a.editor.undo()); assert_eq!(a.editor.project,before);
        a.ui = a.build_ui(); a.open_dropdown(DropdownKind::AssetLibrary); a.ui = a.build_ui();
        let selected = a.editor.selected; a.mouse = [1.,1.]; assert!(a.dropdown_pointer_down(MouseButton::Left));
        assert!(a.dropdown.is_none()); assert_eq!(a.editor.selected,selected); assert_eq!(a.editor.project,before);
    }
    #[test]
    fn wheel_resize_and_keyboard_navigation_clamp_without_moving_the_camera() {
        let mut a = app(); a.asset_action(Action::AssetCategory(1)); let distance = a.distance;
        a.mouse = [a.asset_layout().grid[0]+10.,a.asset_layout().grid[1]+10.];
        a.asset_popup_scroll(-1000.); let l = a.asset_layout();
        assert_eq!(a.assets.offset,l.max_offset(a.asset_entries().len())); assert_eq!(a.distance,distance);
        a.asset_popup_key(&Key::Named(NamedKey::Tab),false); assert_eq!(a.assets.focus,Focus::Categories);
        a.asset_popup_key(&Key::Named(NamedKey::Tab),false); assert_eq!(a.assets.focus,Focus::Cards);
        a.asset_popup_key(&Key::Named(NamedKey::End),false); assert_eq!(a.assets.candidate,a.asset_entries().len()-1);
        a.width = 640.; a.height = 480.; a.refresh_assets();
        assert!(a.assets.offset<=a.asset_layout().max_offset(a.asset_entries().len()));
        a.asset_popup_key(&Key::Named(NamedKey::Home),false); assert_eq!(a.assets.offset,0);
    }
}
