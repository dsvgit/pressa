// ============================================================================
// ЭТОТ ФАЙЛ — ТЕСТ. Он не делает "полезную работу" в программе,
// а проверяет, что другие части проекта (crates = "пакеты"/"библиотеки")
// не нарушают правила архитектуры.
//
// Идея простыми словами:
// В проекте есть 4 отдельных "коробки" с кодом (crates):
//   pressa-core    — самая внутренняя, "мозг", ничего не знает про остальных
//   pressa-storage — работает с базой данных (SQLite)
//   pressa-app     — бизнес-логика приложения
//   pressa-tui     — то, что рисует интерфейс в терминале (TUI = Text UI)
//
// Правило: "коробки" могут зависеть друг от друга только в одну сторону
// (например, core никогда не должен знать про tui или storage).
// Этот файл читает Cargo.toml (файл настроек) каждой коробки и проверяет,
// что в списке "зависимостей" (dependencies) нет запрещённых вещей.
// ============================================================================

//! Это специальный вид комментария (//!) — комментарий "ко всему модулю".
//! Он ссылается на документацию проекта: архитектуру и ADR (Architecture
//! Decision Record — запись о принятом архитектурном решении) №0006.
//!
//! Компилятор Rust не разрешит нарушить границу, только когда код реально
//! попытается её пересечь (т.е. вызовет функцию из "чужой" коробки).
//! А этот тест ловит нарушение РАНЬШЕ — прямо в файле Cargo.toml, в строчке,
//! которая делает такое нарушение вообще возможным (то есть в самой строке
//! "добавить эту зависимость").

// "use" — это как "import" в других языках: подключаем готовые инструменты.
use std::collections::BTreeSet; // BTreeSet — множество (набор уникальных элементов), которое хранит их в отсортированном порядке
use std::fs; // fs — работа с файлами (чтение и т.п.)
use std::path::{Path, PathBuf}; // Path/PathBuf — работа с путями к файлам/папкам

// Массив (список фиксированного размера) из 4 строк — имена всех коробок проекта.
// [&str; 4] значит: "массив из 4 элементов типа &str (ссылка на строку)".
const CRATES: [&str; 4] = ["pressa-core", "pressa-storage", "pressa-app", "pressa-tui"];

// Функция, которая находит корневую папку всего проекта (workspace).
fn workspace_root() -> PathBuf {
    // env!("CARGO_MANIFEST_DIR") — это папка, где лежит Cargo.toml ЭТОГО теста
    // (то есть папка pressa-tui). Функция берёт её родительскую папку —
    // это и есть корень всего workspace (там, где лежат все 4 коробки).
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent() // .parent() — "поднимись на уровень выше" в дереве папок
        .expect("pressa-tui сидит внутри корня workspace") // если родителя нет — упасть с этим сообщением об ошибке
        .to_path_buf() // превращаем в "владеющий" тип пути (PathBuf), а не просто ссылку
}

