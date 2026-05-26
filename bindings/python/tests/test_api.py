from __future__ import annotations

import zipfile
from pathlib import Path

import pytest

import mcd


ROOT = Path(__file__).resolve().parents[3]


def example(name: str) -> Path:
    return ROOT / "examples" / name / f"{name}.mcd"


def test_open_validate_blocks_and_markdown() -> None:
    doc = mcd.open(example("revenue-report"))

    validation = doc.validate()
    assert validation.valid
    assert bool(validation)
    assert validation.diagnostics == []

    blocks = doc.blocks()
    assert blocks
    assert blocks[0].id.startswith("block-")
    assert blocks[0].type == "heading"
    assert blocks[0].as_dict()["text"] == "Revenue Report"

    assert ":::table" in doc.markdown()
    expanded = doc.markdown(expand_tables=True)
    assert "| Quarter | Revenue |" in expanded
    assert "GBP 125000" in expanded


def test_table_access_and_rows() -> None:
    table = mcd.open(example("revenue-report")).table("revenue")

    assert table.id == "revenue"
    assert table.source == "tables/revenue.csv"
    assert table.schema.id == "revenue"
    assert table.schema.columns[0]["name"] == "quarter"
    assert table.rows()[0]["quarter"] == "Q1"
    assert table.rows()[0]["revenue_gbp"] == "125000"
    assert table.typed_rows()[0]["revenue_gbp"] == {
        "type": "decimal",
        "value": "125000",
    }


def test_chart_access_source_rows_and_markdown() -> None:
    chart = mcd.open(example("revenue-report")).chart("revenue-chart")

    assert chart.table_id == "revenue"
    assert chart.view_id == "quarterly-bar-chart"
    assert chart.placement_ref == "revenue-chart"
    assert chart.rows()[0]["quarter"] == "Q1"
    assert chart.rows()[0]["revenue_gbp"] == "125000"
    assert chart.layout() is None
    assert "| Quarter | Revenue |" in chart.to_markdown_table()


def test_image_metadata_access() -> None:
    image = mcd.open(example("visual-report")).image("process-diagram")

    assert image.asset_path == "assets/process-diagram.svg"
    assert image.role == "diagram"
    assert image.alt
    assert image.caption
    assert image.intrinsic_size == {"width": 640, "height": 180, "unit": "px"}


def test_agent_context_options() -> None:
    doc = mcd.open(example("revenue-report"))

    context = doc.to_agent_context(include_tables=False)
    assert context["sourcePath"] == "content/main.md"
    assert "tables" not in context
    assert context["charts"][0]["tableId"] == "revenue"


def test_provenance_external_data_and_relationship_shortcuts() -> None:
    doc = mcd.open(example("auto-manufacturer-tech-spec"))

    external_data = doc.external_data()
    assert external_data[0]["id"] == "raw-auto-spec-source"
    assert external_data[0]["mediaType"] == "text/csv"

    provenance = doc.provenance()
    assert provenance is not None
    assert provenance["activities"][0]["id"] == "derive-example-package"

    relationships = doc.relationships()
    assert relationships == [
        {
            "tableId": "chassis_brake_validation_specs",
            "columns": ["vehicle_variant"],
            "references": {
                "table": "vehicle_variant_configuration_specs",
                "columns": ["variant_id"],
            },
        }
    ]


def test_sql_query_api() -> None:
    doc = mcd.open(example("revenue-report"))

    result = doc.query(
        "select count(*) as rows, max(revenue_gbp) as max_revenue from revenue"
    )

    assert result.columns == ["rows", "max_revenue"]
    assert result.row_count == 1
    assert len(result) == 1
    assert result.rows == [{"rows": 4, "max_revenue": 158250.0}]
    assert result.as_dict()["rowCount"] == 1
    assert "max_revenue" in result.to_json()
    assert result.to_csv() == "rows,max_revenue\n4,158250\n"
    assert "max_revenue" in result.to_table()

    top_level = mcd.query(
        example("revenue-report"),
        "select quarter from revenue order by revenue_gbp desc limit 1",
    )
    assert top_level.rows == [{"quarter": "Q4"}]

    with pytest.raises(ValueError, match="query must be a SELECT statement"):
        doc.query("delete from revenue")


