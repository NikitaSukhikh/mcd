//! CLI integration tests for stable command behavior.

use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Output},
    time::{SystemTime, UNIX_EPOCH},
};

use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

fn mcd() -> Command {
    Command::new(env!("CARGO_BIN_EXE_mcd"))
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates")
        .parent()
        .expect("repo")
        .to_path_buf()
}

fn example_package(name: &str) -> PathBuf {
    repo_root()
        .join("examples")
        .join(name)
        .join(format!("{name}.mcd"))
}

fn temp_path(name: &str) -> PathBuf {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    std::env::temp_dir().join(format!("mcd-cli-test-{}-{now}-{name}", std::process::id()))
}

fn run(command: &mut Command) -> Output {
    command.output().expect("run mcd")
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("stdout utf8")
}

fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).expect("stderr utf8")
}

#[test]
fn inspect_and_validate_minimal_fixture() {
    let minimal = example_package("minimal");

    let inspect = run(mcd().arg("inspect").arg(&minimal));
    assert!(inspect.status.success(), "{}", stderr(&inspect));
    let inspect_json: serde_json::Value =
        serde_json::from_str(&stdout(&inspect)).expect("inspect json");
    assert_eq!(inspect_json["format"], "MCD");
    assert_eq!(inspect_json["entrypoint"], "content/main.md");

    let validate = run(mcd().arg("validate").arg(&minimal));
    assert!(validate.status.success(), "{}", stderr(&validate));
    assert_eq!(stdout(&validate), "valid\n");
    assert!(stderr(&validate).is_empty());
}

#[test]
fn validate_json_failure_has_stable_shape_and_nonzero_exit() {
    let package = temp_path("missing-manifest.mcd");
    write_zip(
        &package,
        &[
            ("mimetype", "application/vnd.mcd+zip\n"),
            ("content/main.md", "# Missing manifest\n"),
        ],
    );

    let output = run(mcd()
        .arg("validate")
        .arg(&package)
        .arg("--format")
        .arg("json"));

    assert!(!output.status.success());
    let json: serde_json::Value = serde_json::from_str(&stdout(&output)).expect("json diagnostics");
    assert_eq!(json["valid"], false);
    assert_eq!(json["diagnostics"][0]["level"], "error");
    assert_eq!(json["diagnostics"][0]["code"], "manifest.missing");
    assert!(stderr(&output).contains("Package is missing manifest.json."));

    let _ = fs::remove_file(package);
}

