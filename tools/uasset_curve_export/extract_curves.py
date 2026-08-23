from uasset import ParseError, Reader, read_type_nodes

RCIM_LINEAR = 0
RCIM_CONSTANT = 1
RCIM_CUBIC = 2
RCCE_NONE = 5

TAGGED_STRUCTS = {
    "RichCurve": {"DefaultValue", "PreInfinityExtrap", "PostInfinityExtrap", "Keys"},
    "FloatCurve": {"LastObservedName_DEPRECATED", "Name_DEPRECATED", "Color",
                   "Comment", "CurveName", "CurveTypeFlags", "FloatCurve"},
    "SmartName": {"DisplayName"},
    "RawCurveTracks": {"FloatCurves", "TransformCurves"},
}

TOP_LEVEL_WANT = {"RawCurveData"}

SCALAR_FIELDS = {
    "IntProperty": "i32",
    "Int64Property": "i64",
    "FloatProperty": "f32",
    "DoubleProperty": "f64",
    "NameProperty": "fname",
    "StrProperty": "string",
    "ObjectProperty": "i32",
    "InterfaceProperty": "i32",
}

ARRAY_SCALARS = {
    "NameProperty": "fname",
    "StrProperty": "string",
    "FloatProperty": "f32",
    "DoubleProperty": "f64",
    "IntProperty": "i32",
    "ObjectProperty": "i32",
}


class CurveKey:
    __slots__ = ("interp", "tangent_mode", "weight_mode", "time", "value",
                 "arrive_tangent", "arrive_weight", "leave_tangent", "leave_weight")

    def __init__(self):
        self.interp = RCIM_LINEAR
        self.tangent_mode = 0
        self.weight_mode = 0
        self.time = 0.0
        self.value = 0.0
        self.arrive_tangent = 0.0
        self.arrive_weight = 0.0
        self.leave_tangent = 0.0
        self.leave_weight = 0.0


class FloatCurve:
    __slots__ = ("name", "flags", "default_value", "pre_infinity", "post_infinity", "keys")

    def __init__(self):
        self.name = ""
        self.flags = 0
        self.default_value = 0.0
        self.pre_infinity = RCCE_NONE
        self.post_infinity = RCCE_NONE
        self.keys = []


def _read_rich_curve_key(r):
    k = CurveKey()
    k.interp = r.u8()
    k.tangent_mode = r.u8()
    k.weight_mode = r.u8()
    k.time = r.f32()
    k.value = r.f32()
    k.arrive_tangent = r.f32()
    k.arrive_weight = r.f32()
    k.leave_tangent = r.f32()
    k.leave_weight = r.f32()
    return k


def _read_linear_color(r):
    return (r.f32(), r.f32(), r.f32(), r.f32())


NATIVE_STRUCTS = {
    "RichCurveKey": _read_rich_curve_key,
    "LinearColor": _read_linear_color,
}


def _skip_tag_extra_fields(r, flags):
    if flags & 0x01:
        r.i32()
    if flags & 0x02:
        r.read(16)
    if flags & 0x04:
        ext_bits = r.u8()
        if ext_bits & 0x02:
            r.u8()
            r.i32()
        if ext_bits & 0x04:
            r.i32()


def _read_struct_body(r, struct_name, end, depth, strict_end=True):
    if depth > 16:
        raise ParseError("tagged property nesting deeper than 16")
    want = TAGGED_STRUCTS[struct_name]
    props = {}
    while True:
        tag_start = r.pos
        name = r.fname()
        if name == "None":
            if strict_end and r.pos != end:
                raise ParseError(
                    "struct %s: terminator at %d but payload ends at %d"
                    % (struct_name, r.pos, end))
            if not strict_end and r.pos > end:
                raise ParseError(
                    "struct %s: terminator at %d past payload end %d"
                    % (struct_name, r.pos, end))
            return props
        if r.pos > end:
            raise ParseError("struct %s: overrun at %d (end %d)" % (struct_name, r.pos, end))
        nodes = read_type_nodes(r)
        size = r.i32()
        flags = r.u8()
        _skip_tag_extra_fields(r, flags)
        payload_start = r.pos
        payload_end = payload_start + size
        if payload_end > end:
            raise ParseError(
                "struct %s property %s: payload %d..%d exceeds parent end %d"
                % (struct_name, name, payload_start, payload_end, end))
        if name not in want:
            r.pos = payload_end
            continue
        value = _read_value(r, payload_start, size, nodes, depth + 1)
        consumed = r.pos - payload_start
        if consumed != size:
            raise ParseError(
                "property %s.%s consumed %d of %d bytes at %d"
                % (struct_name, name, consumed, size, tag_start))
        props[name] = value


def _read_top_level(r, want, end_pos, depth=0):
    props = {}
    while True:
        name = r.fname()
        if name == "None":
            return props
        if r.pos > end_pos:
            raise ParseError("tag stream overrun at %d (end %d)" % (r.pos, end_pos))
        nodes = read_type_nodes(r)
        size = r.i32()
        flags = r.u8()
        _skip_tag_extra_fields(r, flags)
        payload_start = r.pos
        if name not in want:
            r.pos = payload_start + size
            continue
        value = _read_value(r, payload_start, size, nodes, depth + 1)
        consumed = r.pos - payload_start
        if consumed != size:
            raise ParseError(
                "property %s consumed %d of %d bytes at %d"
                % (name, consumed, size, payload_start))
        props[name] = value


