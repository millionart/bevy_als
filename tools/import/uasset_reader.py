"""Generic UE editor-package (.uasset/.umap) tagged-property reader.

Builds on the curve-export parser (summary / name map / import / export maps)
and captures *all* tagged properties of an export. Unknown or exotic payloads
are skipped with markers so one exotic field never aborts a whole export.

Editor (uncooked) packages serialize FVector/FTransform as doubles.
"""

import struct

import os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from uasset_base import ParseError, UAsset, read_type_nodes

TAG_FLAG_HAS_ARRAY_INDEX = 0x01
TAG_FLAG_HAS_PROPERTY_GUID = 0x02
TAG_FLAG_HAS_PROPERTY_EXTENSIONS = 0x04
TAG_FLAG_BOOL_TRUE = 0x10


class Skip(Exception):
    """Raised to skip one property payload; caller seeks to payload end."""

    def __init__(self, reason):
        self.reason = reason
        super().__init__(reason)


class PropReader:
    def __init__(self, asset: UAsset):
        self.a = asset

    # ------------------------------------------------------------------ refs

    def obj_path(self, index):
        if index == 0:
            return None
        parts = []
        cur = index
        depth = 0
        while cur != 0 and depth < 24:
            if cur < 0:
                imp = self.a.imports[-cur - 1]
                parts.append(imp.object_name)
                cur = imp.outer_index
                if cur == 0:
                    pkg = imp.package_name
                    if pkg:
                        parts.append(pkg)
                    break
            else:
                exp = self.a.exports[cur - 1]
                parts.append(exp.object_name)
                cur = exp.outer_index
            depth += 1
        return "/".join(reversed(parts))

    def export_class(self, index):
        return self.a.class_of_export(index)

    # --------------------------------------------------------------- reading

    def read_export(self, index, want=None):
        exp = self.a.exports[index]
        start = exp.serial_offset
        end = start + exp.serial_size
        best = None
        for shift in (0, 1, 2, 3):
            r = _R(self.a, start + shift, end)
            props, ok, err = {}, True, None
            try:
                props = self._props(r, end, want, 0, top=True)
            except (Skip, ParseError, IndexError, struct.error) as exc:
                ok, err = False, "%s: %s" % (type(exc).__name__, exc)
            if ok:
                return props, True, None
            if best is None:
                best = (props, ok, err)
        return best

    def _skip_extra(self, r, flags):
        if flags & TAG_FLAG_HAS_ARRAY_INDEX:
            r.i32()
        if flags & TAG_FLAG_HAS_PROPERTY_GUID:
            r.read(16)
        if flags & TAG_FLAG_HAS_PROPERTY_EXTENSIONS:
            ext_bits = r.u8()
            if ext_bits & 0x02:
                r.u8()
                r.i32()
            if ext_bits & 0x04:
                r.i32()

    def _props(self, r, end, want, depth, top=False):
        if depth > 24:
            raise Skip("depth")
        props = {}
        old_tag = self.a.summary.get("ue5", 1012) < 1012
        while True:
            name = r.fname()
            if name == "None":
                return props
            if r.pos > end:
                raise Skip("overrun at header")

            if old_tag:
                base_tname = r.fname()
                size = r.i32()
                r.i32()
                sname = None
                bool_value = False
                if base_tname == "StructProperty":
                    sname = r.fname()
                    r.read(16)
                    if sname == "None":
                        nodes = [("StructProperty", 0)]
                    else:
                        nodes = [("StructProperty", 1), (sname, 0)]
                elif base_tname == "BoolProperty":
                    bool_value = r.u32() != 0
                    nodes = [("BoolProperty", 0)]
                elif base_tname == "ByteProperty":
                    ename = r.fname()
                    nodes = [("ByteProperty", 1), (ename, 0)] if ename != "None" else [("ByteProperty", 0)]
                elif base_tname == "EnumProperty":
                    ename = r.fname()
                    nodes = [("EnumProperty", 1), (ename, 0)]
                elif base_tname == "ArrayProperty":
                    inner = r.fname()
                    nodes = [("ArrayProperty", 1), (inner, 0)]
                elif base_tname == "SetProperty":
                    inner = r.fname()
                    nodes = [("SetProperty", 1), (inner, 0)]
                elif base_tname == "MapProperty":
                    k = r.fname()
                    v = r.fname()
                    nodes = [("MapProperty", 3), (k, 1), (k, 0), (v, 0)]
                else:
                    nodes = [(base_tname, 0)]
                has_guid = r.u32() != 0
                if has_guid:
                    r.read(16)
                flags = 0x00

                tname = nodes[0][0]
                if tname == "BoolProperty":
                    props[name] = bool_value
                    continue
            else:
                nodes = read_type_nodes(r)
                size = r.i32()
                flags = r.u8()
                self._skip_extra(r, flags)

                tname = nodes[0][0]

                if tname == "BoolProperty":
                    value = bool(flags & TAG_FLAG_BOOL_TRUE)
                    props[name] = value
                    continue

            payload_start = r.pos
            payload_end = payload_start + size
            if payload_end > end:
                raise Skip("payload exceeds parent")

            if want is not None and name not in want and not self._always(name):
                r.pos = payload_end
                continue

            try:
                props[name] = self._value(
                    r, payload_start, payload_end, nodes, depth + 1
                )
                if r.pos != payload_end:
                    raise Skip(
                        "consumed %d of %d" % (r.pos - payload_start, size)
                    )
            except Skip as sk:
                r.pos = payload_end
                props[name] = {"__skipped__": sk.reason}
        # unreachable

    def _always(self, name):
        return name in (
            "RootComponent",
            "StaticMesh",
            "RelativeLocation",
            "RelativeRotation",
            "RelativeScale3D",
            "Text",
            "Intensity",
            "LightColor",
            "bUseTemperature",
            "Temperature",
            "bAtmosphereSunLight",
            "HeightmapTexture",
            "ComponentSizeQuads",
            "SubsectionSizeQuads",
            "ScalarParameterValues",
            "VectorParameterValues",
            "TextureParameterValues",
            "Parent",
        )

    def _native(self, r, sname, end):
        def f3():
            return round(r.f32(), 6)

        def d3():
            return round(r.f64(), 6)

        table = {
            "Vector": lambda: {k: d3() for k in ("x", "y", "z")},
            "Vector3f": lambda: {k: f3() for k in ("x", "y", "z")},
            "Vector2D": lambda: {"x": f3(), "y": f3()},
            "Quat": lambda: {k: d3() for k in ("x", "y", "z", "w")},
            "Rotator": lambda: {
                "pitch": d3(),
                "yaw": d3(),
                "roll": d3(),
            },
            "LinearColor": lambda: {
                "r": f3(),
                "g": f3(),
                "b": f3(),
                "a": f3(),
            },
        }

        def color():
            b = r.u8(); g = r.u8(); rr = r.u8(); a = r.u8()
            return {"r": rr, "g": g, "b": b, "a": a}

        fn = color if sname == "Color" else table.get(sname)
        if fn is None:
            raise Skip("native struct %s" % sname)
        val = fn()
        if r.pos > end:
            raise Skip("native overrun")
        return val

    def _value(self, r, start, end, nodes, depth):
        if depth > 24:
            raise Skip("depth")
        tname = nodes[0][0]

        if tname == "IntProperty":
            return r.i32()
        if tname == "Int64Property":
            return r.i64()
        if tname == "UInt32Property":
            return r.u32()
        if tname == "UInt64Property":
            return struct.unpack("<Q", r.read(8))[0]
        if tname == "FloatProperty":
            return round(struct.unpack("<f", r.read(4))[0], 6)
        if tname == "DoubleProperty":
            return r.f64()
        if tname == "NameProperty":
            return r.fname()
        if tname == "StrProperty":
            return r.string()
        if tname == "ObjectProperty":
            return self.obj_path(r.i32())
        if tname == "InterfaceProperty":
            raise Skip("InterfaceProperty")
        if tname == "EnumProperty":
            enum_name = nodes[1][0] if len(nodes) > 1 and nodes[1][1] >= 1 else None
            return {"enum": enum_name, "value": r.fname()}
        if tname == "ByteProperty":
            if len(nodes) > 1 and nodes[1][1] >= 1:
                return {"enum_name": nodes[1][0], "value": r.fname()}
            return r.u8()

        if tname == "StructProperty":
            sname = nodes[1][0] if len(nodes) > 1 else "?"
            try:
                return self._native(r, sname, end)
            except Skip:
                r.pos = start
                try:
                    inner = self._props(r, end, None, depth + 1)
                    if r.pos != end:
                        raise Skip("struct residue %d" % (end - r.pos))
                    return {"__tagged__": inner}
                except (Skip, ParseError, IndexError, struct.error):
                    r.pos = end
                    return {"__struct__": sname}

        if tname == "ArrayProperty":
            inner = nodes[1][0]
            elem_struct = nodes[2][0] if len(nodes) > 2 and nodes[1][1] >= 1 else None
            count = r.i32()
            if count < 0 or count > 200000:
                raise Skip("array count")
            scalar = {
                "IntProperty": lambda: r.i32(),
                "Int64Property": lambda: r.i64(),
                "FloatProperty": lambda: struct.unpack("<f", r.read(4))[0],
                "DoubleProperty": lambda: r.f64(),
                "NameProperty": lambda: r.fname(),
                "StrProperty": lambda: r.string(),
                "ObjectProperty": lambda: self.obj_path(r.i32()),
                "BoolProperty": lambda: r.u8() != 0,
            }.get(inner)
            if scalar is not None:
                out = []
                for _ in range(count):
                    out.append(scalar())
                    if r.pos > end:
                        raise Skip("scalar array overrun")
                return out
            if inner == "StructProperty" and elem_struct:
                out = []
                try:
                    for i in range(count):
                        body = self._props(
                            r, min(end, 1 << 30), None, depth + 1
                        )
                        if r.pos > end:
                            raise Skip("struct array overrun")
                        out.append(body)
                except (Skip, ParseError, IndexError, struct.error):
                    raise Skip("element parse failed")
                return out
            raise Skip("array of %s" % inner)

        if tname == "SetProperty":
            inner = nodes[1][0] if len(nodes) > 1 else "?"
            count = r.i32()
            r.pos = end
            return {"set": inner, "count": count}

        if tname == "MapProperty":
            key_t = nodes[1][0]
            val_t = nodes[3][0] if len(nodes) > 3 else "?"
            count = r.i32()
            r.pos = end
            return {"map": [key_t, val_t], "count": count}

        if tname == "TextProperty":
            return self._text(r, end)

        if tname in (
            "SoftObjectProperty",
            "SoftObjectPath",
            "DelegateProperty",
            "MulticastInlineDelegateProperty",
            "MulticastSparseDelegateProperty",
            "WeakObjectProperty",
            "LazyObjectProperty",
            "ObjectPtrBase",
        ):
            raise Skip(tname)

        raise Skip("unknown type %s" % tname)

    def _text(self, r, end):
        flags = r.u8()
        if flags & 0x08:
            r.i32()
            r.string()
        history_type = r.i8()
        if history_type == -1:
            return r.string()
        if history_type == 0:
            count = r.i32()
            src = None
            for _ in range(max(count, 0)):
                key = r.string()
                s = r.string()
                if src is None:
                    src = s
            return src
        if history_type == 1:
            r.i32()
            source = r.string()
            r.string()
            r.i8()
            r.i32()
            r.i32()
            return source
        if history_type in (2, 3):
            r.i32()
            r.i32()
            r.string()
            r.string()
            return None
        raise Skip("text history %d" % history_type)


