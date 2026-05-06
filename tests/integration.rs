use std::fs;
use std::path::Path;
use tempfile::TempDir;

fn setup_site(dir: &Path, fragments: &[(&str, &str)], pages: &[(&str, &str)]) {
    let frag_dir = dir.join("_fragments");
    fs::create_dir_all(&frag_dir).unwrap();
    for (name, content) in fragments {
        fs::write(frag_dir.join(name), content).unwrap();
    }
    for (name, content) in pages {
        fs::write(dir.join(name), content).unwrap();
    }
}

fn setup_site_with_config(
    dir: &Path,
    config: &str,
    fragments: &[(&str, &str)],
    pages: &[(&str, &str)],
) {
    fs::write(dir.join("fragments.toml"), config).unwrap();
    let frag_dir_name = extract_fragments_dir(config);
    let frag_dir = dir.join(frag_dir_name);
    fs::create_dir_all(&frag_dir).unwrap();
    for (name, content) in fragments {
        fs::write(frag_dir.join(name), content).unwrap();
    }
    for (name, content) in pages {
        fs::write(dir.join(name), content).unwrap();
    }
}

fn extract_fragments_dir(config: &str) -> String {
    for line in config.lines() {
        if line.starts_with("fragments_dir") {
            let val = line.split('=').nth(1).unwrap().trim().trim_matches('"');
            return val.to_string();
        }
    }
    "_fragments".to_string()
}

fn run_init(dir: &Path, file: &str) -> std::process::Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_pagekit"))
        .arg(dir.to_str().unwrap())
        .arg("init")
        .arg(file)
        .output()
        .expect("failed to run pagekit")
}

fn run_extract(dir: &Path) -> std::process::Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_pagekit"))
        .arg(dir.to_str().unwrap())
        .arg("extract")
        .output()
        .expect("failed to run pagekit")
}

fn run_sync(dir: &Path) -> std::process::Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_pagekit"))
        .arg(dir.to_str().unwrap())
        .arg("sync")
        .output()
        .expect("failed to run pagekit")
}

// --- Init command ---

#[test]
fn init_creates_page_with_markers() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    setup_site(
        root,
        &[
            ("head.html", "<meta charset=\"utf-8\">"),
            ("body-open.html", "<nav>Nav</nav>"),
            ("cta.html", "<a>Buy</a>"),
        ],
        &[],
    );

    let output = run_init(root, "about.html");
    assert!(output.status.success(), "init failed: {:?}", output);

    let result = fs::read_to_string(root.join("about.html")).unwrap();
    assert!(result.contains("<!DOCTYPE html>"));
    assert!(result.contains("<!-- fragment:head -->"));
    assert!(result.contains("<!-- /fragment:head -->"));
    assert!(result.contains("<!-- fragment:body-open -->"));
    assert!(result.contains("<!-- fragment:cta -->"));
    assert!(result.contains("<title>about</title>"));
}

#[test]
fn init_refuses_to_overwrite() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    setup_site(
        root,
        &[("head.html", "<meta>")],
        &[("index.html", "<p>existing</p>")],
    );

    let output = run_init(root, "index.html");
    assert!(!output.status.success(), "init should refuse to overwrite");

    let result = fs::read_to_string(root.join("index.html")).unwrap();
    assert!(
        result.contains("<p>existing</p>"),
        "original content preserved"
    );
}

#[test]
fn init_then_sync_fills_markers() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    setup_site(
        root,
        &[
            ("head.html", "<link rel=\"stylesheet\" href=\"styles.css\">"),
            ("body-close.html", "<footer>Footer</footer>"),
        ],
        &[],
    );

    let init_out = run_init(root, "new-page.html");
    assert!(init_out.status.success());

    let sync_out = run_sync(root);
    assert!(sync_out.status.success());

    let result = fs::read_to_string(root.join("new-page.html")).unwrap();
    assert!(result.contains("<link rel=\"stylesheet\" href=\"styles.css\">"));
    assert!(result.contains("<footer>Footer</footer>"));
}