// Функция читает содержимое файла Cargo.toml для конкретной коробки (krate)
// и возвращает его как обычную текстовую строку (String).
fn manifest(krate: &str) -> String {
    // Строим путь: корень_проекта / имя_коробки / Cargo.toml
    let path = workspace_root().join(krate).join("Cargo.toml");
    // Пытаемся прочитать файл. unwrap_or_else — "если не получилось, сделай вот что":
    // здесь — вывести понятную ошибку с путём файла и упасть (panic!).
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

// Маленькая функция-помощник: убирает пробелы по краям и кавычки (双 или одинарные).
// Например: превращает `  "hello"  ` в `hello`.
fn unquote(s: &str) -> &str {
    s.trim() // убрать пробелы по краям
        .trim_matches('"') // убрать двойные кавычки по краям, если есть
        .trim_matches('\'') // убрать одинарные кавычки по краям, если есть
}

/// Находит первую пару двойных кавычек в строке и возвращает то, что между ними.
/// Например, из `имя = "rusqlite"` вытащит `rusqlite`.
fn first_quoted(s: &str) -> Option<&str> {
    // Option<&str> означает: "может вернуть строку, а может ничего (None)".
    // split_once('"') разбивает строку на 2 части по первой найденной кавычке.
    let (_, rest) = s.split_once('"')?; // "?" значит: если не нашли — сразу вернуть None
    let (value, _) = rest.split_once('"')?; // ищем вторую кавычку — конец значения
    Some(value) // всё получилось — возвращаем найденное значение, обёрнутое в Some(...)
}

/// В Cargo.toml можно "переименовать" зависимость, например:
///   db = { package = "rusqlite" }
/// Тут в коде используется имя "db", но реальный пакет — "rusqlite".
/// Эта функция как раз вытаскивает настоящее имя пакета (после слова "package").
/// Это важно проверять отдельно, потому что именно так можно "спрятать"
/// запрещённую зависимость под другим именем.
fn inline_package(value: &str) -> Option<&str> {
    let (_, rest) = value.split_once("package")?; // ищем слово "package" в строке
    let (_, rest) = rest.split_once('=')?; // после него ищем знак "="
    first_quoted(rest) // и берём то, что в кавычках после "="
}

/// Главная "рабочая лошадка": читает ВЕСЬ файл Cargo.toml (переданный как текст)
/// и собирает множество (BTreeSet) названий ВСЕХ зависимостей — то есть всех
/// внешних коробок/библиотек, которые эта коробка использует.
///
/// Учитываются разные разделы файла:
///   [dependencies]       — обычные зависимости
///   [dev-dependencies]   — зависимости только для тестов
///   [build-dependencies] — зависимости для сборки
///   [target.'cfg(...)'.dependencies] — зависимости только для конкретной ОС
///
/// Важно: считается имя РЕАЛЬНОГО пакета, а не то имя, под которым его
/// назвали в коде (см. пример с "db = rusqlite" выше).
fn declared_dependencies(manifest: &str) -> BTreeSet<String> {
    let mut found = BTreeSet::new(); // сюда будем складывать найденные имена зависимостей
    let mut in_dependency_table = false; // "мы сейчас внутри раздела про зависимости?"
    // Если мы внутри блока вида [dependencies.имя_зависимости],
    // то тут будет храниться "имя_зависимости" — потому что дальше
    // строки внутри такого блока — это НЕ новые зависимости, а поля
    // (настройки) уже названной зависимости (version, features, package...).
    let mut table_of: Option<String> = None;

    // .lines() — разбивает весь текст файла на отдельные строки.
    // .map(str::trim) — у каждой строки убирает пробелы по краям.
    for line in manifest.lines().map(str::trim) {
        // Проверяем: похожа ли строка на заголовок раздела, типа [dependencies]?
        // strip_prefix('[') — "если строка начинается с [, убери этот символ"
        // strip_suffix(']') — "если заканчивается на ], тоже убери"
        if let Some(header) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            // Мы нашли строку-заголовок. Запоминаем: относится ли она к зависимостям.
            in_dependency_table = header.contains("dependencies");
            table_of = None; // при входе в новый раздел сбрасываем "текущую зависимость"

            if in_dependency_table {
                // Если заголовок вида [dependencies.имя] — достаём "имя" после точки.
                if let Some((_, name)) = header.rsplit_once("dependencies.") {
                    let name = unquote(name);
                    found.insert(name.to_string()); // это тоже зависимость — добавляем
                    table_of = Some(name.to_string()); // запоминаем, что мы теперь "внутри" неё
                }
            }
            continue; // строка была заголовком — переходим к следующей строке файла
        }

        // Пропускаем строку, если: мы не в разделе про зависимости,
        // строка пустая, или это комментарий (начинается с #).
        if !in_dependency_table || line.is_empty() || line.starts_with('#') {
            continue;
        }

        // Пытаемся разбить строку по знаку "=", например: rusqlite = "0.32"
        // key = "rusqlite", value = "\"0.32\""
        let Some((key, value)) = line.split_once('=') else {
            continue; // если знака "=" нет — это не похоже на зависимость, пропускаем
        };

        // Если мы находимся внутри блока [dependencies.имя] (см. выше),
        // то строки здесь — это НАСТРОЙКИ уже известной зависимости,
        // а не новые зависимости. Нас интересует только строка "package = ...",
        // потому что она может подменять реальное имя пакета.
        if table_of.is_some() {
            if key.trim() == "package" {
                if let Some(name) = first_quoted(value) {
                    found.insert(name.to_string());
                }
            }
            continue; // остальные настройки (version, features и т.д.) не важны
        }

        // Обычная строка вида: имя = значение (не внутри table_of).
        // Сначала проверяем: а вдруг это "db = { package = \"rusqlite\" }"?
        if let Some(name) = inline_package(value) {
            found.insert(name.to_string()); // тогда реальное имя — rusqlite
        }

        // В любом случае само имя слева от "=" тоже считается зависимостью
        // (например rusqlite.workspace = true — имя тут "rusqlite").
        let key = unquote(key);
        let key = key.split('.').next().unwrap_or(key); // отрезаем всё после точки, если есть (".workspace" и т.п.)
        found.insert(key.to_string());
    }

    found // возвращаем итоговое множество всех найденных зависимостей
}

