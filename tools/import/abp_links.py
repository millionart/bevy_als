import argparse
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from uasset_base import ParseError, UAsset
from uasset_reader import PropReader


def export_class(asset, index):
    export = asset.exports[index]
    if export.class_index < 0:
        imported = asset.imports[-export.class_index - 1]
        if imported.class_name == "Class":
            return imported.object_name
        return imported.class_name
    if export.class_index > 0:
        class_export = asset.exports[export.class_index - 1]
        if class_export.class_index < 0:
            imported = asset.imports[-class_export.class_index - 1]
            if imported.class_name == "Class":
                return imported.object_name
            return imported.class_name
        return class_export.object_name
    return ""


def walk_links(value, path):
    if isinstance(value, dict):
        if "LinkID" in value:
            print(
                "%s link=%s source=%s"
                % (path, value.get("LinkID"), value.get("SourceLinkID"))
            )
            return
        for key, child in value.items():
            walk_links(child, "%s/%s" % (path, key))
    elif isinstance(value, list):
        for index, child in enumerate(value):
            walk_links(child, "%s/%d" % (path, index))


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("asset")
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
    for name, value in properties.items():
        if name.startswith(("AnimGraphNode_", "AlsAnimGraphNode_")):
            print("NODE %s" % name)
            walk_links(value, name)


if __name__ == "__main__":
    main()
