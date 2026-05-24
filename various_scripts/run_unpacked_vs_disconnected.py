#!/usr/bin/env python3
"""Compare one provider's QA accuracy on connected and disconnected unpacked datasets."""

from __future__ import annotations

import argparse
import csv
import json
import os
import sys
import time
from dataclasses import dataclass
from datetime import datetime
from pathlib import Path
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[2]
PLAIN_EVAL_DIR = REPO_ROOT / "tests" / "llm_plain_eval"
sys.path.insert(0, str(PLAIN_EVAL_DIR))

import run_plain_eval as plain_eval  # noqa: E402


DEFAULT_CONNECTED_DIR = Path("datasets/auto-manufacturer-tech-spec/unpacked")
DEFAULT_DISCONNECTED_DIR = Path("datasets/auto-manufacturer-tech-spec/unpacked_disconnected")
DEFAULT_QUESTIONS_PATH = Path("datasets/auto-manufacturer-tech-spec/qa_pilot_questions_20.jsonl")
DEFAULT_RESULTS_ROOT = Path("results/llm_unpacked_vs_disconnected")
DEFAULT_OPENAI_MODEL = "gpt-5.4"
DEFAULT_ANTHROPIC_MODEL = "claude-sonnet-4-5"
DEFAULT_XAI_MODEL = "grok-4.3"
DEFAULT_MAX_OUTPUT_TOKENS = 12000


@dataclass(frozen=True)
class DatasetTarget:
    label: str
    dir: Path
    use_manifest: bool


@dataclass(frozen=True)
class ProviderConfig:
    name: str
    model: str
    api_key_env: str


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--connected-dir", type=Path, default=DEFAULT_CONNECTED_DIR)
    parser.add_argument("--disconnected-dir", type=Path, default=DEFAULT_DISCONNECTED_DIR)
    parser.add_argument("--questions", type=Path, default=DEFAULT_QUESTIONS_PATH)
    parser.add_argument("--results-root", type=Path, default=DEFAULT_RESULTS_ROOT)
    parser.add_argument(
        "--provider",
        choices=["openai", "anthropic", "xai"],
        default="openai",
        help="Run exactly one provider for both datasets.",
    )
    parser.add_argument("--openai-model", default=os.getenv("OPENAI_MODEL", DEFAULT_OPENAI_MODEL))
    parser.add_argument(
        "--anthropic-model", default=os.getenv("ANTHROPIC_MODEL", DEFAULT_ANTHROPIC_MODEL)
    )
    parser.add_argument("--xai-model", default=os.getenv("XAI_MODEL", DEFAULT_XAI_MODEL))
    parser.add_argument("--limit", type=int, default=None, help="Run only the first N questions.")
    parser.add_argument(
        "--eval-mode",
        choices=["kb_agent", "plain_context"],
        default="kb_agent",
        help="Use the same modes as llm_plain_eval; kb_agent is the default.",
    )
    parser.add_argument("--max-tool-steps", type=int, default=8)
    parser.add_argument("--batch-size", type=int, default=1)
    parser.add_argument("--max-output-tokens", type=int, default=DEFAULT_MAX_OUTPUT_TOKENS)
    parser.add_argument("--temperature", type=float, default=0.0)
    parser.add_argument("--timeout-seconds", type=int, default=120)
    parser.add_argument("--retries", type=int, default=2)
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Write the output structure without calling provider APIs.",
    )
    return parser.parse_args()


def provider_config(args: argparse.Namespace) -> ProviderConfig:
    configs = {
        "openai": ProviderConfig("openai", args.openai_model, "OPENAI_API_KEY"),
        "anthropic": ProviderConfig("anthropic", args.anthropic_model, "ANTHROPIC_API_KEY"),
        "xai": ProviderConfig("xai", args.xai_model, "XAI_API_KEY"),
    }
    return configs[args.provider]


def make_output_dir(results_root: Path) -> Path:
    timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
    for suffix in ["", *[f"_{index:02d}" for index in range(1, 100)]]:
        output_dir = results_root / f"run_{timestamp}{suffix}"
        try:
            output_dir.mkdir(parents=True, exist_ok=False)
            return output_dir
        except FileExistsError:
            continue
    raise RuntimeError(f"Could not create a unique output directory under {results_root}")


def score_or_none(answer: str, question: dict[str, Any], error: str | None, dry_run: bool) -> dict[str, Any] | None:
    if dry_run or error:
        return None
    return plain_eval.score_answer(answer, question["expected_contains"])


def int_token(value: Any) -> int:
    try:
        if value is None:
            return 0
        return int(value)
    except (TypeError, ValueError):
        return 0


def token_usage_from_metadata(metadata: dict[str, Any]) -> dict[str, int]:
    usage = metadata.get("usage")
    if not isinstance(usage, dict):
        return {"input_tokens": 0, "output_tokens": 0, "total_tokens": 0}

    input_tokens = int_token(usage.get("input_tokens") or usage.get("prompt_tokens"))
    output_tokens = int_token(usage.get("output_tokens") or usage.get("completion_tokens"))

    # Anthropic reports cache tokens separately from input_tokens.
    input_tokens += int_token(usage.get("cache_creation_input_tokens"))
    input_tokens += int_token(usage.get("cache_read_input_tokens"))

    total_tokens = int_token(usage.get("total_tokens"))
    if not total_tokens:
        total_tokens = input_tokens + output_tokens

    return {
        "input_tokens": input_tokens,
        "output_tokens": output_tokens,
        "total_tokens": total_tokens,
    }


