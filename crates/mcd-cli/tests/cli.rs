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

    let auto = example_package("auto-manufacturer-tech-spec");
    let schemas = run(mcd().arg("extract").arg(&auto).arg("--schemas"));
    assert!(schemas.status.success(), "{}", stderr(&schemas));
    let schema_json: serde_json::Value =
        serde_json::from_str(&stdout(&schemas)).expect("schema json");
    assert_eq!(schema_json["schemas"][0]["primaryKey"][0], "variant_id");
    assert_eq!(
        schema_json["schemas"][0]["columns"][6]["unit"]["code"],
        "mm"
    );
    assert_eq!(
        schema_json["schemas"][3]["foreignKeys"][0]["references"]["table"],
        "vehicle_variant_configuration_specs"
    );

    let external_data = run(mcd().arg("extract").arg(&auto).arg("--external-data"));
    assert!(external_data.status.success(), "{}", stderr(&external_data));
    let external_json: serde_json::Value =
        serde_json::from_str(&stdout(&external_data)).expect("external data json");
    assert_eq!(
        external_json["externalData"][0]["id"],
        "raw-auto-spec-source"
    );

    let provenance = run(mcd().arg("extract").arg(&auto).arg("--provenance"));
    assert!(provenance.status.success(), "{}", stderr(&provenance));
    let provenance_json: serde_json::Value =
        serde_json::from_str(&stdout(&provenance)).expect("provenance json");
    assert_eq!(
        provenance_json["provenance"]["activities"][0]["id"],
        "derive-example-package"
    );

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
fn query_runs_read_only_sql_against_package_tables() {
    let revenue = example_package("revenue-report");

    let aggregate = run(mcd()
        .arg("query")
        .arg(&revenue)
        .arg("select count(*) as rows, max(revenue_gbp) as max_revenue from revenue")
        .arg("--format")
        .arg("json"));
    assert!(aggregate.status.success(), "{}", stderr(&aggregate));
    let json: serde_json::Value = serde_json::from_str(&stdout(&aggregate)).expect("query json");
    assert_eq!(json["rows"][0]["rows"], 4);
    assert_eq!(json["rows"][0]["max_revenue"], 158250.0);

    let ordered = run(mcd()
        .arg("query")
        .arg(&revenue)
        .arg("select quarter, revenue_gbp from revenue order by revenue_gbp desc limit 1")
        .arg("--format")
        .arg("csv"));
    assert!(ordered.status.success(), "{}", stderr(&ordered));
    assert_eq!(stdout(&ordered), "quarter,revenue_gbp\nQ4,158250\n");

    let rejected = run(mcd().arg("query").arg(&revenue).arg("delete from revenue"));
    assert!(!rejected.status.success());
    assert!(stderr(&rejected).contains("query must be a SELECT statement"));

    let auto = example_package("auto-manufacturer-tech-spec");
    let relationships = run(mcd()
        .arg("query")
        .arg(&auto)
        .arg("select table_id, column_name, ref_table_id, ref_column_name from mcd_foreign_keys")
        .arg("--format")
        .arg("json"));
    assert!(relationships.status.success(), "{}", stderr(&relationships));
    let json: serde_json::Value =
        serde_json::from_str(&stdout(&relationships)).expect("relationships json");
    assert_eq!(
        json["rows"][0]["table_id"],
        "chassis_brake_validation_specs"
    );
    assert_eq!(json["rows"][0]["column_name"], "vehicle_variant");
    assert_eq!(
        json["rows"][0]["ref_table_id"],
        "vehicle_variant_configuration_specs"
    );
    assert_eq!(json["rows"][0]["ref_column_name"], "variant_id");

    let pragma = run(mcd()
        .arg("query")
        .arg(&auto)
        .arg("select name, pk from pragma_table_info('vehicle_variant_configuration_specs') where pk > 0")
        .arg("--format")
        .arg("json"));
    assert!(pragma.status.success(), "{}", stderr(&pragma));
    let json: serde_json::Value = serde_json::from_str(&stdout(&pragma)).expect("pragma json");
    assert_eq!(json["rows"][0]["name"], "variant_id");
    assert_eq!(json["rows"][0]["pk"], 1);

    let batch = run(mcd()
        .arg("query-batch")
        .arg(&revenue)
        .arg("--sql")
        .arg("select count(*) as rows from revenue")
        .arg("--sql")
        .arg("select quarter from revenue order by revenue_gbp desc limit 1"));
    assert!(batch.status.success(), "{}", stderr(&batch));
    let json: serde_json::Value = serde_json::from_str(&stdout(&batch)).expect("batch json");
    assert_eq!(json["queryCount"], 2);
    assert_eq!(json["queries"][0]["result"]["rows"][0]["rows"], 4);
    assert_eq!(json["queries"][1]["result"]["rows"][0]["quarter"], "Q4");
}

