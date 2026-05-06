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

fn run_extract_split(dir: &Path) -> std::process::Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_pagekit"))
        .arg(dir.to_str().unwrap())
        .arg("extract")
        .arg("--split-variants")
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

fn run_check_strict(dir: &Path, extra: &[&str]) -> std::process::Output {
    let mut cmd = std::process::Command::new(env!("CARGO_BIN_EXE_pagekit"));
    cmd.arg(dir.to_str().unwrap()).arg("check").arg("--strict");
    for arg in extra {
        cmd.arg(arg);
    }
    cmd.output().expect("failed to run pagekit")
}

fn run_check(dir: &Path) -> std::process::Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_pagekit"))
        .arg(dir.to_str().unwrap())
        .arg("check")
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
fn extract_legacy_tag_field_silently_ignored() {
    // The `tag = "..."` field was required pre-lol_html; serde drops it
    // silently for legacy configs that still declare it. Fragment still
    // ships, marker still wraps. No error, no warning, just works.
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

    let sidebar = "<aside class=\"sidebar\"><h3>Links</h3></aside>";
    let page = |unique: &str| {
        format!("<!DOCTYPE html><html><body>{sidebar}<main>{unique}</main></body></html>")
    };

    fs::write(root.join("a.html"), page("A")).unwrap();
    fs::write(root.join("b.html"), page("B")).unwrap();

    let output = run_extract(root);
    assert!(
        output.status.success(),
        "legacy tag field must not break extract: {:?}",
        output
    );
    assert!(
        root.join("_fragments/sidebar.html").exists(),
        "fragment should still ship despite legacy tag field"
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

// --- check --strict ---

#[test]
fn check_strict_uniform_passes() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    let nav = "<!-- fragment:nav -->\n<nav><a href=\"/\">Home</a></nav>\n<!-- /fragment:nav -->";
    let footer = "<!-- fragment:footer -->\n<footer>(c) Site</footer>\n<!-- /fragment:footer -->";
    let page = |body: &str| {
        format!("<!DOCTYPE html><html><body>\n{nav}\n<main>{body}</main>\n{footer}\n</body></html>")
    };

    fs::write(root.join("a.html"), page("A")).unwrap();
    fs::write(root.join("b.html"), page("B")).unwrap();
    fs::write(root.join("c.html"), page("C")).unwrap();

    let output = run_check_strict(root, &[]);
    assert!(
        output.status.success(),
        "expected exit 0 for uniform site, got {:?}\nstdout: {}\nstderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("nav"));
    assert!(stdout.contains("footer"));
    assert!(stdout.contains("✓ uniform"));
    assert!(!stdout.contains("⚠ varies"));
}

#[test]
fn check_strict_detects_variance() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    let nav_a =
        "<!-- fragment:nav -->\n<nav><a class=\"default\">Home</a></nav>\n<!-- /fragment:nav -->";
    let nav_b = "<!-- fragment:nav -->\n<nav><a class=\"transparent\">Home</a></nav>\n<!-- /fragment:nav -->";
    let page = |nav: &str, body: &str| {
        format!("<!DOCTYPE html><html><body>\n{nav}\n<main>{body}</main>\n</body></html>")
    };

    fs::write(root.join("a.html"), page(nav_a, "A")).unwrap();
    fs::write(root.join("b.html"), page(nav_a, "B")).unwrap();
    fs::write(root.join("c.html"), page(nav_b, "C")).unwrap();
    fs::write(root.join("d.html"), page(nav_b, "D")).unwrap();

    let output = run_check_strict(root, &[]);
    assert_eq!(
        output.status.code(),
        Some(2),
        "expected exit 2 on variance, got {:?}\nstdout: {}\nstderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("⚠ varies"),
        "stdout missing variance row:\n{stdout}"
    );
    assert!(
        stdout.contains("nav has 2 variants"),
        "stdout missing per-fragment variant block:\n{stdout}"
    );
    // Both 8-hex hashes appear and they differ.
    let hash_count = stdout.matches("hash ").count();
    assert!(
        hash_count >= 2,
        "expected ≥2 'hash ' lines, got {hash_count}:\n{stdout}"
    );
}

#[test]
fn check_strict_with_name_filter() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    // nav uniform across all 3 pages; footer varies between page1 and others.
    let nav = "<!-- fragment:nav -->\n<nav>HOME</nav>\n<!-- /fragment:nav -->";
    let footer_a = "<!-- fragment:footer -->\n<footer>A</footer>\n<!-- /fragment:footer -->";
    let footer_b = "<!-- fragment:footer -->\n<footer>B</footer>\n<!-- /fragment:footer -->";

    let page = |footer: &str, body: &str| {
        format!("<!DOCTYPE html><html><body>\n{nav}\n<main>{body}</main>\n{footer}\n</body></html>")
    };

    fs::write(root.join("a.html"), page(footer_a, "A")).unwrap();
    fs::write(root.join("b.html"), page(footer_b, "B")).unwrap();
    fs::write(root.join("c.html"), page(footer_b, "C")).unwrap();

    // Without filter: footer variance trips exit 2.
    let unfiltered = run_check_strict(root, &[]);
    assert_eq!(
        unfiltered.status.code(),
        Some(2),
        "unfiltered run should detect footer variance"
    );

    // With --name nav: footer variance is not in scope, exit 0.
    let filtered = run_check_strict(root, &["--name", "nav"]);
    assert!(
        filtered.status.success(),
        "expected exit 0 with --name nav, got {:?}\nstdout: {}",
        filtered.status,
        String::from_utf8_lossy(&filtered.stdout),
    );
    let stdout = String::from_utf8_lossy(&filtered.stdout);
    assert!(
        stdout.contains("nav"),
        "filter run missing nav row:\n{stdout}"
    );
    assert!(
        !stdout.contains("footer"),
        "filter run leaked footer:\n{stdout}"
    );
}