def add_token_usage(left: dict[str, int], right: dict[str, int]) -> dict[str, int]:
    return {
        "input_tokens": left.get("input_tokens", 0) + right.get("input_tokens", 0),
        "output_tokens": left.get("output_tokens", 0) + right.get("output_tokens", 0),
        "total_tokens": left.get("total_tokens", 0) + right.get("total_tokens", 0),
    }


def token_usage_from_rows(rows: list[dict[str, Any]]) -> dict[str, int]:
    total = {"input_tokens": 0, "output_tokens": 0, "total_tokens": 0}
    for row in rows:
        metadata = row.get("metadata", {})
        if not isinstance(metadata, dict):
            continue
        token_usage = metadata.get("token_usage")
        if not isinstance(token_usage, dict):
            continue
        total = add_token_usage(total, {key: int_token(value) for key, value in token_usage.items()})
    return total


def format_tokens(value: int) -> str:
    return f"{value:,}"


def elapsed_seconds_from_rows(rows: list[dict[str, Any]]) -> float:
    return sum(float(row.get("elapsed_seconds") or 0.0) for row in rows)


def format_seconds(value: float) -> str:
    return f"{value:.1f} sec"


def filesystem_file_index(dataset_dir: Path) -> list[str]:
    return [
        path.relative_to(dataset_dir).as_posix()
        for path in sorted(dataset_dir.rglob("*"))
        if path.is_file() and path.name != "manifest.json"
    ]


def filesystem_dataset_summary(dataset_dir: Path) -> str:
    entrypoint = "content/main.md"
    return json.dumps(
        {
            "title": dataset_dir.name,
            "entrypoint": entrypoint if (dataset_dir / entrypoint).exists() else None,
            "files": filesystem_file_index(dataset_dir),
            "note": "No manifest or table registry is provided for this dataset. Inspect files directly.",
        },
        ensure_ascii=False,
        indent=2,
    )


def csv_table_paths(dataset_dir: Path) -> dict[str, Path]:
    return {
        path.stem: path
        for path in sorted((dataset_dir / "tables").glob("*.csv"))
        if path.is_file()
    }


def load_csv_rows(dataset_dir: Path, table_id: str) -> list[dict[str, str]]:
    paths = csv_table_paths(dataset_dir)
    if table_id not in paths:
        raise ValueError(f"Unknown table: {table_id}")
    with paths[table_id].open("r", encoding="utf-8", newline="") as handle:
        return list(csv.DictReader(handle))


def load_dataset_rows(target: DatasetTarget, table_id: str) -> list[dict[str, str]]:
    if target.use_manifest:
        return plain_eval.load_table_rows(target.dir, table_id)
    return load_csv_rows(target.dir, table_id)


def normalize_op(op: str) -> str:
    aliases = {
        "=": "eq",
        "==": "eq",
        "!=": "ne",
        "<>": "ne",
        ">": "gt",
        ">=": "gte",
        "ge": "gte",
        "<": "lt",
        "<=": "lte",
        "le": "lte",
    }
    return aliases.get(op.casefold(), op.casefold())


def derived_value(row: dict[str, Any], spec: Any) -> Any:
    if isinstance(spec, str):
        return plain_eval.row_value(row, spec)
    if not isinstance(spec, dict):
        return spec

    op = str(spec.get("op", "column"))
    if op == "column":
        return plain_eval.row_value(row, str(spec["column"]))
    if op == "literal":
        return spec.get("value")
    if op == "prefix_before":
        value = str(derived_value(row, spec["value"]))
        delimiter = str(spec.get("delimiter", "-"))
        return value.split(delimiter, 1)[0]
    if op == "lower":
        return str(derived_value(row, spec["value"])).casefold()
    if op == "add":
        return sum(float(derived_value(row, item)) for item in spec.get("values", []))
    if op == "subtract":
        values = [float(derived_value(row, item)) for item in spec.get("values", [])]
        if not values:
            return 0
        result = values[0]
        for value in values[1:]:
            result -= value
        return result
    raise ValueError(f"Unsupported derived value op: {op}")


def compare_values(actual: Any, op: str, expected: Any) -> bool:
    op = normalize_op(op)
    if op == "in":
        values = expected if isinstance(expected, list) else [expected]
        return any(compare_values(actual, "eq", value) for value in values)
    if op == "not_in":
        values = expected if isinstance(expected, list) else [expected]
        return all(not compare_values(actual, "eq", value) for value in values)

    actual_number = plain_eval.to_number(actual)
    expected_number = plain_eval.to_number(expected)
    if actual_number is not None and expected_number is not None:
        if op == "eq":
            return actual_number == expected_number
        if op == "ne":
            return actual_number != expected_number
        if op == "gt":
            return actual_number > expected_number
        if op == "gte":
            return actual_number >= expected_number
        if op == "lt":
            return actual_number < expected_number
        if op == "lte":
            return actual_number <= expected_number

    actual_text = str(actual).casefold()
    expected_text = str(expected).casefold()
    if op == "eq":
        return actual_text == expected_text
    if op == "ne":
        return actual_text != expected_text
    if op == "contains":
        return expected_text in actual_text
    if op == "startswith":
        return actual_text.startswith(expected_text)
    if op == "endswith":
        return actual_text.endswith(expected_text)
    raise ValueError(f"Unsupported filter op: {op}")


def row_matches_filter(row: dict[str, Any], item: dict[str, Any]) -> bool:
    if "any" in item:
        return any(row_matches_filter(row, child) for child in item["any"])
    if "all" in item:
        return all(row_matches_filter(row, child) for child in item["all"])

    left_spec = item.get("left", item.get("column"))
    actual = derived_value(row, left_spec)
    if "right" in item:
        expected = derived_value(row, item["right"])
    else:
        expected = item.get("value")
    return compare_values(actual, str(item.get("op", "eq")), expected)