/// Простая проверка: "зависит ли этот manifest (текст Cargo.toml) от пакета dep,
/// под каким бы именем он ни был подключён?"
fn depends_on(manifest: &str, dep: &str) -> bool {
    declared_dependencies(manifest).contains(dep)
}

// Проверяет: коробка krate НЕ должна зависеть ни от одной из "запрещённых" (forbidden).
fn assert_forbidden(krate: &str, forbidden: &[&str]) {
    let manifest = manifest(krate); // читаем содержимое Cargo.toml коробки
    for dep in forbidden {
        // assert! — если условие ложно, тест падает с сообщением после запятой.
        assert!(
            !depends_on(&manifest, dep), // "НЕ должно зависеть от dep"
            "{krate} не должен зависеть от {dep} (docs/architecture.md §2)"
        );
    }
}

// Проверяет обратное: коробка krate ОБЯЗАНА зависеть от каждой из "нужных" (required).
fn assert_required(krate: &str, required: &[&str]) {
    let manifest = manifest(krate);
    for dep in required {
        assert!(
            depends_on(&manifest, dep), // "должно зависеть от dep"
            "{krate} должен зависеть от {dep} (docs/architecture.md §2)"
        );
    }
}

// ============================================================================
// Дальше идут САМИ ТЕСТЫ. Каждая функция с #[test] сверху — это отдельный
// автоматический тест, который запускается командой `cargo test`.
// ============================================================================

#[test] // помечает функцию как тест
fn the_workspace_has_exactly_the_four_crates_of_adr_0006() {
    // Проверяем: главный (корневой) Cargo.toml перечисляет все 4 коробки,
    // и у каждой из них реально существует своя папка с Cargo.toml.
    let root = fs::read_to_string(workspace_root().join("Cargo.toml")).expect("root Cargo.toml");
    for krate in CRATES {
        // Ищем, что в тексте корневого файла есть строка вида "pressa-core"
        assert!(
            root.contains(&format!("\"{krate}\"")),
            "{krate} не является участником workspace"
        );
        // Проверяем, что файл krate/Cargo.toml реально существует на диске
        assert!(
            workspace_root().join(krate).join("Cargo.toml").is_file(),
            "{krate}/Cargo.toml отсутствует"
        );
    }
}

#[test]
fn core_knows_nothing_of_the_ui_or_the_database() {
    // "core" (мозг программы) не должен ничего знать про интерфейс (ratatui,
    // crossterm), про базу данных (rusqlite), про парсинг командной строки (clap),
    // и вообще ни про одну из других 3 коробок проекта.
    assert_forbidden(
        "pressa-core",
        &[
            "ratatui",
            "crossterm",
            "rusqlite",
            "clap",
            "pressa-storage",
            "pressa-app",
            "pressa-tui",
        ],
    );
}

#[test]
fn storage_knows_nothing_of_the_ui() {
    // "storage" (работа с БД) не должен знать про интерфейс и про app/tui.
    assert_forbidden(
        "pressa-storage",
        &["ratatui", "crossterm", "clap", "pressa-app", "pressa-tui"],
    );
}

#[test]
fn app_knows_nothing_of_the_ui_or_sqlite() {
    // "app" (бизнес-логика) не должен знать про интерфейс, ни про конкретную
    // базу данных rusqlite напрямую, ни про саму коробку pressa-storage —
    // потому что app должен общаться с хранилищем через общий "интерфейс"
    // (описанный в pressa-core), а не напрямую с реализацией.
    // Если бы app зависел от pressa-storage — стрелка зависимости "смотрела бы"
    // в неправильную сторону (см. §1 архитектуры).
    assert_forbidden(
        "pressa-app",
        &[
            "ratatui",
            "crossterm",
            "rusqlite",
            "pressa-storage",
            "pressa-tui",
        ],
    );
}

