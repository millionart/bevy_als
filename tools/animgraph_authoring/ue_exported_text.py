import argparse
import json
import re


class ExportedTextError(ValueError):
    pass


def split_top_level(value):
    result = []
    start = 0
    depth = 0
    quoted = False
    escaped = False
    for index, character in enumerate(value):
        if quoted:
            if escaped:
                escaped = False
            elif character == "\\":
                escaped = True
            elif character == '"':
                quoted = False
            continue
        if character == '"':
            quoted = True
        elif character == "(":
            depth += 1
        elif character == ")":
            depth -= 1
            if depth < 0:
                raise ExportedTextError("struct closing delimiter is invalid")
        elif character == "," and depth == 0:
            result.append(value[start:index].strip())
            start = index + 1
    if quoted or depth != 0:
        raise ExportedTextError("struct delimiters are invalid")
    result.append(value[start:].strip())
    return [item for item in result if item]


def parse_struct(value):
    if not isinstance(value, str) or len(value) < 2 or value[0] != "(" or value[-1] != ")":
        raise ExportedTextError("struct literal is invalid")
    fields = {}
    for item in split_top_level(value[1:-1]):
        if "=" not in item:
            raise ExportedTextError("struct field is invalid")
        name, raw_value = item.split("=", 1)
        name = name.strip()
        raw_value = raw_value.strip()
        if not name or not raw_value or name in fields:
            raise ExportedTextError("struct field is missing or duplicated")
        fields[name] = raw_value
    return fields


def parse_list(value):
    if not isinstance(value, str) or len(value) < 2 or value[0] != "(" or value[-1] != ")":
        raise ExportedTextError("list literal is invalid")
    return split_top_level(value[1:-1])


def field(fields, name, default=None):
    if name in fields:
        return fields[name]
    if default is not None:
        return default
    raise ExportedTextError(f"struct field is missing: {name}")


def parse_bool(value):
    if value == "True":
        return True
    if value == "False":
        return False
    raise ExportedTextError("boolean value is invalid")


def parse_number(value):
    if not isinstance(value, str) or re.fullmatch(r"-?[0-9]+(?:\.[0-9]+)?", value) is None:
        raise ExportedTextError("numeric value is invalid")
    return float(value)


def parse_quoted(value):
    if not isinstance(value, str) or len(value) < 2 or value[0] != '"' or value[-1] != '"':
        raise ExportedTextError("quoted value is invalid")
    return value[1:-1].replace('\\"', '"').replace("\\\\", "\\")


def parse_range(value, default_min, default_max):
    fields = parse_struct(value)
    return {
        "min": parse_number(field(fields, "Min", str(default_min))),
        "max": parse_number(field(fields, "Max", str(default_max))),
    }


def parse_scale_bias(value):
    fields = parse_struct(value)
    unknown = set(fields) - {"Scale", "Bias"}
    if unknown:
        raise ExportedTextError("scale bias contains unsupported fields")
    return {
        "scale": parse_number(field(fields, "Scale", "1.0")),
        "bias": parse_number(field(fields, "Bias", "0.0")),
    }


def parse_scale_bias_clamp(value):
    fields = parse_struct(value)
    allowed = {
        "bMapRange",
        "bClampResult",
        "bInterpResult",
        "InRange",
        "OutRange",
        "Scale",
        "Bias",
        "ClampMin",
        "ClampMax",
        "InterpSpeedIncreasing",
        "InterpSpeedDecreasing",
    }
    unknown = set(fields) - allowed
    if unknown:
        raise ExportedTextError("scale bias clamp contains unsupported fields")
    return {
        "source_semantics": True,
        "map_range": parse_bool(field(fields, "bMapRange", "False")),
        "input_min": parse_range(field(fields, "InRange", "()"), 0.0, 1.0)["min"],
        "input_max": parse_range(field(fields, "InRange", "()"), 0.0, 1.0)["max"],
        "output_min": parse_range(field(fields, "OutRange", "()"), 0.0, 1.0)["min"],
        "output_max": parse_range(field(fields, "OutRange", "()"), 0.0, 1.0)["max"],
        "scale": parse_number(field(fields, "Scale", "1.0")),
        "bias": parse_number(field(fields, "Bias", "0.0")),
        "clamp_result": parse_bool(field(fields, "bClampResult", "False")),
        "clamp_min": parse_number(field(fields, "ClampMin", "0.0")),
        "clamp_max": parse_number(field(fields, "ClampMax", "1.0")),
        "interpolate_result": parse_bool(field(fields, "bInterpResult", "False")),
        "interpolate_speed_increasing": parse_number(field(fields, "InterpSpeedIncreasing", "10.0")),
        "interpolate_speed_decreasing": parse_number(field(fields, "InterpSpeedDecreasing", "10.0")),
    }


def reject_unknown_fields(fields, allowed):
    unknown = set(fields) - set(allowed)
    if unknown:
        raise ExportedTextError("struct contains unsupported fields: " + ",".join(sorted(unknown)))


def run_self_test():
    fields = parse_struct(
        '(Name="Example, Value",Nested=(Min=-2.000000,Max=3.000000),Enabled=True)'
    )
    assert parse_quoted(fields["Name"]) == "Example, Value"
    assert parse_range(fields["Nested"], 0.0, 1.0) == {"min": -2.0, "max": 3.0}
    assert parse_bool(fields["Enabled"])
    assert [parse_quoted(item) for item in parse_list('(\"A\",\"B, C\")')] == ["A", "B, C"]
    mapping = parse_scale_bias_clamp(
        "(bMapRange=True,bClampResult=True,bInterpResult=True,InRange=(Min=-500.000000,Max=-1000.000000),OutRange=(Min=1.000000,Max=0.500000),Scale=2.000000,Bias=-0.250000,ClampMin=0.250000,ClampMax=1.000000,InterpSpeedIncreasing=2.500000,InterpSpeedDecreasing=20.000000)"
    )
    assert mapping["input_min"] == -500.0
    assert mapping["input_max"] == -1000.0
    assert mapping["output_min"] == 1.0
    assert mapping["output_max"] == 0.5
    assert mapping["interpolate_speed_decreasing"] == 20.0
    try:
        parse_scale_bias_clamp("(Unsupported=True)")
    except ExportedTextError:
        pass
    else:
        raise AssertionError("unsupported source field was accepted")
    return {
        "nested_struct_fields_preserved": "PASS",
        "nested_list_values_preserved": "PASS",
        "source_scale_bias_clamp_parsed": "PASS",
        "unsupported_struct_field_rejected": "PASS",
    }


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if not args.self_test:
        raise SystemExit("--self-test is required")
    print(json.dumps(run_self_test(), ensure_ascii=False, indent=2))