def apply_filters(rows: list[dict[str, Any]], filters: list[dict[str, Any]] | None) -> list[dict[str, Any]]:
    if not filters:
        return rows
    filtered = rows
    for item in filters:
        filtered = [row for row in filtered if row_matches_filter(row, item)]
    return filtered


def sort_rows(rows: list[dict[str, Any]], sort_by: Any, sort_desc: bool) -> list[dict[str, Any]]:
    if not sort_by:
        return rows

    def key(row: dict[str, Any]) -> tuple[int, Any]:
        value = derived_value(row, sort_by)
        number = plain_eval.to_number(value)
        if number is not None:
            return (0, number)
        return (1, str(value).casefold())

    return sorted(rows, key=key, reverse=sort_desc)


def alias_row(row: dict[str, Any], alias: str) -> dict[str, Any]:
    aliased = dict(row)
    aliased.update({f"{alias}.{key}": value for key, value in row.items()})
    return aliased


def apply_derived_columns(rows: list[dict[str, Any]], derive: dict[str, Any] | None) -> list[dict[str, Any]]:
    if not derive:
        return rows
    derived_rows: list[dict[str, Any]] = []
    for row in rows:
        item = dict(row)
        for name, spec in derive.items():
            item[name] = derived_value(item, spec)
        derived_rows.append(item)
    return derived_rows


def project_query_rows(rows: list[dict[str, Any]], columns: list[Any] | None) -> list[dict[str, Any]]:
    if not columns:
        return rows

    projected: list[dict[str, Any]] = []
    for row in rows:
        item: dict[str, Any] = {}
        for column in columns:
            if isinstance(column, str):
                item[column] = plain_eval.row_value(row, column)
            elif isinstance(column, dict):
                name = str(column["name"])
                item[name] = derived_value(row, column.get("value", name))
            else:
                raise ValueError(f"Unsupported column projection: {column}")
        projected.append(item)
    return projected


def grouped_query_rows(rows: list[dict[str, Any]], group_by: list[Any]) -> list[dict[str, Any]]:
    groups: dict[tuple[Any, ...], dict[str, Any]] = {}
    for row in rows:
        key_values = tuple(
            derived_value(group_row, group.get("value", group["name"]) if isinstance(group, dict) else group)
            for group in group_by
            for group_row in [row]
        )
        if key_values not in groups:
            item: dict[str, Any] = {"count": 0}
            for index, group in enumerate(group_by):
                if isinstance(group, dict):
                    name = str(group["name"])
                else:
                    name = str(group).split(".")[-1]
                item[name] = key_values[index]
            groups[key_values] = item
        groups[key_values]["count"] += 1
    return list(groups.values())


def filesystem_table_summary(dataset_dir: Path) -> list[dict[str, Any]]:
    summaries: list[dict[str, Any]] = []
    for table_id, path in csv_table_paths(dataset_dir).items():
        with path.open("r", encoding="utf-8", newline="") as handle:
            reader = csv.DictReader(handle)
            rows = list(reader)
        summaries.append(
            {
                "table": table_id,
                "path": path.relative_to(dataset_dir).as_posix(),
                "row_count": len(rows),
                "columns": reader.fieldnames or [],
            }
        )
    return summaries


def execute_filesystem_tool(dataset_dir: Path, tool: str, args: dict[str, Any]) -> dict[str, Any]:
    if tool == "list_files":
        return {"files": filesystem_file_index(dataset_dir)}

    if tool == "read_text":
        path = plain_eval.safe_dataset_path(dataset_dir, str(args["path"]))
        max_chars = min(int(args.get("max_chars", 12000)), 40000)
        return {
            "path": path.relative_to(dataset_dir).as_posix(),
            "text": path.read_text(encoding="utf-8")[:max_chars],
        }

    if tool == "search_text":
        query = str(args["query"]).casefold()
        paths = [str(args["path"])] if args.get("path") else filesystem_file_index(dataset_dir)
        limit = min(int(args.get("limit", 20)), 100)
        matches: list[dict[str, Any]] = []
        for relative in paths:
            path = plain_eval.safe_dataset_path(dataset_dir, relative)
            for line_number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), start=1):
                if query in line.casefold():
                    matches.append({"path": relative, "line": line_number, "text": line[:1000]})
                    if len(matches) >= limit:
                        return {"matches": matches}
        return {"matches": matches}

    if tool == "table_info":
        table_id = str(args["table"])
        for item in filesystem_table_summary(dataset_dir):
            if item["table"] == table_id:
                sample_rows = load_csv_rows(dataset_dir, table_id)[: min(int(args.get("sample", 3)), 10)]
                return {**item, "sample_rows": sample_rows}
        raise ValueError(f"Unknown table: {table_id}")

    if tool == "table_select":
        rows = load_csv_rows(dataset_dir, str(args["table"]))
        rows = apply_filters(rows, args.get("filters"))
        rows = sort_rows(rows, args.get("sort_by"), bool(args.get("sort_desc", False)))
        limit = min(int(args.get("limit", 20)), 200)
        return {
            "total_matches": len(rows),
            "rows": plain_eval.project_rows(rows[:limit], args.get("columns")),
        }

    if tool == "table_join":
        left_table = str(args["left_table"])
        right_table = str(args["right_table"])
        left_key = str(args["left_key"])
        right_key = str(args["right_key"])
        left_rows = load_csv_rows(dataset_dir, left_table)
        right_rows = load_csv_rows(dataset_dir, right_table)
        right_index: dict[str, list[dict[str, str]]] = {}
        for right in right_rows:
            right_index.setdefault(right[right_key], []).append(right)

        joined: list[dict[str, Any]] = []
        for left in left_rows:
            for right in right_index.get(left[left_key], []):
                row = {f"left.{key}": value for key, value in left.items()}
                row.update({f"right.{key}": value for key, value in right.items()})
                joined.append(row)

        joined = apply_filters(joined, args.get("filters"))
        joined = sort_rows(joined, args.get("sort_by"), bool(args.get("sort_desc", False)))
        limit = min(int(args.get("limit", 20)), 200)
        return {
            "total_matches": len(joined),
            "rows": plain_eval.project_rows(joined[:limit], args.get("columns")),
        }

    raise ValueError(f"Unknown tool: {tool}")


