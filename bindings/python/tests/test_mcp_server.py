from __future__ import annotations

from pathlib import Path

import pytest
import mcd.mcp_server as mcp_server


ROOT = Path(__file__).resolve().parents[3]


def example(name: str) -> Path:
    return ROOT / "examples" / name / f"{name}.mcd"


def test_mcp_helpers_validate_context_query_and_table() -> None:
    path = str(example("revenue-report"))

    validation = mcp_server.validate_package(path)
    assert validation == {"valid": True, "diagnostics": []}

    context = mcp_server.agent_context(path)
    assert context["sourcePath"] == "content/main.md"
    assert "tables" not in context

    result = mcp_server.query(
        path,
        "select quarter, revenue_gbp from revenue order by revenue_gbp desc limit 1",
    )
    assert result["rows"] == [{"quarter": "Q4", "revenue_gbp": 158250.0}]

    batch = mcp_server.queries(
        path,
        [
            "select count(*) as rows from revenue",
            "select quarter from revenue order by revenue_gbp desc limit 1",
        ],
    )
    assert batch["queryCount"] == 2
    assert batch["queries"][0]["result"]["rows"] == [{"rows": 4}]
    assert batch["queries"][1]["result"]["rows"] == [{"quarter": "Q4"}]

    table = mcp_server.table(path, "revenue", max_rows=2)
    assert table["id"] == "revenue"
    assert table["rowCount"] == 4
    assert table["returnedRowCount"] == 2
    assert table["rows"][0] == {"quarter": "Q1", "revenue_gbp": "125000"}


def test_mcp_helpers_metadata_shortcuts() -> None:
    path = str(example("auto-manufacturer-tech-spec"))

    relationships = mcp_server.relationships(path)
    assert relationships["count"] == 1
    assert relationships["relationships"][0]["tableId"] == "chassis_brake_validation_specs"

    external_data = mcp_server.external_data(path)
    assert external_data["count"] == 1
    assert external_data["externalData"][0]["id"] == "raw-auto-spec-source"

    provenance = mcp_server.provenance(path)
    assert provenance["provenance"]["activities"][0]["id"] == "derive-example-package"


def test_mcp_server_registers_tool_names() -> None:
    pytest.importorskip("mcp.server.fastmcp")

    server = mcp_server.create_server("test")
    tool_manager = getattr(server, "_tool_manager")
    tools = getattr(tool_manager, "_tools")

    assert sorted(tools) == [
        "mcd_agent_context",
        "mcd_annotations",
        "mcd_chart",
        "mcd_convert_pdf",
        "mcd_external_data",
        "mcd_image",
        "mcd_markdown",
        "mcd_provenance",
        "mcd_queries",
        "mcd_query",
        "mcd_relationships",
        "mcd_table",
        "mcd_validate",
    ]
