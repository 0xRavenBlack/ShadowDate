use chrono::{Datelike, NaiveDate};
use std::sync::OnceLock;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lang {
    En,
    De,
    Fr,
    Es,
    Zh,
    Ja,
    Pl,
}

static LANG: OnceLock<Lang> = OnceLock::new();

/// Detect the UI language from the environment (LC_ALL / LC_MESSAGES / LANG).
fn detect_lang() -> Lang {
    let raw = std::env::var("LC_ALL")
        .or_else(|_| std::env::var("LC_MESSAGES"))
        .or_else(|_| std::env::var("LANG"))
        .unwrap_or_default()
        .to_ascii_lowercase();
    let code = raw.split(['_', '.', '@']).next().unwrap_or("");
    match code {
        "de" => Lang::De,
        "fr" => Lang::Fr,
        "es" => Lang::Es,
        "zh" => Lang::Zh,
        "ja" => Lang::Ja,
        "pl" => Lang::Pl,
        _ => Lang::En,
    }
}

pub fn lang() -> Lang {
    *LANG.get_or_init(detect_lang)
}

/// Translate a message key into the active language.
pub fn t(key: &str) -> &'static str {
    let l = lang();
    let (en, de, fr, es, zh, ja, pl) = match key {
        "new" => ("+ New", "+ Neu", "+ Nouveau", "+ Nuevo", "+ 新建", "+ 新規", "+ Nowy"),
        "today" => ("Today", "Heute", "Aujourd'hui", "Hoy", "今天", "今日", "Dzisiaj"),
        "import" => ("Import", "Importieren", "Importer", "Importar", "导入", "インポート", "Importuj"),
        "export" => ("Export", "Exportieren", "Exporter", "Exportar", "导出", "エクスポート", "Eksportuj"),
        "exit" => ("Exit", "Beenden", "Quitter", "Salir", "退出", "終了", "Wyjście"),
        "cancel" => ("Cancel", "Abbrechen", "Annuler", "Cancelar", "取消", "キャンセル", "Anuluj"),
        "save" => ("Save", "Speichern", "Enregistrer", "Guardar", "保存", "保存", "Zapisz"),
        "open" => ("Open", "Öffnen", "Ouvrir", "Abrir", "打开", "開く", "Otwórz"),
        "delete" => ("Delete", "Löschen", "Supprimer", "Eliminar", "删除", "削除", "Usuń"),
        "ok" => ("OK", "OK", "OK", "Aceptar", "确定", "OK", "OK"),
        "new_appointment" => (
            "New appointment",
            "Neuer Termin",
            "Nouveau rendez-vous",
            "Nueva cita",
            "新建约会",
            "新しい予定",
            "Nowe wydarzenie",
        ),
        "edit_appointment" => (
            "Edit appointment",
            "Termin bearbeiten",
            "Modifier le rendez-vous",
            "Editar cita",
            "编辑约会",
            "予定を編集",
            "Edytuj wydarzenie",
        ),
        "details" => ("Details", "Details", "Détails", "Detalles", "详情", "詳細", "Szczegóły"),
        "date_time" => (
            "Date & time",
            "Datum & Uhrzeit",
            "Date et heure",
            "Fecha y hora",
            "日期和时间",
            "日付と時刻",
            "Data i godzina",
        ),
        "title" => ("Title", "Titel", "Titre", "Título", "标题", "タイトル", "Tytuł"),
        "description" => (
            "Description",
            "Beschreibung",
            "Description",
            "Descripción",
            "描述",
            "説明",
            "Opis",
        ),
        "location" => ("Location", "Ort", "Lieu", "Lugar", "地点", "場所", "Miejsce"),
        "start" => ("Start", "Beginn", "Début", "Inicio", "开始", "開始", "Początek"),
        "end" => ("End", "Ende", "Fin", "Fin", "结束", "終了", "Koniec"),
        "all_day" => (
            "All day",
            "Ganztägig",
            "Toute la journée",
            "Todo el día",
            "全天",
            "終日",
            "Cały dzień",
        ),
        "add_title" => (
            "Add a title",
            "Titel hinzufügen",
            "Ajouter un titre",
            "Añadir un título",
            "添加标题",
            "タイトルを追加",
            "Dodaj tytuł",
        ),
        "add_description" => (
            "Add a description",
            "Beschreibung hinzufügen",
            "Ajouter une description",
            "Añadir una descripción",
            "添加描述",
            "説明を追加",
            "Dodaj opis",
        ),
        "add_location" => (
            "Add a location",
            "Ort hinzufügen",
            "Ajouter un lieu",
            "Añadir un lugar",
            "添加地点",
            "場所を追加",
            "Dodaj miejsce",
        ),
        "no_appointments" => (
            "No appointments",
            "Keine Termine",
            "Aucun rendez-vous",
            "Sin citas",
            "无约会",
            "予定なし",
            "Brak wydarzeń",
        ),
        "import_ics" => (
            "Import .ics",
            ".ics importieren",
            "Importer .ics",
            "Importar .ics",
            "导入 .ics",
            ".ics をインポート",
            "Importuj .ics",
        ),
        "export_ics" => (
            "Export .ics",
            ".ics exportieren",
            "Exporter .ics",
            "Exportar .ics",
            "导出 .ics",
            ".ics をエクスポート",
            "Eksportuj .ics",
        ),
        "title_required" => (
            "Title is required.",
            "Titel ist erforderlich.",
            "Le titre est obligatoire.",
            "El título es obligatorio.",
            "标题为必填项。",
            "タイトルは必須です。",
            "Tytuł jest wymagany.",
        ),
        "time_out_of_range" => (
            "Time values out of range.",
            "Zeitwerte außerhalb des gültigen Bereichs.",
            "Valeurs horaires hors limites.",
            "Valores de tiempo fuera de rango.",
            "时间值超出范围。",
            "時刻の値が範囲外です。",
            "Wartości czasu poza zakresem.",
        ),
        "invalid_date" => (
            "Invalid date.",
            "Ungültiges Datum.",
            "Date invalide.",
            "Fecha no válida.",
            "无效的日期。",
            "無効な日付です。",
            "Nieprawidłowa data.",
        ),
        "start_hour" => (
            "Start hour",
            "Startstunde",
            "Heure de début",
            "Hora de inicio",
            "开始小时",
            "開始時",
            "Godzina rozpoczęcia",
        ),
        "start_min" => (
            "Start minute",
            "Startminute",
            "Minute de début",
            "Minuto de inicio",
            "开始分钟",
            "開始分",
            "Minuta rozpoczęcia",
        ),
        "end_hour" => (
            "End hour",
            "Endstunde",
            "Heure de fin",
            "Hora de fin",
            "结束小时",
            "終了時",
            "Godzina zakończenia",
        ),
        "end_min" => (
            "End minute",
            "Endminute",
            "Minute de fin",
            "Minuto de fin",
            "结束分钟",
            "終了分",
            "Minuta zakończenia",
        ),
        _ => ("???", "???", "???", "???", "???", "???", "???"),
    };
    match l {
        Lang::En => en,
        Lang::De => de,
        Lang::Fr => fr,
        Lang::Es => es,
        Lang::Zh => zh,
        Lang::Ja => ja,
        Lang::Pl => pl,
    }
}