// --- D2 transforms: path-relative sync ---

/// Set up a 3-page site at depths 0/1/2 with a footer fragment containing
/// absolute paths. Returns the root path; caller writes fragments.toml.
fn setup_depth_site(root: &Path) {
    fs::write(
        root.join("fragments.toml"),
        "target_dir = \"www\"\n[transforms]\npath_root = \"/\"\n",
    )
    .unwrap();

    let frag_dir = root.join("_fragments");
    fs::create_dir_all(&frag_dir).unwrap();
    fs::write(
        frag_dir.join("footer.html"),
        "<footer>\n<a href=\"/sollentuna/index.html\">Sollentuna</a>\n<a href=\"/about\">About</a>\n<img src=\"/img/logo.png\" alt=\"\">\n</footer>",
    )
    .unwrap();

    let www = root.join("www");
    fs::create_dir_all(www.join("kista")).unwrap();
    fs::create_dir_all(www.join("sv").join("sollentuna")).unwrap();

    let page = "<!DOCTYPE html><html><body>\n<!-- fragment:footer -->\n<footer>old</footer>\n<!-- /fragment:footer -->\n</body></html>";
    fs::write(www.join("index.html"), page).unwrap(); // depth 0
    fs::write(www.join("kista").join("index.html"), page).unwrap(); // depth 1
    fs::write(www.join("sv").join("sollentuna").join("index.html"), page).unwrap();
    // depth 2
}

#[test]
fn sync_rewrites_paths_per_depth() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_depth_site(root);

    let output = run_sync(root);
    assert!(
        output.status.success(),
        "sync failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let depth0 = fs::read_to_string(root.join("www/index.html")).unwrap();
    assert!(
        depth0.contains("href=\"sollentuna/index.html\""),
        "depth 0: expected stripped leading slash, got:\n{depth0}"
    );
    assert!(depth0.contains("href=\"about\""));
    assert!(depth0.contains("src=\"img/logo.png\""));

    let depth1 = fs::read_to_string(root.join("www/kista/index.html")).unwrap();
    assert!(
        depth1.contains("href=\"../sollentuna/index.html\""),
        "depth 1: expected one ../, got:\n{depth1}"
    );
    assert!(depth1.contains("src=\"../img/logo.png\""));

    let depth2 = fs::read_to_string(root.join("www/sv/sollentuna/index.html")).unwrap();
    assert!(
        depth2.contains("href=\"../../sollentuna/index.html\""),
        "depth 2: expected two ../, got:\n{depth2}"
    );
    assert!(depth2.contains("src=\"../../img/logo.png\""));
}

