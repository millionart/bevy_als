import argparse
import hashlib
import json
import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from normalize_fsm_editor_layout import (
    field_span,
    parse_position,
    split_top_level,
    top_level_colon,
)


def collection_text(text, name, offset=0):
    field_start, start, end = field_span(text, name, offset)
    return field_start, start, end, text[start : end + 1]


def entries(text, name, offset=0):
    field_start, start, end, value = collection_text(text, name, offset)
    return field_start, start, end, split_top_level(value[1:-1], ",")


def quoted_field(entry, name):
    match = re.search(rf'\b{re.escape(name)}:\s*"([^"]+)"', entry)
    return match.group(1) if match is not None else None


def raw_fields(text, names, offset=0):
    result = {}
    for name in names:
        try:
            _, _, _, value = collection_text(text, name, offset)
        except ValueError:
            result[name] = None
        else:
            result[name] = value
    return result


def parse_positions(text, field_name):
    metadata_field_start, metadata_start, metadata_end, _ = collection_text(text, "editor_metadata")
    field_start, start, end, value = collection_text(text, field_name, metadata_field_start)
    if field_start > metadata_end:
        raise ValueError(f"{field_name} is outside editor metadata")
    parsed = []
    malformed = []
    for index, entry in enumerate(split_top_level(value[1:-1], ",")):
        try:
            key, position_value = top_level_colon(entry)
        except ValueError:
            malformed.append(entry)
            continue
        match = re.fullmatch(r'"([^"]+)"', key.strip())
        if match is None:
            malformed.append(entry)
            continue
        parsed.append(
            {
                "id": match.group(1),
                "position": parse_position(position_value),
                "raw_value": position_value,
                "index": index,
                "entry": entry,
            }
        )
    return {"entries": parsed, "malformed": malformed}


def capture_animgraph(text):
    _, _, _, node_entries = entries(text, "nodes")
    nodes = []
    for index, entry in enumerate(node_entries):
        nodes.append(
            {
                "id": quoted_field(entry, "id"),
                "name": quoted_field(entry, "name"),
                "entry": entry,
                "index": index,
            }
        )
    return {
        "kind": "animgraph",
        "nodes": nodes,
        "node_positions": parse_positions(text, "node_positions"),
        "raw_fields": raw_fields(
            text,
            ["edges_inverted", "io_spec", "default_data", "editor_metadata"],
        ),
    }


def capture_fsm(text):
    _, _, _, state_entries = entries(text, "states")
    _, _, _, transition_entries = entries(text, "transitions")
    states = []
    for index, entry in enumerate(state_entries):
        states.append(
            {
                "id": quoted_field(entry, "id"),
                "label": quoted_field(entry, "label"),
                "graph": quoted_field(entry, "graph"),
                "entry": entry,
                "index": index,
            }
        )
    transitions = []
    for index, entry in enumerate(transition_entries):
        transitions.append(
            {
                "id": quoted_field(entry, "id"),
                "source": quoted_field(entry, "source"),
                "target": quoted_field(entry, "target"),
                "entry": entry,
                "index": index,
            }
        )
    return {
        "kind": "fsm",
        "states": states,
        "transitions": transitions,
        "state_positions": parse_positions(text, "states"),
        "raw_fields": raw_fields(
            text,
            ["node_spec", "default_data", "editor_metadata", "start_state"],
        ),
    }


def capture_file(path, root):
    text = path.read_text(encoding="utf-8")
    payload = {
        "path": path.relative_to(root).as_posix(),
        "sha256": hashlib.sha256(text.encode("utf-8")).hexdigest(),
        "raw_text": text,
    }
    try:
        payload["topology"] = capture_fsm(text) if path.name.endswith(".fsm.ron") else capture_animgraph(text)
    except ValueError as error:
        payload["parse_error"] = str(error)
    return payload


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", required=True, type=Path)
    parser.add_argument("--include", action="append", default=[])
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    roots = [args.root / value for value in args.include] if args.include else [args.root]
    paths = []
    for root in roots:
        if root.is_file():
            paths.append(root)
        elif root.is_dir():
            paths.extend(root.rglob("*.animgraph.ron"))
            paths.extend(root.rglob("*.fsm.ron"))
        else:
            raise ValueError(f"included path does not exist: {root}")
    paths = sorted(set(paths))
    files = [capture_file(path, args.root) for path in paths]
    payload = {
        "format": "als_animgraph_ron_topology_v1",
        "files": files,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(payload, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(
        json.dumps(
            {
                "output": args.output.as_posix(),
                "files": len(files),
                "parse_errors": sum("parse_error" in item for item in files),
            },
            ensure_ascii=False,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
