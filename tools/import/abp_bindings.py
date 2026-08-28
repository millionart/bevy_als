import argparse
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from uasset_base import ParseError, UAsset, read_type_nodes
from uasset_reader import PropReader, _R


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


def outer_path(asset, index):
    parts = []
    current = index + 1
    while current > 0:
        export = asset.exports[current - 1]
        parts.append(export.object_name)
        current = export.outer_index
    return "/".join(reversed(parts))


def property_bindings(asset, index):
    export = asset.exports[index]
    end = export.serial_offset + export.serial_size
    reader_impl = PropReader(asset)
    for shift in range(4):
        reader = _R(asset, export.serial_offset + shift, end)
        try:
            name = reader.fname()
            nodes = read_type_nodes(reader)
            size = reader.i32()
            flags = reader.u8()
            reader_impl._skip_extra(reader, flags)
        except (IndexError, ParseError):
            continue
        if name != "PropertyBindings" or nodes[0][0] != "MapProperty":
            continue
        payload_end = reader.pos + size
        reader.i32()
        count = reader.i32()
        bindings = []
        for _ in range(count):
            pin_name = reader.fname()
            value = reader_impl._props(
                reader,
                payload_end,
                {"PropertyName", "PropertyPath"},
                0,
            )
            bindings.append((pin_name, value.get("PropertyPath", [])))
        return bindings
    return []


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("asset")
    args = parser.parse_args()
    asset = UAsset(args.asset)
    for index in range(len(asset.exports)):
        if export_class(asset, index) != "AnimGraphNodeBinding_Base":
            continue
        bindings = property_bindings(asset, index)
        if not bindings:
            continue
        parent = asset.exports[index].outer_index
        if parent <= 0:
            continue
        for pin_name, property_path in bindings:
            print(
                "%s pin=%s source=%s"
                % (
                    outer_path(asset, parent - 1),
                    pin_name,
                    ".".join(property_path),
                )
            )


if __name__ == "__main__":
    main()
