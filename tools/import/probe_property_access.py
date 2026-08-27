import argparse
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from uasset_base import UAsset


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


def printable_strings(data):
    return [value.decode("ascii") for value in re.findall(rb"[ -~]{3,}", data)]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("asset")
    args = parser.parse_args()
    asset = UAsset(args.asset)
    for index, export in enumerate(asset.exports):
        if export_class(asset, index) != "K2Node_PropertyAccess":
            continue
        start = export.serial_offset
        end = start + export.serial_size
        strings = printable_strings(asset.buf[start:end])
        print("%s: %s" % (outer_path(asset, index), " | ".join(strings)))


if __name__ == "__main__":
    main()