#[test]
fn sync_preserves_external_urls() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    fs::write(
        root.join("fragments.toml"),
        "[transforms]\npath_root = \"/\"\n",
    )
    .unwrap();

    let frag_dir = root.join("_fragments");
    fs::create_dir_all(&frag_dir).unwrap();
    fs::write(
        frag_dir.join("contact.html"),
        "<div>\n<a href=\"https://example.com\">x</a>\n<a href=\"http://example.com\">y</a>\n<a href=\"mailto:hi@example.com\">m</a>\n<a href=\"tel:+46123\">t</a>\n<a href=\"#section\">a</a>\n<a href=\"//cdn.example.com/x\">cdn</a>\n<a href=\"relative/path.html\">r</a>\n<img src=\"data:image/png;base64,xx\">\n</div>",
    )
    .unwrap();

    fs::create_dir_all(root.join("a/b")).unwrap();
    let page = "<!DOCTYPE html><html><body>\n<!-- fragment:contact -->\n<div>old</div>\n<!-- /fragment:contact -->\n</body></html>";
    fs::write(root.join("a/b/page.html"), page).unwrap();

    let output = run_sync(root);
    assert!(
        output.status.success(),
        "sync failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let result = fs::read_to_string(root.join("a/b/page.html")).unwrap();
    for preserved in [
        "https://example.com",
        "http://example.com",
        "mailto:hi@example.com",
        "tel:+46123",
        "#section",
        "//cdn.example.com/x",
        "relative/path.html",
        "data:image/png;base64,xx",
    ] {
        assert!(
            result.contains(preserved),
            "expected preserved value '{preserved}' in:\n{result}"
        );
    }
    // No accidental ../ injection
    assert!(
        !result.contains("../https"),
        "external URL was incorrectly rewritten:\n{result}"
    );
}

#[test]
fn sync_idempotent_with_transforms() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_depth_site(root);

    run_sync(root);
    let after_first = fs::read_to_string(root.join("www/kista/index.html")).unwrap();

    let second = run_sync(root);
    assert!(second.status.success());

    let after_second = fs::read_to_string(root.join("www/kista/index.html")).unwrap();
    assert_eq!(
        after_first, after_second,
        "second sync produced a diff (transforms are not idempotent)"
    );
}

