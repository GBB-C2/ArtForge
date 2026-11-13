//! Interface language is a workspace preference; authored names are never changed.
use std::borrow::Cow;
#[derive(Clone,Copy,Debug,Default,PartialEq,Eq)]
pub enum Language { #[default] Russian, English }
impl Language {
    pub fn from_name(name:&str)->Self {if name=="en" {Self::English}else{Self::Russian}}
    pub fn as_str(self)->&'static str {match self {Self::Russian=>"ru",Self::English=>"en"}}
    pub fn text<'a>(self,text:&'a str)->Cow<'a,str> {
        if self==Self::Russian {return Cow::Borrowed(text);}
        if let Some((_,english))=LABELS.iter().find(|(ru,_)|*ru==text) {return Cow::Borrowed(english);}
        // Composite UI captions keep their numeric values and shortcuts intact.
        for separator in [" · "," — ",": "] {
            if text.contains(separator) {
                let parts:Vec<_>=text.split(separator).collect();
                let translated:Vec<_>=parts.iter().map(|part|self.text(part)).collect();
                if parts.iter().zip(&translated).any(|(a,b)|*a!=b.as_ref()) {return Cow::Owned(translated.join(separator));}
            }
        }
        Cow::Borrowed(text)
    }
}
const LABELS:&[(&str,&str)]=&[
    ("Выберите объект в сцене","Select an object in the scene"),("Выберите объект или сустав","Select an object or joint"),
    ("Объект и трансформация","Object and transform"),("Интерфейс","Interface"),("Просмотр","Viewport"),("Отображение","Display"),
    ("Источник света","Light source"),("Положение и форма","Position and shape"),("Среда","Medium"),
    ("Камера и трансформация","Camera and transform"),("Объектив и кадр","Lens and framing"),
    ("Фокус и композиция","Focus and composition"),("Сохранённые ракурсы","Saved views"),("Силовое поле","Force field"),
    ("Кривая","Curve"),("Геометрия кривой","Curve geometry"),("Скелет и IK","Skeleton and IK"),
    ("Привязка скелета","Rig binding"),("Поза и действия","Pose and actions"),("Физика объекта","Object physics"),
    ("Параметры жидкости","Fluid settings"),("Физика ткани","Cloth physics"),("Параметры симуляции","Simulation settings"),
    ("Морфы","Morphs"),("Прозрачность","Transparency"),("Текстуры","Textures"),("Стекло и объём","Glass and volume"),
    ("Контактное затенение","Ambient occlusion"),("Отражения и мягкие тени","Reflections and soft shadows"),
    ("Освещение и окружение","Lighting and environment"),("Вывод изображения","Image output"),("Фон","Background"),
    ("NPR · экранные контуры","NPR · screen contours"),("Трансформация UV","UV transform"),("Развёртка","UV unwrap"),
    ("Поиск · Ctrl+F","Search · Ctrl+F"),
    ("Программа","Preferences"),("Запустить","Play"),("В начало","Restart"),("Разрушение примитива","Mesh destruction"),
    ("Материалы осколков","Shard materials"),("Раздвинуть, м","Spread, m"),("Импульс, м/с","Impulse, m/s"),
    ("Гравитация, м/с²","Gravity, m/s²"),("Пол · Y, м","Floor · Y, m"),("Кадры записи · 24 кадр/с","Bake frames · 24 fps"),
    ("Контраст разрезов","Cut contrast"),("Столкновения осколков между собой","Collisions between shards"),
    ("Добавить разрушение","Add destruction"),("Разрушение назначается мешу","Assign destruction to a mesh"),
    ("Форма меша задаёт объём осколков.","The mesh defines the volume of the shards."),
    ("Замкнутый меш · в том числе вогнутый.","Closed mesh · concave shapes supported."),
    ("Язык","Language"),("Стартовый экран","Welcome"),("Последние проекты","Recent projects"),
    ("Здесь появятся открытые и сохранённые проекты","Opened and saved projects will appear here"),
    ("Esc — перейти в рабочую область","Esc — continue to the workspace"),
    ("Новый проект","New project"),("Открыть проект…","Open project…"),("Открыть проект","Open project"),
    ("Проект","Project"),("Имя проекта","Project name"),("Открыть…","Open…"),("Импорт модели…","Import model…"),
    ("Сохранить","Save"),("Сохранить как…","Save as…"),("Сохранить проект","Save project"),
    ("Сцена","Scene"),("СЦЕНА","SCENE"),("Рендер и экспорт…","Render and export…"),("Справка и клавиши","Help and shortcuts"),
    ("Отчёт последнего импорта","Last import report"),("Отчёт","Report"),("Свойства","Properties"),
    ("Отмена","Cancel"),("Продолжить","Continue"),("Закрыть","Close"),("Удалить","Delete"),("Добавить","Add"),
    ("Вставить","Paste"),("Обзор…","Browse…"),("На уровень ↑","Up one level ↑"),
    ("Импорт модели","Import model"),("Выберите модель GLB, glTF или OBJ","Choose a GLB, glTF or OBJ model"),
    ("Путь: Ctrl+V или «Обзор…» для выбора файла / папки","Path: Ctrl+V or Browse… to choose a file / folder"),
    ("Выберите файл или введите полный путь","Choose a file or enter its full path"),
    ("Проект открыт.","Project opened."),("Сначала сохраните текущий проект.","Save the current project first."),
    ("Вид","View"),("Спереди","Front"),("Сбоку","Side"),("Сверху","Top"),("Перспектива","Perspective"),
    ("Ортография","Orthographic"),("Сетка","Grid"),("Показать всё","Frame all"),("Показать выбранное","Frame selected"),
    ("Камера","Camera"),("Свет","Light"),("Текст","Text"),("Пустышка","Empty"),("Поле","Field"),("Меш","Mesh"),
    ("Кривые","Curves"),("Скелет","Skeleton"),("Объект","Object"),("Объекты","Objects"),("Папка","Folder"),
    ("Моделирование","Modeling"),("Анимация","Animation"),("Рендер","Render"),("Инструменты","Tools"),
    ("Выделение","Selection"),("Перемещение","Move"),("Вращение","Rotate"),("Масштаб","Scale"),
    ("Положение","Position"),("Поворот","Rotation"),("Трансформация","Transform"),("Привязка","Binding"),
    ("Следить","Track"),("Разово","Once"),("Цель","Target"),("Объект цели","Target object"),
    ("Материал","Material"),("Материалы","Materials"),("Материалы меша","Mesh materials"),("Поверхность","Surface"),
    ("Цвет","Color"),("Металл","Metallic"),("Шероховатость","Roughness"),("Показатель IOR","IOR"),
    ("Слой лака","Clearcoat"),("Шерох. лака","Coat roughness"),("Свечение","Emission"),("Сила свечения","Emission strength"),
    ("Объёмный свет","Volumetric light"),("Узлы","Nodes"),("Библиотека","Library"),("Избранное","Favorites"),
    ("Все","All"),("Поиск","Search"),("Поиск: Ctrl+F","Search: Ctrl+F"),("Очистить","Clear"),("Сбросить","Reset"),
    ("Сброс","Reset"),("Базовый","Base"),("NPR · рисованный материал","NPR · painted material"),
    ("Модификаторы","Modifiers"),("Новый модификатор","New modifier"),("Применить","Apply"),("Дублировать","Duplicate"),
    ("Разрушение","Destruction"),("Осколки","Shards"),("Пол","Floor"),("Столкновения","Collisions"),
    ("Пол · столкновения","Floor collisions"),("Показать пол","Show floor"),("Показывать пыль","Show dust"),
    ("Новый вариант при пересоздании","New variation on rebuild"),("Плотность пыли","Dust density"),
    ("Размер облака","Cloud size"),("Время пыли, с","Dust lifetime, s"),("Плотность","Density"),
    ("Размер","Size"),("Время жизни","Lifetime"),("Случайность","Seed"),("Число осколков","Shard count"),
    ("Количество","Count"),("Разлёт","Spread"),("Импульс","Impulse"),("Гравитация","Gravity"),
    ("Высота пола","Floor height"),("Отскок","Bounce"),("Кадры","Frames"),("Ширина швов, %","Gap width, %"),
    ("Контраст срезов","Cut contrast"),("Материал срезов","Cut material"),("Создать осколки","Create shards"),
    ("Пересоздать осколки","Rebuild shards"),("Запечь падение","Bake falling shards"),("Вернуть исходник","Restore source"),
    ("Удалить разрушение","Remove destruction"),("Воспроизвести","Play"),("Пауза","Pause"),("С начала","Restart"),
    ("Столкновения с мешами","Mesh collisions"),("Столкновения осколков","Shard collisions"),
    ("Пыль","Dust"),("Сила","Strength"),("Скорость","Speed"),("Длительность","Duration"),("Начало","Start"),
    ("Конец","End"),("Повторять","Loop"),("Сохранить PNG","Save PNG"),("Экспорт PNG","Export PNG"),
    ("Ширина","Width"),("Высота","Height"),("Разрешение","Resolution"),("Качество","Quality"),("Прозрачный фон","Transparent background"),
    ("Фокус","Focus"),("Глубина резкости","Depth of field"),("Диафрагма","Aperture"),("Экспозиция","Exposure"),
    ("Эффекты камеры","Camera effects"),("Добавить эффект","Add effect"),("Эффекты","Effects"),("Виньетка","Vignette"),
    ("Зерно","Grain"),("Хроматическая аберрация","Chromatic aberration"),("Контраст","Contrast"),("Насыщенность","Saturation"),
    ("Яркость","Brightness"),("Размытие","Blur"),("Туман","Fog"),("Дым","Smoke"),("Огонь","Fire"),
    ("Настройки","Settings"),("Настройки программы","Preferences"),("Тема интерфейса","Interface theme"),
    ("Тёмная","Dark"),("Светлая","Light"),("Акцент интерфейса","Accent color"),("Выбрать цвет","Choose color"),
    ("Показывать скелеты","Show skeletons"),("Адаптивное качество","Adaptive quality"),("Статистика просмотра","Viewport statistics"),
    ("Настройки сохраняются автоматически","Preferences are saved automatically"),("Синий","Blue"),("Фиолетовый","Violet"),
    ("Розовый","Pink"),("Оранжевый","Orange"),("Зелёный","Green"),("Бирюзовый","Teal"),
    ("Освещение","Lighting"),("Окружение","Environment"),("Тени","Shadows"),("Мощность","Power"),
    ("Основной свет","Key light"),("Точечный","Point"),("Направленный","Directional"),("Площадь","Area"),
    ("Солнце","Sun"),("Радиус","Radius"),("Угол","Angle"),("Дальность","Range"),("Мягкость","Softness"),
    ("Плоскость","Plane"),("Куб","Cube"),("Сфера","Sphere"),("Цилиндр","Cylinder"),("Конус","Cone"),("Тор","Torus"),
    ("Геометрия","Geometry"),("Сегменты","Segments"),("Нормали","Normals"),("Сглаживание","Smoothing"),
    ("Вершины","Vertices"),("Рёбра","Edges"),("Грани","Faces"),("Выбрать всё","Select all"),("Снять выделение","Deselect all"),
    ("Отразить","Mirror"),("Разделить","Split"),("Объединить","Join"),("Экструзия","Extrude"),("Фаска","Bevel"),
    ("Видимость","Visibility"),("Форма","Shape"),("Кость","Bone"),("Сустав","Joint"),("Кости","Bones"),
    ("Треугольник","Triangle"),("Прямоугольник","Rectangle"),("Круг","Circle"),("Заполненный","Filled"),
    ("Контур","Outline"),("Непрозрачность","Opacity"),("Цвет кости","Bone color"),("Цвет сустава","Joint color"),
    ("Ключ","Keyframe"),("Ключи","Keyframes"),("Кадр","Frame"),("Слои анимации","Animation layers"),
    ("Записать","Record"),("Таймлайн","Timeline"),("Отбросить","Discard"),("Поза","Pose"),
    ("Жидкость","Fluid"),("Частицы","Particles"),("Турбулентность","Turbulence"),("Направление","Direction"),
    ("Материал осколков","Shard material"),("Настройки разрушения обновлены.","Destruction settings updated."),
];

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn catalog_has_unique_keys_and_unknown_user_text_is_preserved() {
        let keys:std::collections::BTreeSet<_>=LABELS.iter().map(|p|p.0).collect();assert_eq!(keys.len(),LABELS.len());
        assert_eq!(Language::English.text("Проект · Ctrl+O"),"Project · Ctrl+O");
        assert_eq!(Language::English.text("Моя лиса.artforge"),"Моя лиса.artforge");
        let mut ui=crate::ui::Ui::new(1.);ui.language=Language::English;
        ui.field([0.,0.,200.,32.],"Камера",crate::ui::Action::EditTitle);
        assert_eq!(ui.texts[0].text,"Камера");
        let mut edit=crate::ui::TextEdit::new("Камера".into(),crate::ui::Action::EditTitle);
        ui.edit_field([0.,0.,200.,32.],&mut edit,false);assert!(ui.texts.iter().any(|t|t.text=="Камера"));assert!(!ui.texts.iter().any(|t|t.text=="Camera"));
    }
}