def execute_table_select(target: DatasetTarget, args: dict[str, Any]) -> dict[str, Any]:
    rows = load_dataset_rows(target, str(args["table"]))
    rows = apply_filters(rows, args.get("filters"))
    rows = sort_rows(rows, args.get("sort_by"), bool(args.get("sort_desc", False)))
    limit = min(int(args.get("limit", 20)), 200)
    return {
        "total_matches": len(rows),
        "rows": plain_eval.project_rows(rows[:limit], args.get("columns")),
    }


def execute_table_join(target: DatasetTarget, args: dict[str, Any]) -> dict[str, Any]:
    left_table = str(args["left_table"])
    right_table = str(args["right_table"])
    left_key = str(args["left_key"])
    right_key = str(args["right_key"])
    left_rows = load_dataset_rows(target, left_table)
    right_rows = load_dataset_rows(target, right_table)
    right_index: dict[str, list[dict[str, str]]] = {}
    for right in right_rows:
        right_index.setdefault(right[right_key], []).append(right)

    joined: list[dict[str, Any]] = []
    for left in left_rows:
        for right in right_index.get(left[left_key], []):
            row = {f"left.{key}": value for key, value in left.items()}
            row.update({f"right.{key}": value for key, value in right.items()})
            joined.append(row)

    joined = apply_filters(joined, args.get("filters"))
    joined = sort_rows(joined, args.get("sort_by"), bool(args.get("sort_desc", False)))
    limit = min(int(args.get("limit", 20)), 200)
    return {
        "total_matches": len(joined),
        "rows": plain_eval.project_rows(joined[:limit], args.get("columns")),
    }


def execute_table_group_count(target: DatasetTarget, args: dict[str, Any]) -> dict[str, Any]:
    table_id = str(args["table"])
    group_by = str(args["group_by"])
    rows = load_dataset_rows(target, table_id)
    rows = apply_filters(rows, args.get("filters"))

    counts: dict[str, int] = {}
    for row in rows:
        value = str(plain_eval.row_value(row, group_by))
        counts[value] = counts.get(value, 0) + 1

    sort_desc = bool(args.get("sort_desc", True))
    limit = min(int(args.get("limit", 20)), 200)
    groups = [
        {
            group_by: value,
            "count": count,
        }
        for value, count in sorted(counts.items(), key=lambda item: (item[1], item[0]), reverse=sort_desc)
    ]
    return {
        "table": table_id,
        "group_by": group_by,
        "total_rows": len(rows),
        "groups": groups[:limit],
    }


def execute_table_count(target: DatasetTarget, args: dict[str, Any]) -> dict[str, Any]:
    rows = load_dataset_rows(target, str(args["table"]))
    rows = apply_filters(rows, args.get("filters"))
    return {
        "table": str(args["table"]),
        "count": len(rows),
    }


def execute_table_validate_rule(target: DatasetTarget, args: dict[str, Any]) -> dict[str, Any]:
    rows = load_dataset_rows(target, str(args["table"]))
    pass_filter = args.get("pass_filter")
    if not isinstance(pass_filter, dict):
        raise ValueError("table_validate_rule requires pass_filter object.")

    invalid_rows = [row for row in rows if not row_matches_filter(row, pass_filter)]
    first_columns = args.get("columns")
    if not first_columns:
        first_columns = list(invalid_rows[0].keys())[:5] if invalid_rows else []
    first_invalid = (
        {column: plain_eval.row_value(invalid_rows[0], column) for column in first_columns}
        if invalid_rows
        else None
    )
    return {
        "table": str(args["table"]),
        "total_rows": len(rows),
        "valid_count": len(rows) - len(invalid_rows),
        "invalid_count": len(invalid_rows),
        "first_invalid": first_invalid,
    }


def execute_table_query(target: DatasetTarget, args: dict[str, Any]) -> dict[str, Any]:
    table_id = str(args.get("from") or args["table"])
    base_alias = str(args.get("alias") or table_id)
    rows: list[dict[str, Any]] = [
        alias_row(row, base_alias) for row in load_dataset_rows(target, table_id)
    ]

    for join in args.get("joins", []):
        right_table = str(join["table"])
        right_alias = str(join.get("alias") or right_table)
        right_rows = [alias_row(row, right_alias) for row in load_dataset_rows(target, right_table)]
        joined: list[dict[str, Any]] = []
        for left in rows:
            left_value = derived_value(left, join["left"])
            for right in right_rows:
                if str(left_value) == str(derived_value(right, join["right"])):
                    combined = dict(left)
                    combined.update(right)
                    joined.append(combined)
        rows = joined

    rows = apply_filters(rows, args.get("filters"))
    rows = apply_derived_columns(rows, args.get("derive"))

    if args.get("group_by"):
        rows = grouped_query_rows(rows, args["group_by"])

    total_matches = len(rows)
    rows = sort_rows(rows, args.get("sort_by"), bool(args.get("sort_desc", False)))
    limit = min(int(args.get("limit", 20)), 200)
    selected = rows[:limit]

    return {
        "table": table_id,
        "total_matches": total_matches,
        "returned": len(selected),
        "rows": project_query_rows(selected, args.get("columns")),
    }