#[test]
fn add_annotation_updates_manifest_and_metadata() {
    let root = temp_path("add-annotation");
    fs::create_dir_all(&root).expect("temp root");
    let package = root.join("annotated.mcd");
    write_zip(
        &package,
        &[
            ("mimetype", "application/vnd.mcd+zip\n"),
            (
                "manifest.json",
                r#"{"format":"MCD","version":"0.1","profile":"MCD-Core","entrypoint":"content/main.md"}"#,
            ),
            ("content/main.md", "# Annotated\n\nNeeds review.\n"),
        ],
    );

    let add = run(mcd()
        .arg("add-annotation")
        .arg(&package)
        .arg("Check the opening paragraph.")
        .arg("--page")
        .arg("content/main.md")
        .arg("--line")
        .arg("3")
        .arg("--id")
        .arg("review-intro"));
    assert!(add.status.success(), "{}", stderr(&add));
    assert_eq!(stdout(&add), "review-intro\n");

    let validate = run(mcd().arg("validate").arg(&package));
    assert!(validate.status.success(), "{}", stderr(&validate));

    let annotations = run(mcd().arg("extract").arg(&package).arg("--annotations"));
    assert!(annotations.status.success(), "{}", stderr(&annotations));
    let annotation_json: serde_json::Value =
        serde_json::from_str(&stdout(&annotations)).expect("annotation json");
    assert_eq!(annotation_json["annotations"][0]["id"], "review-intro");
    assert_eq!(
        annotation_json["annotations"][0]["body"],
        "Check the opening paragraph."
    );
    assert_eq!(
        annotation_json["annotations"][0]["target"]["path"],
        "content/main.md"
    );
    assert_eq!(
        annotation_json["annotations"][0]["target"]["source"]["startLine"],
        3
    );

    let filtered = run(mcd()
        .arg("extract")
        .arg(&package)
        .arg("--export")
        .arg("annotations")
        .arg("--page")
        .arg("content/main.md")
        .arg("--line")
        .arg("3"));
    assert!(filtered.status.success(), "{}", stderr(&filtered));
    let filtered_json: serde_json::Value =
        serde_json::from_str(&stdout(&filtered)).expect("filtered annotation json");
    assert_eq!(filtered_json["annotations"][0]["id"], "review-intro");

    let missing = run(mcd()
        .arg("extract")
        .arg(&package)
        .arg("--export")
        .arg("annotations")
        .arg("--page")
        .arg("content/main.md")
        .arg("--line")
        .arg("2"));
    assert!(missing.status.success(), "{}", stderr(&missing));
    let missing_json: serde_json::Value =
        serde_json::from_str(&stdout(&missing)).expect("missing annotation json");
    assert_eq!(missing_json["annotations"].as_array().unwrap().len(), 0);
    assert_eq!(missing_json["message"], "no annotations found");

    let rendered_markdown = root.join("annotated.md");
    let render = run(mcd()
        .arg("render")
        .arg(&package)
        .arg("--markdown")
        .arg("--output")
        .arg(&rendered_markdown));
    assert!(render.status.success(), "{}", stderr(&render));
    let markdown = fs::read_to_string(&rendered_markdown).expect("rendered markdown");
    assert!(markdown.contains("(@annotation: [Check the opening paragraph.])"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn extract_modes_emit_stdout_and_reject_ambiguous_selection() {
    let revenue = example_package("revenue-report");
    let charts = run(mcd().arg("extract").arg(&revenue).arg("--charts"));
    assert!(charts.status.success(), "{}", stderr(&charts));
    let chart_json: serde_json::Value = serde_json::from_str(&stdout(&charts)).expect("chart json");
    assert_eq!(chart_json["charts"][0]["tableId"], "revenue");
    assert!(chart_json["charts"][0]["viewId"].is_string());
    assert!(chart_json["charts"][0]["rows"].is_array());
    assert!(stderr(&charts).is_empty());

    let visual = example_package("visual-report");
    let images = run(mcd().arg("extract").arg(&visual).arg("--images"));
    assert!(images.status.success(), "{}", stderr(&images));
    let image_json: serde_json::Value = serde_json::from_str(&stdout(&images)).expect("image json");
    assert_eq!(
        image_json["images"][0]["asset"],
        "assets/process-diagram.svg"
    );
    assert!(!stdout(&images).contains("<svg"));

    let annotated = temp_path("annotated.mcd");
    write_zip(
        &annotated,
        &[
            ("mimetype", "application/vnd.mcd+zip\n"),
            (
                "manifest.json",
                r#"{"format":"MCD","version":"0.1","profile":"MCD-Core","entrypoint":"content/main.md","annotations":[{"id":"review-intro","metadata":"annotations/review-intro.annotation.json"}]}"#,
            ),
            ("content/main.md", "# Annotated\n\nNeeds review.\n"),
            (
                "annotations/review-intro.annotation.json",
                r#"{"id":"review-intro","target":{"type":"document"},"kind":"comment","status":"open","body":"Review the opening copy.","labels":["review"]}"#,
            ),
        ],
    );
    let annotations = run(mcd().arg("extract").arg(&annotated).arg("--annotations"));
    assert!(annotations.status.success(), "{}", stderr(&annotations));
    let annotation_json: serde_json::Value =
        serde_json::from_str(&stdout(&annotations)).expect("annotation json");
    assert_eq!(annotation_json["annotations"][0]["id"], "review-intro");
    assert_eq!(annotation_json["annotations"][0]["kind"], "comment");
    let _ = fs::remove_file(annotated);

    let ambiguous = run(mcd()
        .arg("extract")
        .arg(&revenue)
        .arg("--json")
        .arg("--tables"));
    assert!(!ambiguous.status.success());
    assert!(stdout(&ambiguous).is_empty());
    assert!(stderr(&ambiguous).contains("choose exactly one extraction mode"));
}

#[test]
fn render_html_writes_standalone_output() {
    let root = temp_path("render-html");
    fs::create_dir_all(&root).expect("temp root");
    let output_path = root.join("report.html");

    let render = run(mcd()
        .arg("render")
        .arg(example_package("revenue-report"))
        .arg("--html")
        .arg("--output")
        .arg(&output_path));

    assert!(render.status.success(), "{}", stderr(&render));
    assert!(stdout(&render).is_empty());
    assert!(stderr(&render).is_empty());

    let html = fs::read_to_string(&output_path).expect("rendered html");
    assert!(html.contains("<!doctype html>"));
    assert!(html.contains("data-mcd-ref=\"revenue-table\""));
    assert!(html.contains("data-mcd-ref=\"revenue-chart\""));
    assert!(html.contains("<svg class=\"mcd-chart\" role=\"img\""));
    assert!(html.contains("GBP 125000"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn render_markdown_writes_plain_projection_with_embedded_tables() {
    let root = temp_path("render-markdown");
    fs::create_dir_all(&root).expect("temp root");
    let output_path = root.join("report.md");

    let render = run(mcd()
        .arg("render")
        .arg(example_package("revenue-report"))
        .arg("--markdown")
        .arg("--output")
        .arg(&output_path));

    assert!(render.status.success(), "{}", stderr(&render));
    assert!(stdout(&render).is_empty());
    assert!(stderr(&render).is_empty());

    let markdown = fs::read_to_string(&output_path).expect("rendered markdown");
    assert!(markdown.contains("# Revenue Report"));
    assert!(markdown.contains("| Quarter | Revenue |"));
    assert!(markdown.contains("| Q1 | GBP 125000 |"));
    assert!(
        markdown.contains(
            "**Chart metadata:** table `revenue`, view `quarterly-bar-chart`, type `bar`."
        )
    );
    assert!(!markdown.contains(":::table"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn convert_pdf_writes_valid_mcd_package() {
    let root = temp_path("convert-pdf");
    fs::create_dir_all(&root).expect("temp root");
    let pdf = root.join("source.pdf");
    let package = root.join("source.mcd");
    fs::write(&pdf, minimal_pdf("Hello from a PDF")).expect("pdf");

    let convert = run(mcd()
        .arg("convert-pdf")
        .arg(&pdf)
        .arg("--output")
        .arg(&package)
        .arg("--title")
        .arg("Imported PDF"));
    assert!(convert.status.success(), "{}", stderr(&convert));
    assert!(stdout(&convert).is_empty());
    assert!(stderr(&convert).is_empty());

    let validate = run(mcd().arg("validate").arg(&package));
    assert!(validate.status.success(), "{}", stderr(&validate));

    let markdown = run(mcd().arg("extract").arg(&package).arg("--markdown"));
    assert!(markdown.status.success(), "{}", stderr(&markdown));
    assert!(stdout(&markdown).contains("# Imported PDF"));
    assert!(stdout(&markdown).contains("Hello from a PDF"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn plain_markdown_renamed_to_mcd_validates_and_renders() {
    let root = temp_path("plain-markdown");
    fs::create_dir_all(&root).expect("temp root");
    let package = root.join("notes.mcd");
    let output_path = root.join("notes.html");
    fs::write(
        &package,
        "# Plain Markdown\n\nThis was saved as an `.mcd` file.\n",
    )
    .expect("markdown");

    let validate = run(mcd().arg("validate").arg(&package));
    assert!(validate.status.success(), "{}", stderr(&validate));
    assert_eq!(stdout(&validate), "valid\n");

    let inspect = run(mcd().arg("inspect").arg(&package));
    assert!(inspect.status.success(), "{}", stderr(&inspect));
    let inspect_json: serde_json::Value =
        serde_json::from_str(&stdout(&inspect)).expect("inspect json");
    assert_eq!(inspect_json["entrypoint"], "content/main.md");
    assert_eq!(inspect_json["entries"], 3);

    let render = run(mcd()
        .arg("render")
        .arg(&package)
        .arg("--html")
        .arg("--output")
        .arg(&output_path));
    assert!(render.status.success(), "{}", stderr(&render));

    let html = fs::read_to_string(&output_path).expect("rendered html");
    assert!(html.contains("<h1"));
    assert!(html.contains("Plain Markdown"));
    assert!(html.contains("This was saved as an .mcd file."));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn init_pack_and_validate_minimal_document() {
    let root = temp_path("init-pack");
    let unpacked = root.join("unpacked");
    let package = root.join("created.mcd");
    fs::create_dir_all(&root).expect("temp root");

    let init = run(mcd().arg("init").arg(&unpacked));
    assert!(init.status.success(), "{}", stderr(&init));

    let pack = run(mcd()
        .arg("pack")
        .arg(&unpacked)
        .arg("--output")
        .arg(&package));
    assert!(pack.status.success(), "{}", stderr(&pack));

    let validate = run(mcd().arg("validate").arg(&package));
    assert!(validate.status.success(), "{}", stderr(&validate));
    assert_eq!(stdout(&validate), "valid\n");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn pack_adds_root_mimetype_when_source_omits_it() {
    let root = temp_path("pack-mimetype");
    let unpacked = root.join("unpacked");
    let content = unpacked.join("content");
    let package = root.join("created.mcd");
    fs::create_dir_all(&content).expect("content dir");
    fs::write(
        unpacked.join("manifest.json"),
        r#"{"format":"MCD","version":"0.1","profile":"MCD-Core","entrypoint":"content/main.md"}"#,
    )
    .expect("manifest");
    fs::write(content.join("main.md"), "# Untitled\n").expect("markdown");

    let pack = run(mcd()
        .arg("pack")
        .arg(&unpacked)
        .arg("--output")
        .arg(&package));
    assert!(pack.status.success(), "{}", stderr(&pack));

    let validate = run(mcd().arg("validate").arg(&package));
    assert!(validate.status.success(), "{}", stderr(&validate));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn unpack_writes_safe_entries_and_rejects_unsafe_archive_entries() {
    let root = temp_path("unpack");
    let output_dir = root.join("safe");
    fs::create_dir_all(&root).expect("temp root");

    let unpack = run(mcd()
        .arg("unpack")
        .arg(example_package("minimal"))
        .arg("--output")
        .arg(&output_dir));
    assert!(unpack.status.success(), "{}", stderr(&unpack));
    assert!(output_dir.join("mimetype").is_file());
    assert!(output_dir.join("manifest.json").is_file());
    assert!(output_dir.join("content").join("main.md").is_file());

    let unsafe_package = root.join("unsafe.mcd");
    write_zip(
        &unsafe_package,
        &[
            ("mimetype", "application/vnd.mcd+zip\n"),
            ("../evil.txt", "nope"),
        ],
    );
    let unsafe_output = root.join("unsafe-output");
    let rejected = run(mcd()
        .arg("unpack")
        .arg(&unsafe_package)
        .arg("--output")
        .arg(&unsafe_output));
    assert!(!rejected.status.success());
    assert!(stderr(&rejected).contains("security.path.invalid"));
    assert!(!root.join("evil.txt").exists());

    let _ = fs::remove_dir_all(root);
}

fn write_zip(path: &Path, entries: &[(&str, &str)]) {
    let file = fs::File::create(path).expect("create zip");
    let mut writer = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);

    for (name, content) in entries {
        writer.start_file(*name, options).expect("start file");
        writer.write_all(content.as_bytes()).expect("write entry");
    }

    writer.finish().expect("finish zip");
}

fn minimal_pdf(text: &str) -> Vec<u8> {
    let escaped = text
        .replace('\\', r"\\")
        .replace('(', r"\(")
        .replace(')', r"\)");
    let content = format!("BT /F1 24 Tf 100 700 Td ({escaped}) Tj ET");
    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_owned(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_owned(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >>".to_owned(),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_owned(),
        format!("<< /Length {} >>\nstream\n{}\nendstream", content.len(), content),
    ];
    let mut bytes = b"%PDF-1.4\n".to_vec();
    let mut offsets = Vec::new();
    for (index, object) in objects.iter().enumerate() {
        offsets.push(bytes.len());
        bytes.extend_from_slice(format!("{} 0 obj\n{}\nendobj\n", index + 1, object).as_bytes());
    }
    let xref_offset = bytes.len();
    bytes.extend_from_slice(format!("xref\n0 {}\n", objects.len() + 1).as_bytes());
    bytes.extend_from_slice(b"0000000000 65535 f \n");
    for offset in offsets {
        bytes.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    bytes.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{}\n%%EOF\n",
            objects.len() + 1,
            xref_offset
        )
        .as_bytes(),
    );
    bytes
}