class _R:
    """Reader bound to the whole package buffer."""

    def __init__(self, asset, pos, end):
        self.a = asset
        self.names = asset.names
        self.buf = asset.buf
        self.pos = pos
        self.end = end

    @property
    def remaining(self):
        return len(self.buf) - self.pos

    def read(self, n):
        v = self.buf[self.pos:self.pos + n]
        self.pos += n
        return v

    def u8(self):
        v = self.buf[self.pos]
        self.pos += 1
        return v

    def i8(self):
        return struct.unpack("<b", self.read(1))[0]

    def u16(self):
        return struct.unpack("<H", self.read(2))[0]

    def i32(self):
        return struct.unpack("<i", self.read(4))[0]

    def u32(self):
        return struct.unpack("<I", self.read(4))[0]

    def i64(self):
        return struct.unpack("<q", self.read(8))[0]

    def f32(self):
        return struct.unpack("<f", self.read(4))[0]

    def f64(self):
        return struct.unpack("<d", self.read(8))[0]

    def string(self):
        n = self.i32()
        if n == 0:
            return ""
        if n < 0:
            data = self.read(-n * 2)
            return data.decode("utf-16-le", errors="replace").rstrip("\x00")
        data = self.read(n)
        return data.decode("utf-8", errors="replace").rstrip("\x00")

    def fname(self):
        idx = self.i32()
        num = self.i32()
        if idx < 0 or idx >= len(self.names):
            raise ParseError("fname index %d out of range" % idx)
        base = self.names[idx]
        if num == 0:
            return base
        return "%s_%d" % (base, num - 1)

    def object_ref_unused(self):
        raise NotImplementedError


def load(path):
    return UAsset(path)


def read_props(asset: UAsset, index, want=None):
    pr = PropReader(asset)
    return pr.read_export(index, want)
