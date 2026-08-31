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


def normalize_input_order(text):
    keys = input_keys(text)
    field_start, order_start, order_end = field_span(text, "input_order")
    entries = split_top_level(text[order_start + 1 : order_end], ",")
    existing = {}
    max_index = -1
    for entry in entries:
        key, value = top_level_colon(entry)
        existing[compact(key)] = entry
        try:
            max_index = max(max_index, int(value))
        except ValueError:
            pass
    additions = []
    next_index = max_index + 1
    for key in keys:
        if compact(key) not in existing:
            additions.append(f"{key}: {next_index}")
            next_index += 1
    if not additions:
        return text, []
    line_start = text.rfind("\n", 0, field_start) + 1
    base_indent = re.match(r"[ \t]*", text[line_start:]).group(0)
    inner_indent = f"{base_indent}    "
    newline = "\r\n" if "\r\n" in text else "\n"
    rendered = [entry.strip() for entry in entries] + additions
    replacement = "{" + newline
    replacement += "".join(f"{inner_indent}{entry},{newline}" for entry in rendered)
    replacement += f"{base_indent}}}"
    return text[:order_start] + replacement + text[order_end + 1 :], additions


def process(path, write):
    with path.open("r", encoding="utf-8", newline="") as file:
        text = file.read()
    updated, additions = normalize_input_order(text)
    if additions and write:
        with path.open("w", encoding="utf-8", newline="") as file:
            file.write(updated)
    return additions


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path("assets/graphs"))
    parser.add_argument("--write", action="store_true")
    args = parser.parse_args()
    changes = {}
    paths = sorted(
        list(args.root.rglob("*.animgraph.ron"))
        + list(args.root.rglob("*.fsm.ron"))
    )
    for path in paths:
        additions = process(path, args.write)
        if additions:
            changes[path.as_posix()] = additions
    print(json.dumps({"write": args.write, "changes": changes}, indent=2))
    return 0 if args.write or not changes else 1


if __name__ == "__main__":
    raise SystemExit(main())
