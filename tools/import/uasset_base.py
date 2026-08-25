import struct

PACKAGE_TAG = 0x9E2A83C1
LEGACY_FILE_VERSION = -9
UE5_PROPERTY_TAG_COMPLETE_TYPE_NAME = 1008
UE5_SCRIPT_SERIALIZATION_OFFSET = 1010
PKG_FILTER_EDITOR_ONLY = 0x80000000

TAG_FLAG_HAS_ARRAY_INDEX = 0x01
TAG_FLAG_HAS_PROPERTY_GUID = 0x02
TAG_FLAG_HAS_PROPERTY_EXTENSIONS = 0x04
TAG_FLAG_BOOL_TRUE = 0x10

EXT_OVERRIDABLE_INFORMATION = 0x02
EXT_HAS_EXTERNALS_OBJECTS = 0x04


class ParseError(Exception):
    pass


class Reader:
    __slots__ = ("buf", "pos", "names")

    def __init__(self, buf, pos=0, names=None):
        self.buf = buf
        self.pos = pos
        self.names = names if names is not None else []

    def remaining(self):
        return len(self.buf) - self.pos

    def read(self, n):
        end = self.pos + n
        if end > len(self.buf):
            raise ParseError("read %d bytes at %d exceeds buffer size %d" % (n, self.pos, len(self.buf)))
        v = self.buf[self.pos:end]
        self.pos = end
        return v

    def u8(self):
        return self.read(1)[0]

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

    def boolean(self):
        return self.u32() != 0

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
            raise ParseError("name index %d out of range (%d names)" % (idx, len(self.names)))
        base = self.names[idx]
        if num == 0:
            return base
        return "%s_%d" % (base, num - 1)


class Export:
    __slots__ = ("class_index", "super_index", "template_index", "outer_index",
                 "object_name", "object_flags", "serial_size", "serial_offset")

    def __init__(self):
        self.class_index = 0
        self.super_index = 0
        self.template_index = 0
        self.outer_index = 0
        self.object_name = ""
        self.object_flags = 0
        self.serial_size = 0
        self.serial_offset = 0


class Import:
    __slots__ = ("class_package", "class_name", "outer_index", "object_name", "package_name")

    def __init__(self):
        self.class_package = ""
        self.class_name = ""
        self.outer_index = 0
        self.object_name = ""
        self.package_name = ""


