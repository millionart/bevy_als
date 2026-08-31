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


def state_positions(text, ids):
    metadata_start, _, _ = field_span(text, "editor_metadata")
    field_start, positions_start, positions_end = field_span(text, "states", metadata_start)
    entries = split_top_level(text[positions_start + 1 : positions_end], ",")
    existing = {}
    for entry in entries:
        match = re.match(r'"([^"]+)"\s*:', entry)
        if match is not None:
            existing[match.group(1)] = entry
    additions = []
    for index, state_id in enumerate(ids):
        if state_id not in existing:
            column = index % 4
            row = index // 4
            additions.append(f'"{state_id}": ({160.0 + column * 240.0:.1f}, {140.0 + row * 160.0:.1f})')
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
    return text[:positions_start] + replacement + text[positions_end + 1 :], additions


def process(path, write):
    with path.open("r", encoding="utf-8", newline="") as file:
        text = file.read()
    updated, additions = state_positions(text, state_ids(text))
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
    for path in sorted(args.root.rglob("*.fsm.ron")):
        additions = process(path, args.write)
        if additions:
            changes[path.as_posix()] = additions
    print(json.dumps({"write": args.write, "changes": changes}, indent=2))
    return 0 if args.write or not changes else 1


if __name__ == "__main__":
    raise SystemExit(main())
