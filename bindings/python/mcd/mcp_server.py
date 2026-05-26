"""MCP server for Markdown CSV Document packages."""

from __future__ import annotations

import argparse
import json
from collections.abc import Callable
from pathlib import Path
from typing import Any, Literal

import mcd

QueryOutput = Literal["dict", "json", "csv", "table"]
QueryBatchOutput = Literal["dict", "json", "table"]


def _open_document(path: str | Path) -> mcd.Document:
    package_path = Path(path).expanduser()
    if not package_path.exists():
        raise FileNotFoundError(f"MCD package does not exist: {package_path}")
    if not package_path.is_file():
        raise ValueError(f"MCD path is not a file: {package_path}")
    return mcd.open(package_path)


def _slice_rows(rows: list[dict[str, Any]], max_rows: int | None) -> list[dict[str, Any]]:
    if max_rows is None:
        return rows
    if max_rows < 0:
        raise ValueError("max_rows must be non-negative or null")
    return rows[:max_rows]


def validate_package(path: str) -> dict[str, Any]:
    """Validate an MCD package and return diagnostics."""
    return _open_document(path).validate().as_dict()


def agent_context(
    path: str,
    include_tables: bool = False,
    include_layout: bool = False,
) -> dict[str, Any]:
    """Return a compact machine-readable overview of an MCD package."""
    return _open_document(path).to_agent_context(
        include_tables=include_tables,
        include_layout=include_layout,
    )


def markdown(path: str, expand_tables: bool = False) -> str:
    """Read package Markdown, optionally expanding table directives."""
    return _open_document(path).markdown(expand_tables=expand_tables)


def query(path: str, sql: str, output: QueryOutput = "dict") -> dict[str, Any] | str:
    """Run a read-only SQL query against package tables and metadata."""
    result = _open_document(path).query(sql)
    if output == "dict":
        return result.as_dict()
    if output == "json":
        return result.to_json()
    if output == "csv":
        return result.to_csv()
    if output == "table":
        return result.to_table()
    raise ValueError("output must be one of: dict, json, csv, table")


def queries(
    path: str,
    sql: list[str],
    output: QueryBatchOutput = "dict",
) -> dict[str, Any] | str:
    """Run multiple read-only SQL queries against one loaded package."""
    results = _open_document(path).queries(sql)
    payload = {
        "queryCount": len(results),
        "queries": [
            {"index": index, "sql": sql[index], "result": result.as_dict()}
            for index, result in enumerate(results)
        ],
    }
    if output == "dict":
        return payload
    if output == "json":
        return json.dumps(payload, indent=2)
    if output == "table":
        return "\n".join(
            f"-- query {index + 1}: {sql[index]}\n{result.to_table()}"
            for index, result in enumerate(results)
        )
    raise ValueError("output must be one of: dict, json, table")


def table(
    path: str,
    table_id: str,
    include_rows: bool = True,
    typed_rows: bool = False,
    max_rows: int | None = 100,
) -> dict[str, Any]:
    """Return table schema and, optionally, rows from an MCD package."""
    item = _open_document(path).table(table_id)
    data = item.as_dict()
    if include_rows:
        rows = item.typed_rows() if typed_rows else item.rows()
        data["rows"] = _slice_rows(rows, max_rows)
        data["rowCount"] = len(rows)
        if max_rows is not None:
            data["returnedRowCount"] = len(data["rows"])
    else:
        data.pop("rows", None)
        data["rowCount"] = len(item.rows())
    return data


def chart(
    path: str,
    chart_id: str,
    include_rows: bool = True,
    max_rows: int | None = 100,
) -> dict[str, Any]:
    """Return chart metadata and source rows from an MCD package."""
    item = _open_document(path).chart(chart_id)
    data = item.as_dict()
    if include_rows:
        rows = item.rows()
        data["rows"] = _slice_rows(rows, max_rows)
        data["rowCount"] = len(rows)
        if max_rows is not None:
            data["returnedRowCount"] = len(data["rows"])
    else:
        data.pop("rows", None)
        data["rowCount"] = len(item.rows())
    return data


