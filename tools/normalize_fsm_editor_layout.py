import argparse
import json
import math
import re
from pathlib import Path


def matching_end(text, start, opener, closer):
    depth = 0
    quote = False
    index = start
    while index < len(text):
        char = text[index]
        if quote:
            if char == "\\":
                index += 2
                continue
            if char == '"':
                quote = False
        elif char == '"':
            quote = True
        elif char == opener:
            depth += 1
        elif char == closer:
            depth -= 1
            if depth == 0:
                return index
        index += 1
    raise ValueError(f"unclosed {opener} at {start}")


def split_top_level(text, delimiter):
    values = []
    start = 0
    parens = 0
    brackets = 0
    braces = 0
    quote = False
    index = 0
    while index < len(text):
        char = text[index]
        if quote:
            if char == "\\":
                index += 2
                continue
            if char == '"':
                quote = False
        elif char == '"':
            quote = True
        elif char == "(":
            parens += 1
        elif char == ")":
            parens -= 1
        elif char == "[":
            brackets += 1
        elif char == "]":
            brackets -= 1
        elif char == "{":
            braces += 1
        elif char == "}":
            braces -= 1
        elif char == delimiter and parens == 0 and brackets == 0 and braces == 0:
            values.append(text[start:index].strip())
            start = index + 1
        index += 1
    tail = text[start:].strip()
    if tail:
        values.append(tail)
    return [value for value in values if value]


def top_level_colon(text):
    values = split_top_level(text, ":")
    if len(values) < 2:
        raise ValueError(f"entry has no top-level colon: {text}")
    return values[0], ":".join(values[1:]).strip()


def field_span(text, name, offset=0):
    match = re.search(rf"(?<![A-Za-z0-9_]){re.escape(name)}\s*:", text[offset:])
    if match is None:
        raise ValueError(f"missing field {name}")
    field_start = offset + match.start()
    index = offset + match.end()
    while index < len(text) and text[index].isspace():
        index += 1
    if index >= len(text) or text[index] not in "[{(":
        raise ValueError(f"field {name} has no collection")
    opener = text[index]
    closer = {"[": "]", "{": "}", "(": ")"}[opener]
    return field_start, index, matching_end(text, index, opener, closer)


def state_ids(text):
    _, start, end = field_span(text, "states")
    values = split_top_level(text[start + 1 : end], ",")
    result = []
    for value in values:
        match = re.search(r'\bid:\s*"([^"]+)"', value)
        if match is None:
            raise ValueError("state entry has no id")
        result.append(match.group(1))
    return result


def parse_position(value):
    stripped = value.strip()
    if not stripped.startswith("(") or not stripped.endswith(")"):
        return None
    values = split_top_level(stripped[1:-1], ",")
    if len(values) != 2:
        return None
    try:
        position = (float(values[0]), float(values[1]))
    except ValueError:
        return None
    return position if all(math.isfinite(component) for component in position) else None


def parse_state_positions(text):
    metadata_start, _, _ = field_span(text, "editor_metadata")
    field_start, positions_start, positions_end = field_span(
        text, "states", metadata_start
    )
    entries = split_top_level(text[positions_start + 1 : positions_end], ",")
    parsed = []
    malformed = []
    for index, entry in enumerate(entries):
        try:
            key, value = top_level_colon(entry)
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
                "value": value,
                "position": parse_position(value),
                "index": index,
                "entry": entry,
            }
        )
    return field_start, positions_start, positions_end, parsed, malformed


def layout_issues(text):
    ids = state_ids(text)
    _, _, _, parsed, malformed = parse_state_positions(text)
    known_ids = set(ids)
    duplicate_state_ids = sorted({state_id for state_id in ids if ids.count(state_id) > 1})
    metadata = {}
    duplicate_metadata_ids = []
    invalid_positions = []
    for entry in parsed:
        state_id = entry["id"]
        if state_id in metadata:
            duplicate_metadata_ids.append(state_id)
        else:
            metadata[state_id] = entry
        if entry["position"] is None:
            invalid_positions.append(state_id)
    missing_positions = [state_id for state_id in ids if state_id not in metadata]
    extra_positions = sorted(state_id for state_id in metadata if state_id not in known_ids)
    locations = {}
    for entry in parsed:
        if entry["id"] in known_ids and entry["position"] is not None:
            locations.setdefault(entry["position"], []).append(entry["id"])
    overlapping_positions = [
        ids_at_position
        for _, ids_at_position in sorted(locations.items())
        if len(ids_at_position) > 1
    ]
    return {
        "duplicate_state_ids": duplicate_state_ids,
        "malformed_position_entries": malformed,
        "missing_positions": missing_positions,
        "extra_positions": extra_positions,
        "duplicate_metadata_ids": duplicate_metadata_ids,
        "invalid_positions": invalid_positions,
        "overlapping_positions": overlapping_positions,
    }


