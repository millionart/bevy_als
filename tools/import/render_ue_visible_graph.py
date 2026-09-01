import argparse
import html
import json
import math
import tempfile
from pathlib import Path


def read_json(path):
    return json.loads(path.read_text(encoding="utf-8"))


def property_value(node, name, default):
    for value in node.get("properties", []):
        if value.get("name") == name:
            try:
                return float(value.get("exported_text", default))
            except ValueError:
                return default
    return default


def node_size(node):
    if node["class_path"].endswith("EdGraphNode_Comment"):
        return (
            max(property_value(node, "NodeWidth", 320.0), 160.0),
            max(property_value(node, "NodeHeight", 120.0), 80.0),
        )
    pins = [pin for pin in node.get("pins", []) if not pin.get("hidden")]
    return (260.0, max(88.0, 44.0 + len(pins) * 18.0))


def color(node):
    class_path = node["class_path"]
    if class_path.endswith("EdGraphNode_Comment"):
        return "#8a8a8a"
    if "BlendSpace" in class_path:
        return "#287738"
    if "SequencePlayer" in class_path:
        return "#6d3434"
    if "StateMachine" in class_path:
        return "#313131"
    if "ModifyCurve" in class_path:
        return "#323232"
    if "Root" in class_path:
        return "#544a3f"
    return "#3b3b3b"


def visible_pins(node):
    return [pin for pin in node.get("pins", []) if not pin.get("hidden")]


def source_bounds(nodes):
    values = []
    for node in nodes:
        width, height = node_size(node)
        values.append(
            (
                float(node["position_x"]),
                float(node["position_y"]),
                float(node["position_x"]) + width,
                float(node["position_y"]) + height,
            )
        )
    return (
        min(value[0] for value in values),
        min(value[1] for value in values),
        max(value[2] for value in values),
        max(value[3] for value in values),
    )


def render(contract):
    nodes = contract["source_graph"]["nodes"]
    by_path = {node["object_path"]: node for node in nodes}
    min_x, min_y, max_x, max_y = source_bounds(nodes)
    margin = 64.0
    width = max_x - min_x + margin * 2.0
    height = max_y - min_y + margin * 2.0

    def x(value):
        return float(value) - min_x + margin

    def y(value):
        return float(value) - min_y + margin

    parts = [
        (
            f'<svg xmlns="http://www.w3.org/2000/svg" width="{math.ceil(width)}" '
            f'height="{math.ceil(height)}" viewBox="0 0 {width:.3f} {height:.3f}">'
        ),
        '<rect width="100%" height="100%" fill="#181818"/>',
        '<pattern id="grid" width="16" height="16" patternUnits="userSpaceOnUse"><path d="M 16 0 L 0 0 0 16" fill="none" stroke="#252525" stroke-width="1"/></pattern>',
        '<rect width="100%" height="100%" fill="url(#grid)"/>',
    ]
    for node in nodes:
        if not node["class_path"].endswith("EdGraphNode_Comment"):
            continue
        width_value, height_value = node_size(node)
        parts.extend(
            [
                (
                    f'<rect x="{x(node["position_x"]):.3f}" y="{y(node["position_y"]):.3f}" '
                    f'width="{width_value:.3f}" height="{height_value:.3f}" fill="#a0a0a0" '
                    'fill-opacity="0.24" stroke="#c0c0c0" stroke-width="2"/>'
                ),
                (
                    f'<text x="{x(node["position_x"]) + 12:.3f}" y="{y(node["position_y"]) + 24:.3f}" '
                    'font-family="Arial" font-size="18" font-weight="bold" fill="#efefef">'
                    f'{html.escape(node.get("comment") or node.get("title", ""))}</text>'
                ),
            ]
        )
    for link in contract["links"]:
        source = by_path.get(link["source"]["node_object_path"])
        target = by_path.get(link["target"]["node_object_path"])
        if source is None or target is None:
            continue
        source_width, source_height = node_size(source)
        target_width, target_height = node_size(target)
        start_x = x(source["position_x"]) + source_width
        start_y = y(source["position_y"]) + source_height * 0.5
        end_x = x(target["position_x"])
        end_y = y(target["position_y"]) + target_height * 0.5
        middle_x = (start_x + end_x) * 0.5
        parts.append(
            (
                f'<path d="M {start_x:.3f} {start_y:.3f} C {middle_x:.3f} {start_y:.3f}, '
                f'{middle_x:.3f} {end_y:.3f}, {end_x:.3f} {end_y:.3f}" '
                'fill="none" stroke="#d8d8d8" stroke-width="2"/>'
            )
        )
    for node in nodes:
        if node["class_path"].endswith("EdGraphNode_Comment"):
            continue
        width_value, height_value = node_size(node)
        left = x(node["position_x"])
        top = y(node["position_y"])
        parts.append(
            (
                f'<rect x="{left:.3f}" y="{top:.3f}" width="{width_value:.3f}" '
                f'height="{height_value:.3f}" rx="8" fill="{color(node)}" '
                'stroke="#111111" stroke-width="3"/>'
            )
        )
        titles = node.get("title", "").splitlines() or [""]
        for index, title in enumerate(titles):
            parts.append(
                (
                    f'<text x="{left + 12:.3f}" y="{top + 24 + index * 18:.3f}" '
                    'font-family="Arial" font-size="15" fill="#f4f4f4">'
                    f'{html.escape(title)}</text>'
                )
            )
        pins = visible_pins(node)
        for index, pin in enumerate(pins):
            pin_y = top + 48 + index * 18
            direction = pin.get("direction")
            pin_x = left + 9 if direction == "Input" else left + width_value - 9
            text_x = left + 18 if direction == "Input" else left + width_value - 18
            anchor = "start" if direction == "Input" else "end"
            parts.extend(
                [
                    f'<circle cx="{pin_x:.3f}" cy="{pin_y:.3f}" r="4" fill="#5bd45b"/>',
                    (
                        f'<text x="{text_x:.3f}" y="{pin_y + 5:.3f}" font-family="Arial" '
                        f'font-size="11" text-anchor="{anchor}" fill="#e5e5e5">'
                        f'{html.escape(pin.get("name", ""))}</text>'
                    ),
                ]
            )
    parts.append("</svg>")
    return "\n".join(parts) + "\n"