def test_annotation_metadata_access(tmp_path: Path) -> None:
    package = tmp_path / "annotated.mcd"
    with zipfile.ZipFile(package, "w") as archive:
        archive.writestr("mimetype", "application/vnd.mcd+zip")
        archive.writestr(
            "manifest.json",
            '{"format":"MCD","version":"0.1","profile":"MCD-Core","entrypoint":"content/main.md","annotations":[{"id":"review-intro","metadata":"annotations/review-intro.annotation.json"}]}',
        )
        archive.writestr("content/main.md", "# Annotated\n\nNeeds review.\n")
        archive.writestr(
            "annotations/review-intro.annotation.json",
            '{"id":"review-intro","target":{"type":"document"},"kind":"comment","status":"open","body":"Review the opening copy.","labels":["review"]}',
        )

    doc = mcd.open(package)
    annotations = doc.annotations()

    assert len(annotations) == 1
    assert annotations[0].id == "review-intro"
    assert annotations[0].kind == "comment"
    assert annotations[0].target()["type"] == "document"
    assert doc.annotation("review-intro").labels == ["review"]


def test_pdf_conversion(tmp_path: Path) -> None:
    pdf = tmp_path / "source.pdf"
    package = tmp_path / "source.mcd"
    pdf.write_bytes(minimal_pdf("Hello from PDF"))

    doc = mcd.convert_pdf(pdf, package, title="Imported PDF")

    assert doc.validate().valid
    assert "# Imported PDF" in doc.markdown()
    assert "Hello from PDF" in doc.markdown()
    converted_bytes = mcd.pdf_to_mcd_bytes(
        minimal_pdf("Bytes PDF"),
        title="Bytes Import",
        source_filename="bytes.pdf",
    )
    package_from_bytes = tmp_path / "bytes.mcd"
    package_from_bytes.write_bytes(converted_bytes)
    assert "Bytes PDF" in mcd.open(package_from_bytes).markdown()


def test_validation_failure_returns_diagnostic(tmp_path: Path) -> None:
    package = tmp_path / "missing-manifest.mcd"
    with zipfile.ZipFile(package, "w") as archive:
        archive.writestr("mimetype", "application/vnd.mcd+zip")
        archive.writestr("content/main.md", "# Missing manifest\n")

    result = mcd.open(package).validate()

    assert not result.valid
    assert result.diagnostics[0].level == "error"
    assert result.diagnostics[0].code == "manifest.missing"
    assert result.as_dict()["diagnostics"][0]["source"] == "manifest.json"


def test_open_exception_for_fatal_package_error(tmp_path: Path) -> None:
    package = tmp_path / "bad.mcd"
    with zipfile.ZipFile(package, "w") as archive:
        archive.writestr("mimetype", "text/plain")

    with pytest.raises(ValueError, match="package.mimetype.invalid"):
        mcd.open(package)


def test_pandas_dataframe_optional() -> None:
    pytest.importorskip("pandas")

    doc = mcd.open(example("revenue-report"))
    table = doc.table("revenue")
    frame = table.dataframe()

    assert list(frame.columns) == ["quarter", "revenue_gbp"]
    assert frame.iloc[0]["quarter"] == "Q1"

    chart_frame = doc.chart("revenue-chart").dataframe()
    assert list(chart_frame.columns) == ["quarter", "revenue_gbp"]
    assert chart_frame.iloc[0]["revenue_gbp"] == "125000"


def minimal_pdf(text: str) -> bytes:
    escaped = text.replace("\\", r"\\").replace("(", r"\(").replace(")", r"\)")
    content = f"BT /F1 24 Tf 100 700 Td ({escaped}) Tj ET"
    objects = [
        "<< /Type /Catalog /Pages 2 0 R >>",
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >>",
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>",
        f"<< /Length {len(content)} >>\nstream\n{content}\nendstream",
    ]
    pdf = "%PDF-1.4\n"
    offsets: list[int] = []
    for index, obj in enumerate(objects, start=1):
        offsets.append(len(pdf.encode()))
        pdf += f"{index} 0 obj\n{obj}\nendobj\n"
    xref_offset = len(pdf.encode())
    pdf += f"xref\n0 {len(objects) + 1}\n"
    pdf += "0000000000 65535 f \n"
    for offset in offsets:
        pdf += f"{offset:010} 00000 n \n"
    pdf += (
        f"trailer\n<< /Size {len(objects) + 1} /Root 1 0 R >>\n"
        f"startxref\n{xref_offset}\n%%EOF\n"
    )
    return pdf.encode()