def execute_dataset_tool(target: DatasetTarget, tool: str, args: dict[str, Any]) -> dict[str, Any]:
    if tool == "table_query":
        return execute_table_query(target, args)
    if tool == "table_select":
        return execute_table_select(target, args)
    if tool == "table_join":
        return execute_table_join(target, args)
    if tool == "table_count":
        return execute_table_count(target, args)
    if tool == "table_validate_rule":
        return execute_table_validate_rule(target, args)
    if tool == "table_group_count":
        return execute_table_group_count(target, args)
    if target.use_manifest:
        return plain_eval.execute_kb_tool(target.dir, tool, args)
    return execute_filesystem_tool(target.dir, tool, args)


def dataset_summary(target: DatasetTarget) -> str:
    if target.use_manifest:
        return plain_eval.kb_dataset_summary(target.dir)
    return filesystem_dataset_summary(target.dir)


def extract_first_json_object(text: str) -> dict[str, Any]:
    stripped = text.strip()
    if stripped.startswith("```"):
        stripped = stripped.strip("`").strip()
        if stripped.casefold().startswith("json"):
            stripped = stripped[4:].strip()

    start = stripped.find("{")
    if start == -1:
        raise ValueError("Provider response did not contain a JSON object.")

    decoder = json.JSONDecoder()
    value, _end = decoder.raw_decode(stripped[start:])
    if not isinstance(value, dict):
        raise ValueError("Provider response JSON must be an object.")
    return value


def parse_agent_action(text: str) -> dict[str, Any]:
    action = extract_first_json_object(text)
    if "answer" in action:
        return {"answer": str(action["answer"])}
    if "tool" in action:
        args = action.get("args", {})
        if not isinstance(args, dict):
            raise ValueError("Tool action 'args' must be an object.")
        return {"tool": str(action["tool"]), "args": args}
    raise ValueError("Agent response must contain either 'tool' or 'answer'.")


def make_kb_agent_prompt(
    dataset_summary_text: str,
    question: dict[str, Any],
    observations: list[dict[str, Any]],
) -> str:
    tool_docs = {
        "list_files": {},
        "read_text": {"path": "content/main.md", "max_chars": 12000},
        "search_text": {"query": "CHS-00982", "path": "tables/chassis_brake_validation_specs.csv", "limit": 10},
        "table_info": {"table": "chassis_brake_validation_specs", "sample": 3},
        "table_select": {
            "table": "chassis_brake_validation_specs",
            "columns": ["test_id", "vehicle_variant", "stop_distance_100_0_m"],
            "filters": [{"column": "stop_distance_100_0_m", "op": "<", "value": 35}],
            "sort_by": "stop_distance_100_0_m",
            "sort_desc": False,
            "limit": 5,
        },
        "table_join": {
            "left_table": "chassis_brake_validation_specs",
            "right_table": "vehicle_variant_configuration_specs",
            "left_key": "vehicle_variant",
            "right_key": "variant_id",
            "columns": ["left.test_id", "left.vehicle_variant", "left.stop_distance_100_0_m", "right.body_style"],
            "filters": [{"column": "right.trim_level", "op": "in", "value": ["Sport", "Performance"]}],
            "sort_by": "left.stop_distance_100_0_m",
            "sort_desc": False,
            "limit": 5,
        },
        "table_count": {
            "table": "battery_pack_module_specs",
            "filters": [{"column": "coolant_flow_l_min", "op": ">", "value": 15}],
        },
        "table_group_count": {
            "table": "production_quality_measurements",
            "group_by": "plant_code",
            "filters": [],
            "sort_desc": True,
            "limit": 3,
        },
        "table_validate_rule": {
            "table": "vehicle_variant_configuration_specs",
            "pass_filter": {
                "left": {"op": "prefix_before", "value": {"op": "column", "column": "homologation_code"}, "delimiter": "-"},
                "op": "eq",
                "right": {"op": "column", "column": "region"},
            },
            "columns": ["variant_id", "region", "homologation_code"],
        },
        "table_query": {
            "from": "chassis_brake_validation_specs",
            "alias": "c",
            "joins": [
                {
                    "table": "vehicle_variant_configuration_specs",
                    "alias": "v",
                    "left": "c.vehicle_variant",
                    "right": "v.variant_id",
                }
            ],
            "filters": [{"column": "v.trim_level", "op": "in", "value": ["Executive", "Premium"]}],
            "derive": {
                "loaded_mass": {
                    "op": "add",
                    "values": [
                        {"op": "column", "column": "v.curb_mass_kg"},
                        {"op": "column", "column": "v.max_payload_kg"},
                    ],
                }
            },
            "sort_by": "loaded_mass",
            "sort_desc": True,
            "columns": ["c.test_id", "c.vehicle_variant", "loaded_mass"],
            "limit": 1,
        },
        "table_query_group_example": {
            "tool": "table_query",
            "args": {
                "from": "production_quality_measurements",
                "group_by": ["plant_code"],
                "sort_by": "count",
                "sort_desc": True,
                "limit": 3,
            },
        },
        "computed_sort_example": {
            "tool": "table_join",
            "args": {
                "left_table": "chassis_brake_validation_specs",
                "right_table": "vehicle_variant_configuration_specs",
                "left_key": "vehicle_variant",
                "right_key": "variant_id",
                "sort_by": {
                    "op": "add",
                    "values": [
                        {"op": "column", "column": "right.curb_mass_kg"},
                        {"op": "column", "column": "right.max_payload_kg"},
                    ],
                },
                "sort_desc": True,
                "limit": 1,
            },
        },
    }
    return (
        "You are a knowledge-base assistant with access to the unpacked dataset through tools. "
        "Use the tools to inspect files and query CSV tables. Do not guess from memory. "
        "Prefer table_query when one question needs several table operations at once, such as joins plus filters, "
        "derived values, grouping, sorting, counts, or projection. table_query is a JSON table query tool, not SQL. "
        "For counts, use table_count or table_group_count instead of requesting all rows. "
        "For row-validation rules, use table_validate_rule when the rule can be expressed as a filter. "
        "table_select and table_join support filters with eq, ne, >, >=, <, <=, in, not_in, contains, "
        "startswith, and endswith. Filter left/right values and sort_by may be derived expressions such as "
        "prefix_before or add. "
        "If a tool result says total_matches is greater than the returned row count, treat returned rows as a "
        "capped sample. Do not compute final counts or extremes from capped samples unless the tool sorted exactly "
        "by the required criterion. "
        "For multi-table questions or unfamiliar columns, inspect table_info for the involved tables before using "
        "the columns. "
        "In the final answer, include the key condition values and field names used to select the result, not only "
        "the numeric answer. "
        "Return exactly one JSON object and no prose. "
        "Do not return a tool call and an answer in the same response.\n\n"
        "If you need data, return: "
        '{"tool":"tool_name","args":{...}}\n'
        "When you know the answer, return: "
        '{"answer":"concise answer containing exact IDs, field values, and numbers"}\n\n'
        "Available tools and argument examples:\n"
        f"{json.dumps(tool_docs, ensure_ascii=False, indent=2)}\n\n"
        "Dataset index:\n"
        f"{dataset_summary_text}\n\n"
        "Question:\n"
        f"{json.dumps({'id': question['id'], 'question': question['prompt']}, ensure_ascii=False, indent=2)}\n\n"
        "Previous tool observations:\n"
        f"{json.dumps(observations, ensure_ascii=False, indent=2)}"
    )