def image(path: str, image_id: str) -> dict[str, Any]:
    """Return image metadata from an MCD package."""
    return _open_document(path).image(image_id).as_dict()


def annotations(
    path: str,
    annotation_id: str | None = None,
    page: str | None = None,
    line: int | None = None,
) -> dict[str, Any]:
    """Return annotation metadata, optionally filtered by id, page, or line."""
    doc = _open_document(path)
    if annotation_id:
        return {"annotations": [doc.annotation(annotation_id).as_dict()]}

    items = [item.as_dict() for item in doc.annotations()]
    if page is not None:
        items = [
            item
            for item in items
            if item.get("target", {}).get("path") == page
            or item.get("target", {}).get("page") == page
        ]
    if line is not None:
        items = [
            item
            for item in items
            if item.get("target", {}).get("line") == line
            or item.get("target", {}).get("startLine") == line
        ]
    return {"annotations": items, "count": len(items)}


def relationships(path: str) -> dict[str, Any]:
    """Return table relationship metadata declared by an MCD package."""
    items = _open_document(path).relationships()
    return {"relationships": items, "count": len(items)}


def external_data(path: str) -> dict[str, Any]:
    """Return manifest-declared external data references."""
    items = _open_document(path).external_data()
    return {"externalData": items, "count": len(items)}


def provenance(path: str) -> dict[str, Any]:
    """Return package provenance metadata, if present."""
    return {"provenance": _open_document(path).provenance()}


def convert_pdf(input: str, output: str, title: str | None = None) -> dict[str, Any]:
    """Convert a PDF file into an MCD package and return validation metadata."""
    input_path = Path(input).expanduser()
    output_path = Path(output).expanduser()
    if not input_path.exists():
        raise FileNotFoundError(f"PDF file does not exist: {input_path}")
    if not input_path.is_file():
        raise ValueError(f"PDF path is not a file: {input_path}")
    output_path.parent.mkdir(parents=True, exist_ok=True)
    doc = mcd.convert_pdf(input_path, output_path, title=title)
    validation = doc.validate().as_dict()
    return {
        "path": doc.path,
        "valid": validation["valid"],
        "diagnostics": validation["diagnostics"],
    }


def _register_tools(decorator: Callable[..., Any]) -> None:
    decorator(validate_package, name="mcd_validate")
    decorator(agent_context, name="mcd_agent_context")
    decorator(markdown, name="mcd_markdown")
    decorator(query, name="mcd_query")
    decorator(queries, name="mcd_queries")
    decorator(table, name="mcd_table")
    decorator(chart, name="mcd_chart")
    decorator(image, name="mcd_image")
    decorator(annotations, name="mcd_annotations")
    decorator(relationships, name="mcd_relationships")
    decorator(external_data, name="mcd_external_data")
    decorator(provenance, name="mcd_provenance")
    decorator(convert_pdf, name="mcd_convert_pdf")


def create_server(name: str = "MCD Tools") -> Any:
    """Create the FastMCP server used by the console entrypoint."""
    try:
        from mcp.server.fastmcp import FastMCP
    except ImportError as exc:
        raise RuntimeError(
            "The MCP SDK is not installed. Install it with 'pip install mcdee[mcp]'."
        ) from exc

    server = FastMCP(name)
    _register_tools(server.add_tool)
    return server


def main(argv: list[str] | None = None) -> None:
    """Run the MCD MCP server."""
    parser = argparse.ArgumentParser(description="Run the MCD MCP server.")
    parser.add_argument(
        "--transport",
        choices=["stdio", "streamable-http", "sse"],
        default="stdio",
        help="MCP transport to use. Defaults to stdio.",
    )
    parser.add_argument(
        "--name",
        default="MCD Tools",
        help="Server name shown to MCP clients.",
    )
    args = parser.parse_args(argv)
    create_server(args.name).run(transport=args.transport)


if __name__ == "__main__":
    main()