#[test]
fn init_with_custom_prefix() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    setup_site_with_config(
        root,
        "marker_prefix = \"sync\"\nfragments_dir = \"fragments\"\n",
        &[("nav.html", "<nav>Nav</nav>")],
        &[],
    );

    let output = run_init(root, "page.html");
    assert!(output.status.success());

    let result = fs::read_to_string(root.join("page.html")).unwrap();
    assert!(result.contains("<!-- sync:nav -->"));
    assert!(result.contains("<!-- /sync:nav -->"));
}

#[test]
fn init_creates_agents_md() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    setup_site(root, &[("head.html", "<meta>")], &[]);

    let output = run_init(root, "index.html");
    assert!(output.status.success());

    let agents = fs::read_to_string(root.join("_fragments/AGENTS.md")).unwrap();
    assert!(agents.contains("fragments"));
    assert!(agents.contains("<!-- fragment:<name> -->"));
}

#[test]
fn init_does_not_overwrite_agents_md() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    setup_site(root, &[("head.html", "<meta>")], &[]);

    fs::write(root.join("_fragments/AGENTS.md"), "custom content").unwrap();

    run_init(root, "index.html");

    let agents = fs::read_to_string(root.join("_fragments/AGENTS.md")).unwrap();
    assert_eq!(agents, "custom content");
}

#[test]
fn target_dir_init_creates_in_subdirectory() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    fs::write(root.join("fragments.toml"), "target_dir = \"www\"\n").unwrap();

    let frag_dir = root.join("_fragments");
    fs::create_dir_all(&frag_dir).unwrap();
    fs::write(frag_dir.join("head.html"), "<meta>").unwrap();

    let www = root.join("www");
    fs::create_dir_all(&www).unwrap();

    let output = run_init(root, "about.html");
    assert!(output.status.success(), "init failed: {:?}", output);

    assert!(www.join("about.html").exists(), "file should be in www/");
    assert!(
        !root.join("about.html").exists(),
        "file should NOT be at root"
    );

    let result = fs::read_to_string(www.join("about.html")).unwrap();
    assert!(result.contains("<!-- fragment:head -->"));
}

// --- Extract command ---

#[test]
fn extract_wraps_correct_element_among_siblings() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    let canonical = "<footer><p>&copy; SiteCo</p></footer>";

    let page_a = format!(
        "<!DOCTYPE html><html><body>\n<footer><p>Footnote A</p></footer>\n<main>A</main>\n{canonical}\n</body></html>"
    );
    let page_b = format!(
        "<!DOCTYPE html><html><body>\n<footer><p>Footnote B</p></footer>\n<main>B</main>\n{canonical}\n</body></html>"
    );

    fs::write(root.join("a.html"), &page_a).unwrap();
    fs::write(root.join("b.html"), &page_b).unwrap();

    let output = run_extract(root);
    assert!(output.status.success(), "extract failed: {:?}", output);

    for (path, footnote) in [("a.html", "Footnote A"), ("b.html", "Footnote B")] {
        let content = fs::read_to_string(root.join(path)).unwrap();
        let open = content
            .find("<!-- fragment:footer -->")
            .unwrap_or_else(|| panic!("{path} missing open marker:\n{content}"));
        let close = content
            .find("<!-- /fragment:footer -->")
            .unwrap_or_else(|| panic!("{path} missing close marker"));
        let wrapped = &content[open..close];
        assert!(
            wrapped.contains("SiteCo"),
            "{path}: marker should wrap the canonical footer, got:\n{wrapped}"
        );
        assert!(
            !wrapped.contains(footnote),
            "{path}: marker incorrectly wrapped the page-specific <footer> ({footnote})"
        );
    }
}

#[test]
fn extract_creates_fragment_file() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    let nav_html = "<nav><a href=\"/\">Home</a><a href=\"/about\">About</a></nav>";
    let page = |unique: &str| {
        format!("<!DOCTYPE html><html><body>{nav_html}<main>{unique}</main></body></html>")
    };

    fs::write(root.join("a.html"), page("A")).unwrap();
    fs::write(root.join("b.html"), page("B")).unwrap();

    let output = run_extract(root);
    assert!(output.status.success());

    let frag = fs::read_to_string(root.join("_fragments/nav.html")).unwrap();
    assert!(frag.contains("<a href=\"/\">Home</a>"));
}

