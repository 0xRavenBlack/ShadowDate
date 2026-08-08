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
        "confirm_delete_title" => (
            "Delete appointment?",
            "Termin löschen?",
            "Supprimer le rendez-vous ?",
            "¿Eliminar la cita?",
            "删除约会？",
            "予定を削除しますか？",
            "Usunąć wydarzenie?",
        ),
        "confirm_delete_body" => (
            "This will permanently delete the appointment. This cannot be undone.",
            "Dadurch wird der Termin dauerhaft gelöscht. Dies kann nicht rückgängig gemacht werden.",
            "Le rendez-vous sera définitivement supprimé. Cette action est irréversible.",
            "Esto eliminará la cita de forma permanente. No se puede deshacer.",
            "这将永久删除该约会。此操作无法撤销。",
            "予定を完全に削除します。この操作は元に戻せません。",
            "Spowoduje to trwałe usunięcie wydarzenia. Nie można tego cofnąć.",
        ),
        "add_appointment" => (
            "+ Add appointment",
            "+ Termin hinzufügen",
            "+ Ajouter un rendez-vous",
            "+ Añadir cita",
            "+ 添加约会",
            "+ 予定を追加",
            "+ Dodaj wydarzenie",
        ),
        "all_day_short" => (
            "All day",
            "Ganztägig",
            "Journée",
            "Todo el día",
            "全天",
            "終日",
            "Cały dzień",
        ),
        "hours" => (
            "Hours", "Stunden", "Heures", "Horas", "小时", "時", "Godziny",
        ),
        "minutes" => (
            "Minutes", "Minuten", "Minutes", "Minutos", "分钟", "分", "Minuty",
        ),
        "prev_month" => (
            "Previous month",
            "Vorheriger Monat",
            "Mois précédent",
            "Mes anterior",
            "上个月",
            "前の月",
            "Poprzedni miesiąc",
        ),
        "next_month" => (
            "Next month",
            "Nächster Monat",
            "Mois suivant",
            "Mes siguiente",
            "下个月",
            "次の月",
            "Następny miesiąc",
        ),
        "settings" => (
            "Settings",
            "Einstellungen",
            "Paramètres",
            "Ajustes",
            "设置",
            "設定",
            "Ustawienia",
        ),
        "service_settings" => (
            "Service settings",
            "Diensteinstellungen",
            "Paramètres du service",
            "Ajustes del servicio",
            "服务设置",
            "サービス設定",
            "Ustawienia usługi",
        ),
        "reminders" => (
            "Reminders",
            "Erinnerungen",
            "Rappels",
            "Recordatorios",
            "提醒",
            "リマインダー",
            "Przypomnienia",
        ),
        "lead_time" => (
            "Remind before",
            "Erinnern vor",
            "Rappeler avant",
            "Recordar antes",
            "提前提醒",
            "前に通知",
            "Przypomnij przed",
        ),
        "all_day_time" => (
            "All-day reminder time",
            "Ganztägige Erinnerungszeit",
            "Heure de rappel (journée entière)",
            "Hora de recordatorio (todo el día)",
            "全天提醒时间",
            "終日のリマインダー時刻",
            "Godzina przypomnienia (cały dzień)",
        ),
        "test_notification" => (
            "Test notification",
            "Testbenachrichtigung",
            "Notification de test",
            "Notificación de prueba",
            "测试通知",
            "テスト通知",
            "Testuj powiadomienie",
        ),
        "test_notification_body" => (
            "This is a test notification from ShadowDate.",
            "Dies ist eine Testbenachrichtigung von ShadowDate.",
            "Ceci est une notification de test de ShadowDate.",
            "Esta es una notificación de prueba de ShadowDate.",
            "这是一条来自 ShadowDate 的测试通知。",
            "これは ShadowDate からのテスト通知です。",
            "To jest testowe powiadomienie od ShadowDate.",
        ),
        "service_running" => (
            "Service: running",
            "Dienst: läuft",
            "Service : actif",
            "Servicio: en ejecución",
            "服务：运行中",
            "サービス：実行中",
            "Usługa: działa",
        ),
        "service_stopped" => (
            "Service: stopped",
            "Dienst: gestoppt",
            "Service : arrêté",
            "Servicio: detenido",
            "服务：已停止",
            "サービス：停止中",
            "Usługa: zatrzymana",
        ),
        "enable" => (
            "Enable",
            "Aktivieren",
            "Activer",
            "Habilitar",
            "启用",
            "有効にする",
            "Włącz",
        ),
        "disable" => (
            "Disable",
            "Deaktivieren",
            "Désactiver",
            "Deshabilitar",
            "禁用",
            "無効にする",
            "Wyłącz",
        ),
        "service_not_installed" => (
            "Service unit not installed. Enable it with: systemctl --user enable --now shadowdate-service",
            "Diensteinheit nicht installiert. Aktivieren Sie sie mit: systemctl --user enable --now shadowdate-service",
            "Unité de service non installée. Activez-la avec : systemctl --user enable --now shadowdate-service",
            "Unidad de servicio no instalada. Actívela con: systemctl --user enable --now shadowdate-service",
            "未安装服务单元。启用命令：systemctl --user enable --now shadowdate-service",
            "サービスユニットがインストールされていません。有効にするには: systemctl --user enable --now shadowdate-service",
            "Jednostka usługi nie jest zainstalowana. Włącz ją: systemctl --user enable --now shadowdate-service",
        ),
        "load_warnings" => (
            "Some calendar entries could not be read and were skipped:",
            "Einige Kalendereinträge konnten nicht gelesen werden und wurden übersprungen:",
            "Certaines entrées du calendrier n'ont pas pu être lues et ont été ignorées :",
            "Algunas entradas del calendario no se pudieron leer y se omitieron:",
            "日历中的部分条目无法读取，已跳过：",
            "カレンダーの一部の項目を読み取れず、スキップしました：",
            "Niektóre wpisy kalendarza nie mogły zostać odczytane i zostały pominięte:",
        ),
        "import_warnings" => (
            "Some entries in the imported file could not be read and were skipped:",
            "Einige Einträge der importierten Datei konnten nicht gelesen werden und wurden übersprungen:",
            "Certaines entrées du fichier importé n'ont pas pu être lues et ont été ignorées :",
            "Algunas entradas del archivo importado no se pudieron leer y se omitieron:",
            "导入文件中的部分条目无法读取，已跳过：",
            "インポートしたファイルの一部の項目を読み取れず、スキップしました：",
            "Niektóre wpisy zaimportowanego pliku nie mogły zostać odczytane i zostały pominięte:",
        ),
        "load_failed" => (
            "Your calendar file could not be read.",
            "Ihre Kalenderdatei konnte nicht gelesen werden.",
            "Votre fichier de calendrier n'a pas pu être lu.",
            "No se pudo leer el archivo de calendario.",
            "无法读取您的日历文件。",
            "カレンダーファイルを読み取れませんでした。",
            "Nie udało się odczytać pliku kalendarza.",
        ),
        "load_failed_backed_up" => (
            "Your calendar file could not be read. A backup was saved to:",
            "Ihre Kalenderdatei konnte nicht gelesen werden. Eine Sicherungskopie wurde gespeichert unter:",
            "Votre fichier de calendrier n'a pas pu être lu. Une sauvegarde a été enregistrée dans :",
            "No se pudo leer el archivo de calendario. Se guardó una copia de seguridad en:",
            "无法读取您的日历文件。备份已保存到：",
            "カレンダーファイルを読み取れませんでした。バックアップは次に保存されました：",
            "Nie udało się odczytać pliku kalendarza. Kopia zapasowa została zapisana w:",
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

/// Compact "+N" overflow count for the grid dot row. Numeric-only, so it stays
/// language-neutral and narrow enough to never overflow a day cell.
pub fn more_compact(n: usize) -> String {
    format!("+{}", n)
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
