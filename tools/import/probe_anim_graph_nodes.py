import argparse
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from uasset_base import ParseError, UAsset
from uasset_reader import PropReader


def export_class(asset, index):
    export = asset.exports[index]
    if export.class_index < 0:
        imported = asset.imports[-export.class_index - 1]
        return imported.object_name if imported.class_name == "Class" else imported.class_name
    if export.class_index > 0:
        parent = asset.exports[export.class_index - 1]
        if parent.class_index < 0:
            imported = asset.imports[-parent.class_index - 1]
            return imported.object_name if imported.class_name == "Class" else imported.class_name
        return parent.object_name
    return ""


def outer_path(asset, index):
    parts = []
    current = index + 1
    while current > 0:
        export = asset.exports[current - 1]
        parts.append(export.object_name)
        current = export.outer_index
    return "/".join(reversed(parts))


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("asset")
    parser.add_argument("--contains", default="")
    parser.add_argument("--outer", default="")
    parser.add_argument("--exports", action="store_true")
    args = parser.parse_args()
    asset = UAsset(args.asset)
    index = next(
        (
            index
            for index in range(len(asset.exports))
            if export_class(asset, index) == "AnimBlueprintGeneratedClass"
            and asset.exports[index].object_name.startswith("Default__")
        ),
        None,
    )
    if index is None:
        raise ParseError("AnimBlueprintGeneratedClass default object not found")
    properties, ok, error = PropReader(asset).read_export(index)
    if not ok:
        raise ParseError(error)
    if args.exports:
        for export_index, export in enumerate(asset.exports):
            class_name = export_class(asset, export_index)
            if not class_name.startswith(
                ("AnimGraphNode_", "AlsAnimGraphNode_", "AnimStateTransitionNode")
            ):
                continue
            if args.contains and args.contains not in class_name:
                continue
            path = outer_path(asset, export_index)
            if args.outer and args.outer not in path:
                continue
            export_properties, export_ok, export_error = PropReader(asset).read_export(export_index)
            if not export_ok:
                raise ParseError(export_error)
            print(
                "%s [%s]: %s"
                % (
                    path,
                    class_name,
                    json.dumps(export_properties, default=str),
                )
            )
        return
    for name in sorted(properties):
        if not name.startswith(("AnimGraphNode_", "AlsAnimGraphNode_")):
            continue
        if args.contains and args.contains not in name:
            continue
        print("%s: %s" % (name, json.dumps(properties[name], default=str)))


if __name__ == "__main__":
    main()