def run_kb_agent_question(
    *,
    target: DatasetTarget,
    dataset_summary_text: str,
    provider: str,
    model: str,
    question: dict[str, Any],
    max_output_tokens: int,
    temperature: float,
    timeout_seconds: int,
    retries: int,
    max_tool_steps: int,
    dry_run: bool,
) -> tuple[str, dict[str, Any], list[dict[str, Any]], str | None]:
    if dry_run:
        return (
            "",
            {
                "dry_run": True,
                "token_usage": {"input_tokens": 0, "output_tokens": 0, "total_tokens": 0},
            },
            [],
            None,
        )

    observations: list[dict[str, Any]] = []
    trace: list[dict[str, Any]] = []
    metadata: dict[str, Any] = {}
    call_usages: list[dict[str, int]] = []
    total_usage = {"input_tokens": 0, "output_tokens": 0, "total_tokens": 0}
    for step in range(1, max_tool_steps + 1):
        prompt = make_kb_agent_prompt(dataset_summary_text, question, observations)
        raw, metadata = plain_eval.call_with_retries(
            provider,
            prompt,
            model,
            max_output_tokens,
            temperature,
            timeout_seconds,
            retries,
        )
        token_usage = token_usage_from_metadata(metadata)
        call_usages.append(token_usage)
        total_usage = add_token_usage(total_usage, token_usage)
        metadata = {**metadata, "token_usage": total_usage, "call_token_usage": call_usages}
        try:
            action = parse_agent_action(raw)
        except Exception as exc:  # noqa: BLE001
            trace.append({"step": step, "raw": raw, "error": str(exc)})
            return "", metadata, trace, f"Could not parse agent action: {exc}"

        trace_item: dict[str, Any] = {"step": step, "raw": raw, "action": action}
        if "answer" in action:
            trace.append(trace_item)
            return action["answer"], metadata, trace, None

        try:
            observation = execute_dataset_tool(target, action["tool"], action["args"])
        except Exception as exc:  # noqa: BLE001
            observation = {"error": str(exc)}
        trace_item["observation"] = observation
        trace.append(trace_item)
        observations.append(
            {
                "step": step,
                "tool": action["tool"],
                "args": action["args"],
                "observation": observation,
            }
        )

    return "", metadata, trace, f"Agent did not answer within {max_tool_steps} tool steps."


def build_filesystem_context(dataset_dir: Path) -> tuple[str, list[str]]:
    paths = [dataset_dir / "content" / "main.md", *sorted((dataset_dir / "tables").glob("*.csv"))]
    parts: list[str] = []
    included_paths: list[str] = []
    for path in paths:
        if not path.exists() or not path.is_file():
            continue
        relative = path.relative_to(dataset_dir)
        included_paths.append(relative.as_posix())
        parts.append(f"## File: {relative.as_posix()}\n\n{path.read_text(encoding='utf-8')}")
    return "\n\n---\n\n".join(parts), included_paths


def build_dataset_context(target: DatasetTarget) -> tuple[str, list[str]]:
    if target.use_manifest:
        return plain_eval.build_context(target.dir)
    return build_filesystem_context(target.dir)


def run_agent_dataset(
    *,
    target: DatasetTarget,
    questions: list[dict[str, Any]],
    config: ProviderConfig,
    args: argparse.Namespace,
) -> list[dict[str, Any]]:
    dataset_summary_text = dataset_summary(target)
    rows: list[dict[str, Any]] = []
    for index, question in enumerate(questions, start=1):
        started = time.perf_counter()
        answer, metadata, trace, error = run_kb_agent_question(
            target=target,
            dataset_summary_text=dataset_summary_text,
            provider=config.name,
            model=config.model,
            question=question,
            max_output_tokens=args.max_output_tokens,
            temperature=args.temperature,
            timeout_seconds=args.timeout_seconds,
            retries=args.retries,
            max_tool_steps=args.max_tool_steps,
            dry_run=args.dry_run,
        )
        score = score_or_none(answer, question, error, args.dry_run)
        row = {
            "dataset": target.label,
            "dataset_dir": str(target.dir),
            "provider": config.name,
            "model": config.model,
            "question_index": index,
            "question_id": question["id"],
            "family_id": question.get("family_id"),
            "question": question["prompt"],
            "expected_contains": question["expected_contains"],
            "answer": answer,
            "score": score,
            "error": error,
            "metadata": metadata,
            "trace": trace,
            "elapsed_seconds": round(time.perf_counter() - started, 3),
        }
        rows.append(row)
        print(f"{target.label} {index}/{len(questions)} {question['id']}: {status_label(row)}", flush=True)
    return rows