def run_self_test():
    contract = {
        "source_graph": {
            "nodes": [
                {
                    "object_path": "comment",
                    "class_path": "/Script/UnrealEd.EdGraphNode_Comment",
                    "position_x": 0,
                    "position_y": 0,
                    "comment": "Comment",
                    "properties": [],
                    "pins": [],
                },
                {
                    "object_path": "source",
                    "class_path": "/Script/AnimGraph.AnimGraphNode_Test",
                    "position_x": 10,
                    "position_y": 20,
                    "title": "Source",
                    "properties": [],
                    "pins": [{"name": "Pose", "direction": "Output", "hidden": False}],
                },
                {
                    "object_path": "target",
                    "class_path": "/Script/AnimGraph.AnimGraphNode_Test",
                    "position_x": 320,
                    "position_y": 20,
                    "title": "Target",
                    "properties": [],
                    "pins": [{"name": "Pose", "direction": "Input", "hidden": False}],
                },
            ]
        },
        "links": [
            {
                "source": {"node_object_path": "source"},
                "target": {"node_object_path": "target"},
            }
        ],
    }
    value = render(contract)
    assert "<svg" in value and "Comment" in value and "Source" in value
    with tempfile.TemporaryDirectory() as directory:
        path = Path(directory) / "source.svg"
        path.write_text(value, encoding="utf-8")
        assert path.read_text(encoding="utf-8") == value
    return {
        "node_pin_comment_rendering": "PASS",
        "link_rendering": "PASS",
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--contract", type=Path)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        print(json.dumps(run_self_test(), ensure_ascii=False, indent=2))
        return 0
    if args.contract is None or args.output is None:
        raise ValueError("contract and output are required")
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(render(read_json(args.contract)), encoding="utf-8")
    print(json.dumps({"output": args.output.as_posix()}, ensure_ascii=False))


if __name__ == "__main__":
    raise SystemExit(main())