def _read_value(r, start, size, nodes, depth):
    end = start + size
    type_name = nodes[0][0]

    if type_name == "ByteProperty":
        if len(nodes) > 1 and nodes[1][1] >= 1:
            return ("scalar", "ByteProperty", r.fname())
        return ("scalar", "ByteProperty", r.u8())

    if type_name == "EnumProperty":
        return ("scalar", "EnumProperty", r.fname())


    if type_name == "StructProperty":
        if len(nodes) < 2:
            raise ParseError("StructProperty without struct name")
        sname = nodes[1][0]
        native = NATIVE_STRUCTS.get(sname)
        if native is not None:
            val = native(r)
            if r.pos != end:
                raise ParseError("native struct %s: consumed %d of %d" % (sname, r.pos - start, size))
            return ("native", sname, val)
        if sname in TAGGED_STRUCTS:
            return ("props", sname, _read_struct_body(r, sname, end, depth))
        r.pos = end
        return ("skip", sname, size)

    if type_name == "ArrayProperty":
        if len(nodes) < 2:
            raise ParseError("ArrayProperty without inner type")
        inner = nodes[1][0]
        elem_struct = nodes[2][0] if len(nodes) > 2 and nodes[1][1] >= 1 else None
        count = r.i32()
        native_elem = NATIVE_STRUCTS.get(elem_struct) if inner == "StructProperty" else None
        tagged_elem = elem_struct if inner == "StructProperty" and elem_struct in TAGGED_STRUCTS else None
        scalar_fn = ARRAY_SCALARS.get(inner)
        if native_elem is not None:
            arr = [native_elem(r) for _ in range(count)]
            if r.pos != end:
                raise ParseError("native array %s: consumed %d of %d" % (elem_struct, r.pos - start, size))
            return ("array", arr)
        if tagged_elem is not None:
            bodies = [_read_struct_body(r, tagged_elem, end, depth + 1, strict_end=False)
                      for _ in range(count)]
            if r.pos != end:
                raise ParseError("tagged struct array %s: consumed %d of %d" % (elem_struct, r.pos - start, size))
            return ("array", [("props", tagged_elem, b) for b in bodies])
        if scalar_fn is not None:
            arr = [getattr(r, scalar_fn)() for _ in range(count)]
            if r.pos != end:
                raise ParseError("scalar array %s: consumed %d of %d" % (inner, r.pos - start, size))
            return ("array", arr)
        r.pos = end
        return ("skip", "%s[]" % inner, size)

    fn = SCALAR_FIELDS.get(type_name)
    if fn is not None:
        return ("scalar", type_name, getattr(r, fn)())

    r.pos = end
    return ("skip", type_name, size)


def _first(props, key, kind):
    val = props.get(key)
    if val is None:
        return None
    if val[0] != kind:
        raise ParseError("unexpected layout for %s (%s)" % (key, val[0]))
    return val


def _decode_float_curves(asset, export_index):
    exp = asset.exports[export_index]
    start = exp.serial_offset
    end = start + exp.serial_size
    errors = []
    clean = None
    for probe in range(4):
        r = Reader(asset.buf, start + probe, asset.names)
        try:
            top = _read_top_level(r, TOP_LEVEL_WANT, end)
        except ParseError as exc:
            errors.append("probe+%d: %s" % (probe, exc))
            continue
        raw = top.get("RawCurveData")
        if raw is None or raw[0] != "props":
            errors.append("probe+%d: no RawCurveData struct" % probe)
            if clean is None:
                clean = top
            continue
        return r.pos, top
    if clean is not None:
        return None, clean
    if errors:
        raise ParseError("%s export '%s': %s" %
                         (asset.path, exp.object_name, "; ".join(errors)))
    raise ParseError("%s: unreachable" % asset.path)


def extract_float_curves(asset, export_index):
    _, top = _decode_float_curves(asset, export_index)
    if not top:
        return []
    raw = _first(top, "RawCurveData", "props")
    if raw is None:
        return []
    tracks = raw[2]
    arr = _first(tracks, "FloatCurves", "array")
    if arr is None:
        return []
    curves = []
    for elem in arr[1]:
        if elem is None or elem[0] != "props":
            continue
        fields = elem[2]
        curve = FloatCurve()
        cname = _first(fields, "CurveName", "scalar")
        if cname is not None:
            curve.name = cname[2]
        cflags = _first(fields, "CurveTypeFlags", "scalar")
        if cflags is not None:
            curve.flags = cflags[2]
        fc = _first(fields, "FloatCurve", "props")
        if fc is None:
            curves.append(curve)
            continue
        rich = fc[2]
        dv = _first(rich, "DefaultValue", "scalar")
        if dv is not None:
            curve.default_value = dv[2]
        pi = _first(rich, "PreInfinityExtrap", "scalar")
        if pi is not None:
            curve.pre_infinity = pi[2]
        po = _first(rich, "PostInfinityExtrap", "scalar")
        if po is not None:
            curve.post_infinity = po[2]
        keys = _first(rich, "Keys", "array")
        if keys is not None:
            curve.keys = list(keys[1])
        curves.append(curve)
    return curves


def is_anim_sequence(asset, export_index):
    cur = asset.exports[export_index].class_index
    hops = 0
    while hops <= 32:
        hops += 1
        if cur == 0:
            return False
        if cur < 0:
            imp = asset.imports[-cur - 1]
            return imp.object_name == "AnimSequence"
        target = cur - 1
        if target >= len(asset.exports):
            return False
        cur = asset.exports[target].super_index
    return False