def run_plain_context_dataset(
    *,
    target: DatasetTarget,
    questions: list[dict[str, Any]],
    config: ProviderConfig,
    args: argparse.Namespace,
) -> list[dict[str, Any]]:
    context, _included_paths = build_dataset_context(target)
    rows: list[dict[str, Any]] = []
    for batch_index, question_batch in enumerate(plain_eval.batches(questions, args.batch_size), start=1):
        started = time.perf_counter()
        prompt = plain_eval.make_eval_prompt(context, question_batch)
        if args.dry_run:
            raw_answer = json.dumps({"answers": []}, ensure_ascii=False)
            metadata = {
                "dry_run": True,
                "token_usage": {"input_tokens": 0, "output_tokens": 0, "total_tokens": 0},
            }
            provider_error = None
            answer_map: dict[str, str] = {}
        else:
            try:
                raw_answer, metadata = plain_eval.call_with_retries(
                    config.name,
                    prompt,
                    config.model,
                    args.max_output_tokens,
                    args.temperature,
                    args.timeout_seconds,
                    args.retries,
                )
                metadata = {**metadata, "token_usage": token_usage_from_metadata(metadata)}
                provider_error = None
            except Exception as exc:  # noqa: BLE001
                raw_answer = ""
                metadata = {"token_usage": {"input_tokens": 0, "output_tokens": 0, "total_tokens": 0}}
                provider_error = str(exc)

            if provider_error:
                answer_map = {}
            else:
                try:
                    answer_map = plain_eval.parse_answer_map(raw_answer)
                except Exception as exc:  # noqa: BLE001
                    answer_map = {}
                    provider_error = f"Could not parse provider JSON response: {exc}"

        elapsed_seconds = round(time.perf_counter() - started, 3)
        for offset, question in enumerate(question_batch):
            index = questions.index(question) + 1
            answer = answer_map.get(question["id"], "")
            error = provider_error
            if not error and not answer and not args.dry_run:
                error = "Provider response did not include an answer for this question."
            score = score_or_none(answer, question, error, args.dry_run)
            row = {
                "dataset": target.label,
                "dataset_dir": str(target.dir),
                "provider": config.name,
                "model": config.model,
                "question_index": index,
                "question_id": question["id"],
                "family_id": question.get("family_id"),
                "question": question["prompt"],
                "expected_contains": question["expected_contains"],
                "answer": answer,
                "score": score,
                "error": error,
                "metadata": {
                    **metadata,
                    "batch_index": batch_index,
                    "token_usage": (
                        metadata.get("token_usage", {"input_tokens": 0, "output_tokens": 0, "total_tokens": 0})
                        if offset == 0
                        else {"input_tokens": 0, "output_tokens": 0, "total_tokens": 0}
                    ),
                    "batch_token_usage_counted": offset == 0,
                },
                "raw_answer": raw_answer,
                "elapsed_seconds": elapsed_seconds,
            }
            rows.append(row)
            print(f"{target.label} {index}/{len(questions)} {question['id']}: {status_label(row)}", flush=True)
    return rows


def run_dataset(
    *,
    target: DatasetTarget,
    questions: list[dict[str, Any]],
    config: ProviderConfig,
    args: argparse.Namespace,
) -> list[dict[str, Any]]:
    if args.eval_mode == "plain_context":
        return run_plain_context_dataset(target=target, questions=questions, config=config, args=args)
    return run_agent_dataset(target=target, questions=questions, config=config, args=args)


def passed(row: dict[str, Any] | None) -> bool:
    return bool(row and row.get("score") and row["score"].get("passed"))


def status_label(row: dict[str, Any]) -> str:
    if row.get("error"):
        return "ERROR"
    if row.get("score") is None:
        return "DRY" if row.get("metadata", {}).get("dry_run") else "UNSCORED"
    return "PASS" if passed(row) else "FAIL"


def symbol(row: dict[str, Any] | None) -> str:
    return "&#10003;" if passed(row) else "&#10007;"


def row_seconds(row: dict[str, Any] | None) -> str:
    if not row:
        return "n/a"
    return format_seconds(float(row.get("elapsed_seconds") or 0.0))