def has_issues(issues):
    return any(value for value in issues.values())


def next_position(slot, used):
    while True:
        column = slot % 4
        row = slot // 4
        position = (160.0 + column * 240.0, 140.0 + row * 160.0)
        slot += 1
        if position not in used:
            return position, slot


def normalize_state_positions(text):
    issues = layout_issues(text)
    if not has_issues(issues):
        return text, {}
    ids = state_ids(text)
    if issues["duplicate_state_ids"]:
        return text, issues
    field_start, positions_start, positions_end, parsed, _ = parse_state_positions(text)
    known_ids = set(ids)
    selected = {}
    used = set()
    for entry in parsed:
        state_id = entry["id"]
        position = entry["position"]
        if (
            state_id in known_ids
            and state_id not in selected
            and position is not None
            and position not in used
        ):
            selected[state_id] = entry["value"].strip()
            used.add(position)
    slot = 0
    rendered = []
    for state_id in ids:
        value = selected.get(state_id)
        if value is None:
            position, slot = next_position(slot, used)
            used.add(position)
            value = f"({position[0]:.1f}, {position[1]:.1f})"
        rendered.append((state_id, value))
    line_start = text.rfind("\n", 0, field_start) + 1
    base_indent = re.match(r"[ \t]*", text[line_start:]).group(0)
    inner_indent = f"{base_indent}    "
    newline = "\r\n" if "\r\n" in text else "\n"
    replacement = "{" + newline
    replacement += "".join(
        f'{inner_indent}"{state_id}": {value},{newline}'
        for state_id, value in rendered
    )
    replacement += f"{base_indent}}}"
    return text[:positions_start] + replacement + text[positions_end + 1 :], issues


def fsm_topology(text):
    ids = state_ids(text)
    known_ids = set(ids)
    _, start, end = field_span(text, "transitions")
    values = split_top_level(text[start + 1 : end], ",")
    malformed = []
    unknown_sources = []
    unknown_targets = []
    self_transitions = []
    for index, value in enumerate(values):
        source = re.search(r'\bsource:\s*"([^"]+)"', value)
        target = re.search(r'\btarget:\s*"([^"]+)"', value)
        transition_id = re.search(r'\bid:\s*"([^"]+)"', value)
        label = transition_id.group(1) if transition_id is not None else str(index)
        if source is None or target is None:
            malformed.append(label)
            continue
        source_id = source.group(1)
        target_id = target.group(1)
        if source_id not in known_ids:
            unknown_sources.append({"transition": label, "state": source_id})
        if target_id not in known_ids:
            unknown_targets.append({"transition": label, "state": target_id})
        if source_id == target_id:
            self_transitions.append(
                {"transition": label, "state": source_id}
            )
    return {
        "malformed_transitions": malformed,
        "unknown_transition_sources": unknown_sources,
        "unknown_transition_targets": unknown_targets,
        "self_transitions": self_transitions,
    }


def process(path, write):
    with path.open("r", encoding="utf-8", newline="") as file:
        text = file.read()
    updated, repairs = normalize_state_positions(text)
    if repairs and write and updated != text:
        with path.open("w", encoding="utf-8", newline="") as file:
            file.write(updated)
    return repairs


def run_self_test():
    fixture = (
        '(states: [(id: "a"), (id: "b")], '
        'transitions: [(id: "loop", source: "a", target: "a")], '
        'editor_metadata: (states: { "a": (160.0, 140.0), "b": (160.0, 140.0) }))'
    )
    normalized, repairs = normalize_state_positions(fixture)
    assert repairs
    assert not has_issues(layout_issues(normalized))
    topology = fsm_topology(normalized)
    assert topology["self_transitions"] == [{"transition": "loop", "state": "a"}]
    return {"fsm_layout_normalization": "PASS"}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path("assets/graphs"))
    parser.add_argument("--write", action="store_true")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        print(json.dumps(run_self_test(), indent=2))
        return 0
    changes = {}
    for path in sorted(args.root.rglob("*.fsm.ron")):
        repairs = process(path, args.write)
        if repairs:
            changes[path.as_posix()] = repairs
    print(json.dumps({"write": args.write, "changes": changes}, indent=2))
    return 0 if args.write or not changes else 1


if __name__ == "__main__":
    raise SystemExit(main())