#[test]
fn extract_idempotent_does_not_double_wrap() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    let nav_html = "<nav><a href=\"/\">Home</a></nav>";
    let page = |unique: &str| {
        format!("<!DOCTYPE html><html><body>{nav_html}<main>{unique}</main></body></html>")
    };

    fs::write(root.join("a.html"), page("A")).unwrap();
    fs::write(root.join("b.html"), page("B")).unwrap();

    let _ = run_extract(root);
    let after_first = fs::read_to_string(root.join("a.html")).unwrap();

    let _ = run_extract(root);
    let after_second = fs::read_to_string(root.join("a.html")).unwrap();

    assert_eq!(after_first, after_second, "second extract must be a no-op");
    assert_eq!(
        after_first.matches("<!-- fragment:nav -->").count(),
        1,
        "marker must not be duplicated"
    );
}

#[test]
fn extract_picks_up_user_defined_candidate() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    fs::write(
        root.join("fragments.toml"),
        r#"[[extract.candidates]]
name = "sidebar"
selector = "aside.sidebar"
tag = "aside"
"#,
    )
    .unwrap();

    let sidebar = "<aside class=\"sidebar\"><h3>Links</h3><a href=\"/x\">x</a></aside>";
    let page = |unique: &str| {
        format!("<!DOCTYPE html><html><body>{sidebar}<main>{unique}</main></body></html>")
    };

    fs::write(root.join("a.html"), page("A")).unwrap();
    fs::write(root.join("b.html"), page("B")).unwrap();

    let output = run_extract(root);
    assert!(output.status.success(), "extract failed: {:?}", output);

    let frag = fs::read_to_string(root.join("_fragments/sidebar.html")).unwrap();
    assert!(
        frag.contains("class=\"sidebar\""),
        "sidebar fragment file missing or wrong:\n{frag}"
    );

    let a = fs::read_to_string(root.join("a.html")).unwrap();
    assert!(
        a.contains("<!-- fragment:sidebar -->"),
        "page a missing sidebar marker:\n{a}"
    );
}

#[test]
fn extract_user_candidate_appends_to_builtins() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    fs::write(
        root.join("fragments.toml"),
        r#"[[extract.candidates]]
name = "sidebar"
selector = "aside.sidebar"
tag = "aside"
"#,
    )
    .unwrap();

    let nav = "<nav><a href=\"/\">Home</a></nav>";
    let sidebar = "<aside class=\"sidebar\"><h3>Links</h3></aside>";
    let page = |unique: &str| {
        format!("<!DOCTYPE html><html><body>{nav}{sidebar}<main>{unique}</main></body></html>")
    };

    fs::write(root.join("a.html"), page("A")).unwrap();
    fs::write(root.join("b.html"), page("B")).unwrap();

    let output = run_extract(root);
    assert!(output.status.success());

    assert!(
        root.join("_fragments/nav.html").exists(),
        "built-in nav candidate should still fire"
    );
    assert!(
        root.join("_fragments/sidebar.html").exists(),
        "user candidate should fire"
    );
}

// --- Smoke test: pagekit binary delegates fragments commands correctly ---

#[test]
fn sync_delegated_to_fragments_lib() {
    // Verifies pagekit's CLI dispatches sync to the fragments lib without
    // dropping the work. If this test fails after a refactor, the lib
    // wiring in pagekit::main is broken.
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    setup_site(
        root,
        &[("head.html", "<meta charset=\"utf-8\">")],
        &[(
            "index.html",
            "<!-- fragment:head -->\nold\n<!-- /fragment:head -->",
        )],
    );

    let output = run_sync(root);
    assert!(output.status.success());

    let result = fs::read_to_string(root.join("index.html")).unwrap();
    assert!(result.contains("<meta charset=\"utf-8\">"));
    assert!(!result.contains("\nold\n"));
}
