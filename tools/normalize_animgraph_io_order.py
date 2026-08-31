import argparse
import json
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


def compact(text):
    result = []
    quote = False
    index = 0
    while index < len(text):
        char = text[index]
        if quote:
            result.append(char)
            if char == "\\":
                if index + 1 < len(text):
                    result.append(text[index + 1])
                    index += 1
            elif char == '"':
                quote = False
        elif char == '"':
            quote = True
            result.append(char)
        elif not char.isspace():
            result.append(char)
        index += 1
    return "".join(result)


def field_span(text, name):
    match = re.search(rf"(?<![A-Za-z0-9_]){re.escape(name)}\s*:", text)
    if match is None:
        raise ValueError(f"missing field {name}")
    index = match.end()
    while index < len(text) and text[index].isspace():
        index += 1
    if index >= len(text) or text[index] not in "[{":
        raise ValueError(f"field {name} has no collection")
    opener = text[index]
    closer = "]" if opener == "[" else "}"
    return match.start(), index, matching_end(text, index, opener, closer)


def input_keys(text):
    _, times_start, times_end = field_span(text, "input_times")
    _, data_start, data_end = field_span(text, "input_data")
    time_values = split_top_level(text[times_start + 1 : times_end], ",")
    data_entries = split_top_level(text[data_start + 1 : data_end], ",")
    data_keys = [top_level_colon(entry)[0] for entry in data_entries]
    return [f"Time({value})" for value in time_values] + [
        f"Data({value})" for value in data_keys
    ]


def unique_keys(keys):
    result = []
    duplicates = []
    seen = set()
    for key in keys:
        normalized = compact(key)
        if normalized in seen:
            duplicates.append(key)
            continue
        seen.add(normalized)
        result.append(key)
    return result, duplicates


def parse_input_order(text):
    field_start, order_start, order_end = field_span(text, "input_order")
    entries = split_top_level(text[order_start + 1 : order_end], ",")
    parsed = []
    malformed = []
    for position, entry in enumerate(entries):
        try:
            key, value = top_level_colon(entry)
        except ValueError:
            malformed.append(entry)
            continue
        try:
            order = int(value)
        except ValueError:
            order = None
        parsed.append(
            {
                "key": key,
                "normalized": compact(key),
                "order": order,
                "position": position,
                "entry": entry,
            }
        )
    return field_start, order_start, order_end, parsed, malformed


def input_order_issues(text):
    expected, duplicate_declared_inputs = unique_keys(input_keys(text))
    _, _, _, parsed, malformed = parse_input_order(text)
    expected_normalized = {compact(key) for key in expected}
    observed = {}
    duplicate_order_keys = []
    invalid_order_indices = []
    for entry in parsed:
        if entry["normalized"] in observed:
            duplicate_order_keys.append(entry["key"])
        else:
            observed[entry["normalized"]] = entry
        if entry["order"] is None:
            invalid_order_indices.append(entry["entry"])
    missing = [key for key in expected if compact(key) not in observed]
    extra = [
        entry["key"] for entry in parsed if entry["normalized"] not in expected_normalized
    ]
    orders = [entry["order"] for entry in parsed if entry["order"] is not None]
    duplicate_order_indices = sorted(
        {
            order
            for order in orders
            if orders.count(order) > 1
        }
    )
    return {
        "duplicate_declared_inputs": duplicate_declared_inputs,
        "malformed_order_entries": malformed,
        "missing_order_entries": missing,
        "extra_order_entries": extra,
        "duplicate_order_keys": duplicate_order_keys,
        "invalid_order_indices": invalid_order_indices,
        "duplicate_order_indices": duplicate_order_indices,
    }


def has_issues(issues):
    return any(value for value in issues.values())


def normalize_input_order(text):
    issues = input_order_issues(text)
    if not has_issues(issues):
        return text, {}
    expected, duplicate_declared_inputs = unique_keys(input_keys(text))
    if duplicate_declared_inputs:
        return text, issues
    field_start, order_start, order_end, parsed, _ = parse_input_order(text)
    expected_by_normalized = {compact(key): key for key in expected}
    usable = []
    seen = set()
    for entry in parsed:
        normalized = entry["normalized"]
        if (
            normalized in expected_by_normalized
            and normalized not in seen
            and entry["order"] is not None
        ):
            usable.append(entry)
            seen.add(normalized)
    usable.sort(key=lambda entry: (entry["order"], entry["position"]))
    ordered = [expected_by_normalized[entry["normalized"]] for entry in usable]
    ordered.extend(key for key in expected if compact(key) not in seen)
    line_start = text.rfind("\n", 0, field_start) + 1
    base_indent = re.match(r"[ \t]*", text[line_start:]).group(0)
    inner_indent = f"{base_indent}    "
    newline = "\r\n" if "\r\n" in text else "\n"
    replacement = "{" + newline
    replacement += "".join(
        f"{inner_indent}{key}: {index},{newline}"
        for index, key in enumerate(ordered)
    )
    replacement += f"{base_indent}}}"
    return text[:order_start] + replacement + text[order_end + 1 :], issues


def process(path, write):
    with path.open("r", encoding="utf-8", newline="") as file:
        text = file.read()
    updated, repairs = normalize_input_order(text)
    if repairs and write and updated != text:
        with path.open("w", encoding="utf-8", newline="") as file:
            file.write(updated)
    return repairs


def run_self_test():
    fixture = (
        '(input_times: ["tick"], input_data: { "pose": Pose }, '
        'input_order: { Time("tick"): 4, Data("stale"): 0, },)'
    )
    normalized, repairs = normalize_input_order(fixture)
    assert repairs
    assert not has_issues(input_order_issues(normalized))
    assert 'Time("tick"): 0' in normalized
    assert 'Data("pose"): 1' in normalized
    return {"input_order_normalization": "PASS"}


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
    paths = sorted(
        list(args.root.rglob("*.animgraph.ron"))
        + list(args.root.rglob("*.fsm.ron"))
    )
    for path in paths:
        repairs = process(path, args.write)
        if repairs:
            changes[path.as_posix()] = repairs
    print(json.dumps({"write": args.write, "changes": changes}, indent=2))
    return 0 if args.write or not changes else 1


if __name__ == "__main__":
    raise SystemExit(main())