class UAsset:
    def __init__(self, path):
        self.path = path
        with open(path, "rb") as f:
            self.buf = f.read()
        self.summary = {}
        self.names = []
        self.imports = []
        self.exports = []
        self._parse()

    def _parse(self):
        r = Reader(self.buf)
        tag = r.u32()
        if tag != PACKAGE_TAG:
            raise ParseError("%s: bad package tag 0x%08X" % (self.path, tag))
        legacy = r.i32()
        if legacy != LEGACY_FILE_VERSION and legacy != -8:
            raise ParseError("%s: unsupported LegacyFileVersion %d" % (self.path, legacy))
        if legacy != -4:
            r.i32()
        ue4 = r.i32()
        ue5 = 0
        if legacy <= -8:
            ue5 = r.i32()
        licensee = r.i32()
        if ue4 == 0 and ue5 == 0 and licensee == 0:
            raise ParseError("%s: unversioned packages are not supported" % self.path)
        if ue5 < UE5_PROPERTY_TAG_COMPLETE_TYPE_NAME:
            raise ParseError("%s: FileVersionUE5 %d too old for tagged property parsing" % (self.path, ue5))
        self.summary["ue4"] = ue4
        self.summary["ue5"] = ue5
        self.summary["licensee"] = licensee
        saved_hash_layout = ue5 >= 1016
        if saved_hash_layout:
            r.read(20)
            total_header_size = r.i32()
            self.summary["total_header_size"] = total_header_size
        custom_count = r.i32()
        if custom_count < 0 or custom_count > 4096:
            raise ParseError("%s: implausible custom version count %d" % (self.path, custom_count))
        for _ in range(custom_count):
            r.read(16)
            r.i32()
        if not saved_hash_layout:
            total_header_size = r.i32()
            self.summary["total_header_size"] = total_header_size
        r.string()
        pkg_flags = r.u32()
        self.summary["flags"] = pkg_flags
        name_count = r.i32()
        name_offset = r.i32()
        if ue5 >= 1008:
            r.i32()
            r.i32()
        filter_editor_only = bool(pkg_flags & PKG_FILTER_EDITOR_ONLY)
        if not filter_editor_only:
            r.string()
        r.i32()
        r.i32()
        export_count = r.i32()
        export_offset = r.i32()
        import_count = r.i32()
        import_offset = r.i32()
        if ue5 >= 1015:
            r.i32()
            r.i32()
            r.i32()
            r.i32()
        if ue5 >= 1014:
            r.i32()
        depends_offset = r.i32()
        r.i32()
        r.i32()
        searchable_names_offset = r.i32()
        thumbnail_table_offset = r.i32()
        if ue5 >= 1018:
            r.i32()
            r.i32()
        if ue5 < 1016:
            r.read(16)
        if not filter_editor_only:
            r.read(16)
        gen_count = r.i32()
        for _ in range(gen_count):
            r.i32()
            r.i32()
        self._skip_engine_version(r)
        self._skip_engine_version(r)
        compression_flags = r.u32()
        if compression_flags != 0:
            raise ParseError("%s: compressed packages are not supported" % self.path)
        chunk_count = r.i32()
        if chunk_count != 0:
            raise ParseError("%s: package level compression chunks present" % self.path)
        r.u32()
        add_cook_count = r.i32()
        for _ in range(add_cook_count):
            r.string()
        r.i32()
        r.i64()
        if ue4 >= 322:
            r.i32()
        cid_count = r.i32()
        for _ in range(cid_count):
            r.i32()
        preload_dep_count = r.i32()
        r.i32()
        if ue5 >= 1001:
            r.i32()
        if ue5 >= 1002:
            r.i64()
        if ue5 >= 1009:
            r.i32()
        del depends_offset, searchable_names_offset, thumbnail_table_offset, preload_dep_count

        self._read_name_map(r, name_offset, name_count)
        self.exports = self._read_export_map(r, export_offset, export_count, ue5)
        self.imports = self._read_import_map(r, import_offset, import_count, ue5)

    @staticmethod
    def _skip_engine_version(r):
        r.u16()
        r.u16()
        r.u16()
        r.u32()
        r.string()

    def _read_name_map(self, r, offset, count):
        r.pos = offset
        names = []
        for _ in range(count):
            s = r.string()
            r.u16()
            r.u16()
            names.append(s)
        self.names = names
        r.names = names

    def _read_export_map(self, r, offset, count, ue5):
        r.pos = offset
        exports = []
        for _ in range(count):
            e = Export()
            e.class_index = r.i32()
            e.super_index = r.i32()
            e.template_index = r.i32()
            e.outer_index = r.i32()
            e.object_name = r.fname()
            e.object_flags = r.u32()
            e.serial_size = r.i64()
            e.serial_offset = r.i64()
            r.boolean()
            r.boolean()
            r.boolean()
            if ue5 < 1005:
                r.read(16)
            if ue5 >= 1006:
                r.boolean()
            r.u32()
            r.boolean()
            r.boolean()
            if ue5 >= 1003:
                r.boolean()
            r.i32()
            r.i32()
            r.i32()
            r.i32()
            r.i32()
            if ue5 >= UE5_SCRIPT_SERIALIZATION_OFFSET:
                r.i64()
                r.i64()
            exports.append(e)
        return exports

    def _read_import_map(self, r, offset, count, ue5):
        r.pos = offset
        imports = []
        for _ in range(count):
            imp = Import()
            imp.class_package = r.fname()
            imp.class_name = r.fname()
            imp.outer_index = r.i32()
            imp.object_name = r.fname()
            imp.package_name = r.fname()
            if ue5 >= 1003:
                r.boolean()
            imports.append(imp)
        return imports

    def class_of_export(self, index, depth=0):
        exp = self.exports[index]
        ci = exp.class_index
        while True:
            if ci > 0:
                target = ci - 1
                if target >= len(self.exports) or depth > 8:
                    return None
                sub = self.exports[target]
                name = sub.object_name
                ci = sub.class_index
                depth += 1
                if ci == 0:
                    return name
            elif ci < 0:
                imp = self.imports[-ci - 1]
                return imp.class_name
            else:
                return None


def read_type_nodes(r):
    nodes = []
    remaining = 1
    while remaining > 0:
        nm = r.fname()
        ic = r.i32()
        nodes.append((nm, ic))
        remaining += ic - 1
        if len(nodes) > 64:
            raise ParseError("property type node chain too long")
    return nodes


def type_param(nodes):
    root_name, root_inner = nodes[0]
    if root_inner >= 1 and len(nodes) > 1:
        return nodes[1][0]
    return None


def type_deep_param(nodes):
    for nm, ic in nodes[1:]:
        if ic == 0:
            return nm
    return None
