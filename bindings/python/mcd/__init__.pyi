from __future__ import annotations

from pathlib import Path
from typing import Any


class Diagnostic:
    level: str
    code: str
    message: str
    source: str | None
    related: list[str]

    def as_dict(self) -> dict[str, Any]: ...


class ValidationResult:
    valid: bool
    diagnostics: list[Diagnostic]

    def as_dict(self) -> dict[str, Any]: ...


class Block:
    id: str
    type: str
    source: dict[str, Any] | None

    def as_dict(self) -> dict[str, Any]: ...


class TableSchema:
    id: str
    columns: list[dict[str, Any]]

    def as_dict(self) -> dict[str, Any]: ...


class TableView:
    id: str
    table_id: str
    display: str
    columns: list[dict[str, Any]]
    chart: dict[str, Any] | None

    def layout(self) -> dict[str, Any] | None: ...
    def as_dict(self) -> dict[str, Any]: ...


class Table:
    id: str
    source: str
    schema: TableSchema

    def rows(self) -> list[dict[str, Any]]: ...
    def typed_rows(self) -> list[dict[str, Any]]: ...
    def dataframe(self) -> Any: ...
    def as_dict(self) -> dict[str, Any]: ...


class Chart:
    table_id: str
    view_id: str
    placement_ref: str | None
    view: TableView

    def rows(self) -> list[dict[str, Any]]: ...
    def dataframe(self) -> Any: ...
    def to_markdown_table(self) -> str: ...
    def layout(self) -> dict[str, Any] | None: ...
    def as_dict(self) -> dict[str, Any]: ...


class Image:
    id: str
    asset_path: str
    role: str
    alt: str | None
    caption: str | None
    intrinsic_size: dict[str, Any] | None

    def as_dict(self) -> dict[str, Any]: ...


class Annotation:
    id: str
    kind: str
    status: str
    body: str
    labels: list[str]

    def target(self) -> dict[str, Any]: ...
    def proposed_change(self) -> dict[str, Any] | None: ...
    def as_dict(self) -> dict[str, Any]: ...


class Document:
    path: str

    def validate(self) -> ValidationResult: ...
    def blocks(self) -> list[Block]: ...
    def table(self, id: str) -> Table: ...
    def chart(self, id: str) -> Chart: ...
    def image(self, id: str) -> Image: ...
    def annotation(self, id: str) -> Annotation: ...
    def annotations(self) -> list[Annotation]: ...
    def markdown(self, expand_tables: bool = False) -> str: ...
    def to_agent_context(
        self,
        include_tables: bool = True,
        include_layout: bool = False,
    ) -> dict[str, Any]: ...


def open(path: str | Path) -> Document: ...