/// "%d must be a number." style message, using a localized field name.
pub fn must_be_number(field_key: &str) -> String {
    let field = t(field_key);
    match lang() {
        Lang::En => format!("{} must be a number.", field),
        Lang::De => format!("{} muss eine Zahl sein.", field),
        Lang::Fr => format!("{} doit être un nombre.", field),
        Lang::Es => format!("{} debe ser un número.", field),
        Lang::Zh => format!("{}必须是数字。", field),
        Lang::Ja => format!("{}は数値でなければなりません。", field),
        Lang::Pl => format!("{} musi być liczbą.", field),
    }
}

/// "+N more" chip overflow label.
pub fn more_label(n: usize) -> String {
    match lang() {
        Lang::En => format!("+{} more", n),
        Lang::De => format!("+{} weitere", n),
        Lang::Fr => format!("+{} de plus", n),
        Lang::Es => format!("+{} más", n),
        Lang::Zh => format!("+{} 更多", n),
        Lang::Ja => format!("他 {} 件", n),
        Lang::Pl => format!("+{} więcej", n),
    }
}

/// Short weekday abbreviations Mon..Sun for the grid header.
pub fn weekday_abbrevs() -> [&'static str; 7] {
    match lang() {
        Lang::En => ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"],
        Lang::De => ["Mo", "Di", "Mi", "Do", "Fr", "Sa", "So"],
        Lang::Fr => ["Lu", "Ma", "Me", "Je", "Ve", "Sa", "Di"],
        Lang::Es => ["Lu", "Ma", "Mi", "Ju", "Vi", "Sá", "Do"],
        Lang::Zh => ["一", "二", "三", "四", "五", "六", "日"],
        Lang::Ja => ["月", "火", "水", "木", "金", "土", "日"],
        Lang::Pl => ["Pn", "Wt", "Śr", "Cz", "Pt", "So", "Nd"],
    }
}