def write_comparison_markdown(
    *,
    path: Path,
    questions: list[dict[str, Any]],
    connected_rows: list[dict[str, Any]],
    disconnected_rows: list[dict[str, Any]],
    config: ProviderConfig,
    args: argparse.Namespace,
    created_at: str,
) -> None:
    connected_by_id = {row["question_id"]: row for row in connected_rows}
    disconnected_by_id = {row["question_id"]: row for row in disconnected_rows}
    connected_passed = sum(1 for row in connected_rows if passed(row))
    disconnected_passed = sum(1 for row in disconnected_rows if passed(row))
    connected_tokens = token_usage_from_rows(connected_rows)
    disconnected_tokens = token_usage_from_rows(disconnected_rows)
    combined_tokens = add_token_usage(connected_tokens, disconnected_tokens)
    connected_elapsed = elapsed_seconds_from_rows(connected_rows)
    disconnected_elapsed = elapsed_seconds_from_rows(disconnected_rows)
    combined_elapsed = connected_elapsed + disconnected_elapsed
    connected_avg = connected_elapsed / len(connected_rows) if connected_rows else 0.0
    disconnected_avg = disconnected_elapsed / len(disconnected_rows) if disconnected_rows else 0.0
    combined_avg = combined_elapsed / (len(connected_rows) + len(disconnected_rows)) if connected_rows or disconnected_rows else 0.0

    lines = [
        "# LLM Unpacked vs Disconnected Comparison",
        "",
        f"- Created at: `{created_at}`",
        f"- Provider: `{config.name}`",
        f"- Model: `{config.model}`",
        f"- Eval mode: `{args.eval_mode}`",
        f"- Questions: `{args.questions}`",
        f"- Connected dataset: `{args.connected_dir}`",
        f"- Disconnected dataset: `{args.disconnected_dir}`",
        "",
        f"Pass totals: connected `{connected_passed}/{len(questions)}`, disconnected `{disconnected_passed}/{len(questions)}`.",
        "",
        "| Token usage | Input | Output | Total |",
        "| --- | ---: | ---: | ---: |",
        (
            f"| Connected data | {format_tokens(connected_tokens['input_tokens'])} | "
            f"{format_tokens(connected_tokens['output_tokens'])} | "
            f"{format_tokens(connected_tokens['total_tokens'])} |"
        ),
        (
            f"| Disconnected data | {format_tokens(disconnected_tokens['input_tokens'])} | "
            f"{format_tokens(disconnected_tokens['output_tokens'])} | "
            f"{format_tokens(disconnected_tokens['total_tokens'])} |"
        ),
        (
            f"| Combined | {format_tokens(combined_tokens['input_tokens'])} | "
            f"{format_tokens(combined_tokens['output_tokens'])} | "
            f"{format_tokens(combined_tokens['total_tokens'])} |"
        ),
        "",
        "| Timing | Total | Avg per answer |",
        "| --- | ---: | ---: |",
        f"| Connected data | {format_seconds(connected_elapsed)} | {format_seconds(connected_avg)} |",
        f"| Disconnected data | {format_seconds(disconnected_elapsed)} | {format_seconds(disconnected_avg)} |",
        f"| Combined | {format_seconds(combined_elapsed)} | {format_seconds(combined_avg)} |",
        "",
        "| # | Connected data | Connected time | Disconnected data | Disconnected time |",
        "| ---: | :---: | ---: | :---: | ---: |",
    ]
    for index, question in enumerate(questions, start=1):
        connected_row = connected_by_id.get(question["id"])
        disconnected_row = disconnected_by_id.get(question["id"])
        lines.append(
            f"| {index} | {symbol(connected_row)} | {row_seconds(connected_row)} | "
            f"{symbol(disconnected_row)} | {row_seconds(disconnected_row)} |"
        )
    lines.append("")
    path.write_text("\n".join(lines), encoding="utf-8")


def write_run_config(
    *,
    path: Path,
    args: argparse.Namespace,
    config: ProviderConfig,
    question_count: int,
    created_at: str,
) -> None:
    plain_eval.write_json(
        path,
        {
            "created_at": created_at,
            "provider": config.__dict__,
            "connected_dir": str(args.connected_dir),
            "disconnected_dir": str(args.disconnected_dir),
            "questions": str(args.questions),
            "question_count": question_count,
            "eval_mode": args.eval_mode,
            "max_tool_steps": args.max_tool_steps,
            "batch_size": args.batch_size,
            "max_output_tokens": args.max_output_tokens,
            "temperature": args.temperature,
            "timeout_seconds": args.timeout_seconds,
            "retries": args.retries,
            "dry_run": args.dry_run,
        },
    )


def main() -> int:
    plain_eval.load_dotenv()
    args = parse_args()
    config = provider_config(args)
    if not args.dry_run:
        plain_eval.validate_provider_env(
            [plain_eval.ProviderConfig(config.name, config.model, config.api_key_env)]
        )

    args.connected_dir = args.connected_dir.resolve()
    args.disconnected_dir = args.disconnected_dir.resolve()
    args.questions = args.questions.resolve()

    questions = plain_eval.read_jsonl(args.questions)
    if args.limit is not None:
        questions = questions[: args.limit]

    output_dir = make_output_dir(args.results_root)
    created_at = datetime.now().isoformat(timespec="seconds")
    write_run_config(
        path=output_dir / "run_config.json",
        args=args,
        config=config,
        question_count=len(questions),
        created_at=created_at,
    )

    connected_target = DatasetTarget("connected", args.connected_dir, use_manifest=True)
    disconnected_target = DatasetTarget("disconnected", args.disconnected_dir, use_manifest=False)
    connected_rows = run_dataset(target=connected_target, questions=questions, config=config, args=args)
    disconnected_rows = run_dataset(target=disconnected_target, questions=questions, config=config, args=args)

    plain_eval.write_jsonl(output_dir / "connected_results.jsonl", connected_rows)
    plain_eval.write_jsonl(output_dir / "disconnected_results.jsonl", disconnected_rows)
    plain_eval.write_jsonl(output_dir / "all_results.jsonl", [*connected_rows, *disconnected_rows])
    write_comparison_markdown(
        path=output_dir / "comparison.md",
        questions=questions,
        connected_rows=connected_rows,
        disconnected_rows=disconnected_rows,
        config=config,
        args=args,
        created_at=created_at,
    )

    print(f"Results written to {output_dir}")
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except Exception as exc:  # noqa: BLE001
        print(f"ERROR: {exc}", file=sys.stderr)
        sys.exit(1)