#[test]
fn check_uses_same_hooks_as_sync() {
    // Consistency contract from fragments v0.6.0: sync and check must
    // pass the same hooks. After a freshly-synced site, `pagekit check`
    // (no flags) should report zero staleness.
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_depth_site(root);

    let sync_out = run_sync(root);
    assert!(sync_out.status.success());

    let check_out = run_check(root);
    assert!(
        check_out.status.success(),
        "check should pass after sync, got status {:?}\nstdout: {}\nstderr: {}",
        check_out.status,
        String::from_utf8_lossy(&check_out.stdout),
        String::from_utf8_lossy(&check_out.stderr)
    );
    let stdout = String::from_utf8_lossy(&check_out.stdout);
    assert!(
        stdout.contains("up to date"),
        "expected 'up to date' message, got:\n{stdout}"
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

// --- extract --split-variants ---

fn variant_site(root: &Path) -> (&'static str, &'static str) {
    // Two distinct nav variants, three pages each. Six total — well above
    // the ≥2 page threshold for both. Footer is uniform and should keep
    // the plain `footer` name even with --split-variants on.
    let nav_a = "<nav class=\"transparent\"><a href=\"/\">Home</a></nav>";
    let nav_b = "<nav class=\"default\"><a href=\"/\">Home</a></nav>";
    let footer = "<footer>(c) SiteCo</footer>";
    let page = |nav: &str, slug: &str| {
        format!("<!DOCTYPE html><html><body>{nav}<main>{slug}</main>{footer}</body></html>")
    };

    for (name, nav) in [
        ("a.html", nav_a),
        ("b.html", nav_a),
        ("c.html", nav_a),
        ("d.html", nav_b),
        ("e.html", nav_b),
        ("f.html", nav_b),
    ] {
        fs::write(root.join(name), page(nav, name)).unwrap();
    }

    (nav_a, nav_b)
}

#[test]
fn extract_split_variants_emits_n_files() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    let (nav_a, nav_b) = variant_site(root);

    let output = run_extract_split(root);
    assert!(
        output.status.success(),
        "extract --split-variants failed: {:?}",
        output
    );

    let frag_1 =
        fs::read_to_string(root.join("_fragments/nav-1.html")).expect("nav-1.html should exist");
    let frag_2 =
        fs::read_to_string(root.join("_fragments/nav-2.html")).expect("nav-2.html should exist");

    // With tied counts (3 each), tiebreak is ascending content. nav_a
    // ('class="transparent"') sorts before nav_b ('class="default"')
    // by content? No — 'd' < 't' so nav_b ('default') comes first.
    let variants: std::collections::HashSet<_> = [nav_a, nav_b].iter().copied().collect();
    assert!(
        variants.contains(frag_1.as_str()) && variants.contains(frag_2.as_str()),
        "fragment files must each match one of the two variant contents:\n  nav-1: {frag_1}\n  nav-2: {frag_2}",
    );
    assert_ne!(
        frag_1, frag_2,
        "the two variant fragments must hold distinct content"
    );

    // Plain `nav.html` must NOT exist when split mode produced -1/-2.
    assert!(
        !root.join("_fragments/nav.html").exists(),
        "plain nav.html should not be written under --split-variants"
    );
}

#[test]
fn extract_split_variants_rewrites_markers() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    let (nav_a, _nav_b) = variant_site(root);

    let output = run_extract_split(root);
    assert!(output.status.success(), "extract failed: {:?}", output);

    // Determine which variant landed in nav-1 by reading the file.
    let frag_1 = fs::read_to_string(root.join("_fragments/nav-1.html")).unwrap();
    let nav_1_is_a = frag_1 == nav_a;

    let pages_with_a = ["a.html", "b.html", "c.html"];
    let pages_with_b = ["d.html", "e.html", "f.html"];

    let (nav_1_pages, nav_2_pages) = if nav_1_is_a {
        (&pages_with_a, &pages_with_b)
    } else {
        (&pages_with_b, &pages_with_a)
    };

    for page in nav_1_pages.iter() {
        let content = fs::read_to_string(root.join(page)).unwrap();
        assert!(
            content.contains("<!-- fragment:nav-1 -->"),
            "{page} should carry nav-1 marker:\n{content}"
        );
        assert!(
            !content.contains("<!-- fragment:nav-2 -->"),
            "{page} must not carry nav-2 marker"
        );
    }
    for page in nav_2_pages.iter() {
        let content = fs::read_to_string(root.join(page)).unwrap();
        assert!(
            content.contains("<!-- fragment:nav-2 -->"),
            "{page} should carry nav-2 marker:\n{content}"
        );
        assert!(
            !content.contains("<!-- fragment:nav-1 -->"),
            "{page} must not carry nav-1 marker"
        );
    }

    // Re-run is a no-op (idempotent).
    let before: Vec<String> = ["a.html", "d.html"]
        .iter()
        .map(|p| fs::read_to_string(root.join(p)).unwrap())
        .collect();
    let _ = run_extract_split(root);
    let after: Vec<String> = ["a.html", "d.html"]
        .iter()
        .map(|p| fs::read_to_string(root.join(p)).unwrap())
        .collect();
    assert_eq!(before, after, "second --split-variants run must be a no-op");
}

#[test]
fn extract_default_warns_on_variants() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    variant_site(root);

    let output = run_extract(root);
    assert!(output.status.success(), "extract failed: {:?}", output);

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("variants") && stdout.contains("--split-variants"),
        "default extract on a multi-variant site must surface a hint:\n{stdout}"
    );

    // Without the flag, only one nav fragment ships (the dominant), and
    // it lands at the plain name.
    assert!(
        root.join("_fragments/nav.html").exists(),
        "default mode keeps the plain nav.html"
    );
    assert!(
        !root.join("_fragments/nav-1.html").exists(),
        "default mode must NOT emit suffixed variants"
    );
    assert!(
        !root.join("_fragments/nav-2.html").exists(),
        "default mode must NOT emit suffixed variants"
    );
}