#[test]
fn the_tui_never_touches_sqlite_directly() {
    // Интерфейс (tui) не должен напрямую трогать SQLite — доступ к данным
    // должен идти через storage/app, а не в обход них.
    assert_forbidden("pressa-tui", &["rusqlite"]);
}

#[test]
fn dependencies_point_inward() {
    // А это проверка "в обратную сторону": некоторые зависимости ОБЯЗАНЫ быть.
    // storage должен зависеть от core (использовать его интерфейсы),
    // app тоже должен зависеть от core,
    // а tui должен зависеть и от app, и от storage (это "точка сборки всего").
    assert_required("pressa-storage", &["pressa-core"]);
    assert_required("pressa-app", &["pressa-core"]);
    assert_required("pressa-tui", &["pressa-app", "pressa-storage"]);
}

#[test]
fn nothing_in_the_workspace_is_async() {
    // ADR-0002 (ещё одно архитектурное решение): проект решил НЕ использовать
    // асинхронное программирование (tokio, async-std, sqlx), а работать
    // синхронно с обычным rusqlite. Проверяем, что никто тайком не подключил
    // асинхронные библиотеки.
    for krate in CRATES {
        assert_forbidden(krate, &["tokio", "async-std", "sqlx"]);
    }
}

#[test]
fn the_scanner_sees_a_dependency_however_it_is_written() {
    // Это тест "теста" — проверка, что сама функция declared_dependencies
    // умеет распознавать зависимость rusqlite, как бы её ни записали в файле:
    // с пробелами, без пробелов, в кавычках, через .workspace, через переименование,
    // в dev-dependencies, build-dependencies, через [dependencies.имя],
    // или только для конкретной платформы (cfg(unix)).
    let shapes = [
        "[dependencies]\nrusqlite = \"0.32\"",
        "[dependencies]\nrusqlite=\"0.32\"",
        "[dependencies]\nrusqlite  =  \"0.32\"",
        "[dependencies]\n\"rusqlite\" = \"0.32\"",
        "[dependencies]\nrusqlite.workspace = true",
        "[dependencies]\ndb = { package = \"rusqlite\", version = \"0.32\" }",
        "[dev-dependencies]\nrusqlite = \"0.32\"",
        "[build-dependencies.rusqlite]\nversion = \"0.32\"",
        "[dependencies.db]\nversion = \"0.32\"\npackage = \"rusqlite\"",
        "[target.'cfg(unix)'.dependencies]\nrusqlite = \"0.32\"",
    ];
    // Проходим по каждому варианту записи и убеждаемся, что сканер его нашёл.
    for shape in shapes {
        assert!(depends_on(shape, "rusqlite"), "сканер пропустил:\n{shape}");
    }
}

#[test]
fn the_scanner_does_not_invent_dependencies() {
    // Обратная проверка: убеждаемся, что сканер НЕ находит зависимости
    // там, где их нет — то есть не путает, например, имя поля "version"
    // или слово "features" с названием зависимости.
    let manifest = "\
[package]
name = \"pressa-tui\"
version = \"0.0.0\"

[dependencies]
# Только связывание модулей, в корне сборки.
pressa-app.workspace = true
serde = { version = \"1\", features = [\"derive\"] }
";
    // Эти две зависимости ДОЛЖНЫ быть найдены:
    assert!(depends_on(manifest, "pressa-app"));
    assert!(depends_on(manifest, "serde"));

    // А вот эти слова НЕ должны считаться зависимостями — это либо
    // настоящие поля файла (name, version, features...), либо служебные
    // слова (workspace, derive), либо коробка, которая тут вообще не упомянута
    // как зависимость (rusqlite, pressa-tui — это имя самого пакета, не зависимости).
    for absent in [
        "rusqlite",
        "pressa-tui",
        "name",
        "version",
        "workspace",
        "features",
        "derive",
    ] {
        assert!(
            !depends_on(manifest, absent),
            "сканер придумал лишнее: {absent}"
        );
    }
}