#[test]
fn search_finds_markdown_and_schema_hits() {
    let auto = example_package("auto-manufacturer-tech-spec");

    let output = run(mcd()
        .arg("search")
        .arg(&auto)
        .arg("thermal_limit_deg_c coolant V50D")
        .arg("--format")
        .arg("json")
        .arg("--limit")
        .arg("5"));
    assert!(output.status.success(), "{}", stderr(&output));
    let hits: serde_json::Value = serde_json::from_str(&stdout(&output)).expect("search json");
    let hits = hits.as_array().expect("hits array");
    assert!(hits.iter().any(|hit| {
        hit["kind"] == "markdown"
            && hit["path"] == "content/main.md"
            && hit["text"]
                .as_str()
                .is_some_and(|text| text.contains("thermal_limit_deg_c"))
    }));

    let schema = run(mcd()
        .arg("search")
        .arg(&auto)
        .arg("variant_id")
        .arg("--kind")
        .arg("schema")
        .arg("--format")
        .arg("json")
        .arg("--limit")
        .arg("5"));
    assert!(schema.status.success(), "{}", stderr(&schema));
    let schema_hits: serde_json::Value =
        serde_json::from_str(&stdout(&schema)).expect("schema search json");
    assert!(
        schema_hits
            .as_array()
            .expect("schema hits")
            .iter()
            .all(|hit| {
                hit["kind"] == "schema"
                    && hit["path"]
                        .as_str()
                        .is_some_and(|path| path.ends_with(".schema.json"))
            })
    );
}

#[test]
fn tools_lists_python_sql_and_optional_package_schema() {
    let generic = run(mcd().arg("tools"));
    assert!(generic.status.success(), "{}", stderr(&generic));
    assert!(stdout(&generic).contains("Python top-level commands:"));
    assert!(stdout(&generic).contains("mcd.query(path, sql) -> QueryResult"));
    assert!(stdout(&generic).contains("SQL CLI examples:"));

    let revenue = example_package("revenue-report");
    let text = run(mcd().arg("tools").arg(&revenue));
    assert!(text.status.success(), "{}", stderr(&text));
    assert!(stdout(&text).contains("Package tables:"));
    assert!(stdout(&text).contains("- revenue (tables/revenue.csv)"));
    assert!(stdout(&text).contains("revenue_gbp: decimal"));

    let json_output = run(mcd().arg("tools").arg(&revenue).arg("--format").arg("json"));
    assert!(json_output.status.success(), "{}", stderr(&json_output));
    let value: serde_json::Value = serde_json::from_str(&stdout(&json_output)).expect("tools json");
    assert_eq!(value["python"]["import"], "import mcd");
    assert_eq!(value["package"]["tables"][0]["id"], "revenue");
    assert_eq!(
        value["package"]["tables"][0]["columns"][1]["name"],
        "revenue_gbp"
    );

    let auto = example_package("auto-manufacturer-tech-spec");
    let auto_text = run(mcd().arg("tools").arg(&auto));
    assert!(auto_text.status.success(), "{}", stderr(&auto_text));
    assert!(stdout(&auto_text).contains("primary key: variant_id"));
    assert!(stdout(&auto_text).contains(
        "foreign key: (vehicle_variant) -> vehicle_variant_configuration_specs(variant_id)"
    ));
    assert!(stdout(&auto_text).contains("wheelbase_mm: integer, unit mm"));

    let auto_json_output = run(mcd().arg("tools").arg(&auto).arg("--format").arg("json"));
    assert!(
        auto_json_output.status.success(),
        "{}",
        stderr(&auto_json_output)
    );
    let auto_value: serde_json::Value =
        serde_json::from_str(&stdout(&auto_json_output)).expect("tools auto json");
    assert_eq!(
        auto_value["package"]["tables"][0]["primaryKey"][0],
        "variant_id"
    );
    assert_eq!(
        auto_value["package"]["tables"][0]["columns"][6]["unit"]["code"],
        "mm"
    );
    assert_eq!(
        auto_value["package"]["externalData"][0]["id"],
        "raw-auto-spec-source"
    );
    assert_eq!(
        auto_value["package"]["provenance"],
        "provenance/provenance.json"
    );
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
fn render_html_writes_project_directory() {
    let root = temp_path("render-html-project");
    fs::create_dir_all(&root).expect("temp root");
    let output_path = root.join("render");

    let render = run(mcd()
        .arg("render")
        .arg(example_package("visual-report"))
        .arg("--html")
        .arg("--output")
        .arg(&output_path));

    assert!(render.status.success(), "{}", stderr(&render));
    assert!(stdout(&render).is_empty());
    assert!(stderr(&render).is_empty());

    let html = fs::read_to_string(output_path.join("index.html")).expect("index html");
    let css = fs::read_to_string(output_path.join("styles.css")).expect("styles css");
    assert!(html.contains("<!doctype html>"));
    assert!(html.contains("<link rel=\"stylesheet\" href=\"styles.css\">"));
    assert!(html.contains("src=\"assets/process-diagram.svg\""));
    assert!(css.contains("@page"));
    assert!(output_path.join("assets/process-diagram.svg").is_file());

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