fn full_weekday(idx: usize) -> &'static str {
    // idx: 0 = Monday .. 6 = Sunday
    let table: [[&str; 7]; 7] = [
        ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"],
        ["Montag", "Dienstag", "Mittwoch", "Donnerstag", "Freitag", "Samstag", "Sonntag"],
        ["lundi", "mardi", "mercredi", "jeudi", "vendredi", "samedi", "dimanche"],
        ["lunes", "martes", "miércoles", "jueves", "viernes", "sábado", "domingo"],
        ["星期一", "星期二", "星期三", "星期四", "星期五", "星期六", "星期日"],
        ["月曜日", "火曜日", "水曜日", "木曜日", "金曜日", "土曜日", "日曜日"],
        [
            "poniedziałek",
            "wtorek",
            "środa",
            "czwartek",
            "piątek",
            "sobota",
            "niedziela",
        ],
    ];
    table[lang_index()][idx]
}

fn full_month(idx: usize) -> &'static str {
    // idx: 0 = January .. 11 = December
    // Chinese and Japanese share the same "N月" month forms, so the JA row
    // reuses the ZH row to avoid divergence.
    const ZH_MONTHS: [&str; 12] = [
        "1月", "2月", "3月", "4月", "5月", "6月", "7月", "8月", "9月", "10月", "11月", "12月",
    ];
    let table: [[&str; 12]; 7] = [
        [
            "January", "February", "March", "April", "May", "June", "July", "August", "September",
            "October", "November", "December",
        ],
        [
            "Januar", "Februar", "März", "April", "Mai", "Juni", "Juli", "August", "September",
            "Oktober", "November", "Dezember",
        ],
        [
            "janvier", "février", "mars", "avril", "mai", "juin", "juillet", "août", "septembre",
            "octobre", "novembre", "décembre",
        ],
        [
            "enero", "febrero", "marzo", "abril", "mayo", "junio", "julio", "agosto", "septiembre",
            "octubre", "noviembre", "diciembre",
        ],
        ZH_MONTHS,
        ZH_MONTHS,
        [
            "styczeń",
            "luty",
            "marzec",
            "kwiecień",
            "maj",
            "czerwiec",
            "lipiec",
            "sierpień",
            "wrzesień",
            "październik",
            "listopad",
            "grudzień",
        ],
    ];
    table[lang_index()][idx]
}

fn lang_index() -> usize {
    match lang() {
        Lang::En => 0,
        Lang::De => 1,
        Lang::Fr => 2,
        Lang::Es => 3,
        Lang::Zh => 4,
        Lang::Ja => 5,
        Lang::Pl => 6,
    }
}

/// Localized "Month YYYY" title for the grid header.
pub fn format_month_year(year: i32, month0: usize) -> String {
    let m = full_month(month0);
    match lang() {
        Lang::Zh | Lang::Ja => format!("{}年 {}", year, m),
        _ => format!("{} {}", m, year),
    }
}

/// Localized long date, e.g. "Saturday, July 18, 2026".
pub fn format_date(d: NaiveDate) -> String {
    let wd = full_weekday(d.weekday().num_days_from_monday() as usize);
    let m = full_month((d.month() - 1) as usize);
    let day = d.day();
    let year = d.year();
    match lang() {
        Lang::En => format!("{}, {} {}, {}", wd, m, day, year),
        Lang::De => format!("{}, {}. {} {}", wd, day, m, year),
        Lang::Fr => format!("{} {} {} {}", wd, day, m, year),
        Lang::Es => format!("{}, {} de {} de {}", wd, day, m, year),
        Lang::Zh | Lang::Ja => format!("{}年{}{}日 {}", year, m, day, wd),
        Lang::Pl => format!("{}, {} {} {}", wd, day, m, year),
    }
}
