import argparse
import json
import math
import re
import struct
import sys
from pathlib import Path

from ue_control_rig_contract import ControlRigContractError, MATH_OPERATIONS
from ue_exported_text import parse_bool, parse_list, parse_number, parse_quoted, parse_struct

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "import"))
from coordinate_basis import SOURCE_LENGTH_TO_RUNTIME_SCALE, source_direction_to_runtime, source_rotation_to_runtime


LENGTH = SOURCE_LENGTH_TO_RUNTIME_SCALE
UNIT_ARGUMENTS = {
    "FRigUnit_BeginExecution::Execute": "",
    "FRigUnit_PrepareForExecution::Execute": "",
    "FAlsRigVMFunction_IsGameWorld::Execute": "BlockToRun:FName:Hidden",
    "FRigVMFunction_ControlFlowBranch::Execute": "Condition:bool:Input BlockToRun:FName:Hidden",
    "DISPATCH_RigVMDispatch_If::Condition:bool,True:FRigElementKey,False:FRigElementKey,Result:FRigElementKey":
        "Condition:bool:Input True:FRigElementKey:Input False:FRigElementKey:Input Result:FRigElementKey:Output",
    "FRigUnit_GetTransform::Execute":
        "Item:FRigElementKey:Input Space:ERigVMTransformSpace:Input bInitial:bool:Input Transform:FTransform:Output CachedIndex:FCachedRigElement:Hidden",
    "FRigUnit_GetCurveValue::Execute":
        "Curve:FName:Input Valid:bool:Output Value:float:Output CachedCurveIndex:FCachedRigElement:Hidden",
    "FRigUnit_SetScale::Execute":
        "Item:FRigElementKey:Input Space:ERigVMTransformSpace:Input bInitial:bool:Input Scale:FVector:Input Weight:float:Input bPropagateToChildren:bool:Input CachedIndex:FCachedRigElement:Hidden",
    "FRigUnit_SetTranslation::Execute":
        "Item:FRigElementKey:Input Space:ERigVMTransformSpace:Input bInitial:bool:Input Value:FVector:Input Weight:float:Input bPropagateToChildren:bool:Input CachedIndex:FCachedRigElement:Hidden",
    "FRigUnit_SetRotation::Execute":
        "Item:FRigElementKey:Input Space:ERigVMTransformSpace:Input bInitial:bool:Input Value:FQuat:Input Weight:float:Input bPropagateToChildren:bool:Input CachedIndex:FCachedRigElement:Hidden",
    "FAlsRigUnit_DistributeRotationSimple::Execute":
        "Items:TArray<FRigElementKey>:Input Rotation:FQuat:Input CachedItems:TArray<FCachedRigElement>:Hidden",
    "FAlsRigUnit_ChainLength::Execute":
        "AncestorItem:FRigElementKey:Input DescendantItem:FRigElementKey:Input bInitial:bool:Input Length:float:Output CachedAncestorItem:FCachedRigElement:Hidden CachedDescendantItem:FCachedRigElement:Hidden",
    "FAlsRigUnit_CalculatePoleVector::Execute":
        "ItemA:FRigElementKey:Input ItemB:FRigElementKey:Input ItemC:FRigElementKey:Input bInitial:bool:Input bSuccess:bool:Output ItemBLocation:FVector:Output ItemBProjectionLocation:FVector:Output PoleDirection:FVector:Output CachedItemA:FCachedRigElement:Hidden CachedItemB:FCachedRigElement:Hidden CachedItemC:FCachedRigElement:Hidden",
    "FAlsRigVMFunction_DamperExactVector::Execute":
        "Target:FVector:Input HalfLife:float:Input Current:FVector:Output bInitialized:bool:Hidden",
    "FRigUnit_SpringInterpV2::Execute":
        "Target:float:Input Strength:float:Input CriticalDamping:float:Input Force:float:Input bUseCurrentInput:bool:Input Current:float:Input TargetVelocityAmount:float:Input bInitializeFromTarget:bool:Input Result:float:Output Velocity:float:Output SimulatedResult:float:Hidden SpringState:FFloatSpringState:Hidden",
    "FAlsRigUnit_FootOffsetTrace::Execute":
        "FootTargetLocation:FVector:Input TraceChannel:ECollisionChannel:Input TraceDistanceUpward:float:Input TraceDistanceDownward:float:Input WalkableFloorAngle:float:Input FootHeight:float:Input bEnabled:bool:Input bDrawDebug:bool:Input OffsetLocationZ:float:Output OffsetNormal:FVector:Output",
    "FAlsRigUnit_ApplyFootOffsetLocation::Execute":
        "PelvisItem:FRigElementKey:Input ThighItem:FRigElementKey:Input FootTargetLocation:FVector:Input FootOffsetLocationZ:float:Input PelvisOffset:float:Input LegLength:float:Input MinPelvisToFootDistanceZ:float:Input MaxLegStretchRatio:float:Input OffsetInterpolationFrequency:float:Input OffsetInterpolationDampingRatio:float:Input OffsetInterpolationTargetVelocityAmount:float:Input bInitialized:bool:Hidden CachedPelvisItem:FCachedRigElement:Hidden CachedThighItem:FCachedRigElement:Hidden OffsetSpringState:FAlsSpringFloatState:Hidden OffsetLocationZ:float:Hidden FootLocation:FVector:Output",
    "FAlsRigUnit_ApplyFootOffsetRotation::Execute":
        "CalfItem:FRigElementKey:Input FootItem:FRigElementKey:Input FootTargetRotation:FQuat:Input FootOffsetNormal:FVector:Input Swing1LimitAngle:FFloatInterval:Input Swing2LimitAngle:FFloatInterval:Input TwistLimitAngle:FFloatInterval:Input OffsetInterpolationHalfLife:float:Input bInitialized:bool:Hidden CachedCalfItem:FCachedRigElement:Hidden CachedFootItem:FCachedRigElement:Hidden OffsetNormal:FVector:Hidden FootRotation:FQuat:Output FootInitialRotationCalfSpace:FQuat:Hidden",
    "FRigUnit_TwoBoneIKSimplePerItem::Execute":
        "ItemA:FRigElementKey:Input ItemB:FRigElementKey:Input EffectorItem:FRigElementKey:Input Effector:FTransform:Input PrimaryAxis:FVector:Input SecondaryAxis:FVector:Input SecondaryAxisWeight:float:Input PoleVector:FVector:Input PoleVectorKind:EControlRigVectorKind:Input PoleVectorSpace:FRigElementKey:Input bEnableStretch:bool:Input StretchStartRatio:float:Input StretchMaximumRatio:float:Input Weight:float:Input ItemALength:float:Input ItemBLength:float:Input bPropagateToChildren:bool:Input DebugSettings:FRigUnit_TwoBoneIKSimple_DebugSettings:Input CachedItemAIndex:FCachedRigElement:Hidden CachedItemBIndex:FCachedRigElement:Hidden CachedEffectorItemIndex:FCachedRigElement:Hidden CachedPoleVectorSpaceIndex:FCachedRigElement:Hidden",
}
STRUCT_FIELDS = {
    "FVector": "X:double Y:double Z:double",
    "FQuat": "X:double Y:double Z:double W:double",
    "FRotator": "Pitch:double Yaw:double Roll:double",
    "FTransform": "Rotation:FQuat Translation:FVector Scale3D:FVector",
    "FRigElementKey": "Type:ERigElementType Name:FName",
    "FCachedRigElement": "Key:FRigElementKey Index:uint16 ContainerVersion:int32",
    "FFloatInterval": "Min:float Max:float",
    "FAlsSpringFloatState": "Velocity:float PreviousTarget:float bStateValid:bool",
    "FFloatSpringState": "PreviousTarget:float Velocity:float bPrevTargetValid:bool",
    "FRigUnit_TwoBoneIKSimple_DebugSettings": "bEnabled:bool Scale:float WorldOffset:FTransform",
    "FAlsControlRigInput": "bUseHandIkBones:bool bUseFootIkBones:bool bFootTransformsValid:bool VelocityBlendForwardAmount:float VelocityBlendBackwardAmount:float SpineYawAngle:float PelvisOffsetAmount:float FootLeftLocation:FVector FootLeftRotation:FQuat FootRightLocation:FVector FootRightRotation:FQuat",
}
ENUMS = {"ERigElementType", "ERigVMTransformSpace", "EControlRigVectorKind", "ECollisionChannel"}


def cpp_type(value):
    return re.sub(r"TEnumAsByte<(.+)>", r"\1", re.sub(r"\s+", "", value))


def f32(value):
    return struct.unpack("f", struct.pack("f", value))[0]


class Value:
    def __init__(self, row, label, constraints):
        self.kind = cpp_type(row["cpp_type"])
        if row.get("property_class") == "BoolProperty":
            self.kind = "bool"
        self.label = label
        self.factor = None
        self.children = {}
        raw = row.get("default_text", row.get("exported_text", ""))
        if self.kind in {"float", "double"}:
            self.data = parse_number(raw)
            if self.kind == "float":
                self.data = f32(self.data)
        elif self.kind == "bool":
            self.data = parse_bool(raw)
        elif self.kind in {"int32", "uint8", "uint16"}:
            self.data = int(raw)
        elif self.kind == "FName" or self.kind in ENUMS:
            self.data = parse_quoted(raw) if raw.startswith('"') else raw
            if self.kind == "FName" and self.data == "None":
                self.data = ""
        elif self.kind.startswith("TArray<"):
            element_type = self.kind[7:-1]
            elements = row.get("elements")
            if elements is None:
                elements = [{"exported_text": value} for value in parse_list(raw or "()")]
            self.children = {str(index): Value({**element, "cpp_type": element_type}, label + f"[{index}]", constraints)
                for index, element in enumerate(elements)}
        elif self.kind in STRUCT_FIELDS:
            schema = [field.split(":") for field in STRUCT_FIELDS[self.kind].split()]
            fields = row.get("fields", [])
            if not fields:
                defaults = {"FFloatSpringState": {"PreviousTarget": "0.0", "Velocity": "0.0", "bPrevTargetValid": "False"},
                    "FRigElementKey": {"Type": "None", "Name": "None"}}.get(self.kind, {})
                values = defaults | parse_struct(raw or "()")
                if set(values) != {name for name, _ in schema}:
                    raise ControlRigContractError("target struct defaults are incomplete: " + label)
                fields = [{"name": name, "cpp_type": kind, "exported_text": values[name]} for name, kind in schema]
            actual = {field["name"]: field for field in fields}
            if len(actual) != len(fields) or set(actual) != {name for name, _ in schema}:
                raise ControlRigContractError("target struct fields do not match: " + label)
            for name, kind in schema:
                field = actual[name]
                actual_type = "bool" if field.get("property_class") == "BoolProperty" else cpp_type(field["cpp_type"])
                if actual_type != kind:
                    raise ControlRigContractError("target struct field type does not match: " + label + "." + name)
                self.children[name] = Value(field, label + "." + name, constraints)
            if self.kind == "FVector":
                equal(constraints, *self.children.values())
            elif self.kind in {"FQuat", "FRotator"}:
                for name, child in self.children.items():
                    assign(child, -1 if (self.kind == "FQuat" and name != "W") or name == "Yaw" else 1)
            elif self.kind == "FTransform":
                assign(self.children["Translation"], LENGTH)
                assign(self.children["Scale3D"], 1)
            elif self.kind == "FRigUnit_TwoBoneIKSimple_DebugSettings":
                assign(self.children["Scale"], LENGTH)
            elif self.kind == "FAlsControlRigInput":
                for name, child in self.children.items():
                    if child.kind == "FQuat":
                        continue
                    assign(child, LENGTH if name.endswith("Location") else -1 if name == "SpineYawAngle" else 1)
        else:
            raise ControlRigContractError("target value type is unsupported: " + self.kind)

    def numeric(self):
        if self.kind in {"float", "double"}:
            return [self]
        return [leaf for child in self.children.values() for leaf in child.numeric()]

    def field_order(self):
        return {"FVector": ["X", "Z", "Y"], "FQuat": ["X", "Z", "Y", "W"],
            "FRotator": ["Roll", "Yaw", "Pitch"]}.get(self.kind, list(self.children))

    def render(self, vector_component=False):
        if self.kind in {"float", "double"}:
            if self.factor is None:
                raise ControlRigContractError("target numeric units are unresolved: " + self.label)
            if math.isclose(self.factor, LENGTH, rel_tol=1e-12):
                if not vector_component:
                    return {"Centimeters" if self.kind == "float" else "Centimeters64": self.data}
                value = self.data
            else:
                value = self.data * self.factor
            return {"F32" if self.kind == "float" else "F64": f32(value) if self.kind == "float" else value}
        if self.kind == "bool":
            return {"Bool": self.data}
        if self.kind in {"int32", "uint8", "uint16"}:
            return {"I32": self.data}
        if self.kind == "FName" or self.kind in ENUMS:
            return {"Name": self.data}
        values = [self.children[name].render(self.kind == "FVector") for name in self.field_order()]
        variant = {"FVector": "Vector", "FQuat": "Quaternion", "FRotator": "EulerDegrees"}.get(self.kind, "List")
        return {variant: values if variant == "List" else [next(iter(value.values())) for value in values]}


def assign(node, factor):
    changed = False
    for leaf in node.numeric():
        if leaf.factor is None:
            leaf.factor = factor
            changed = True
        elif not math.isclose(leaf.factor, factor, rel_tol=1e-12):
            raise ControlRigContractError("target units conflict: " + leaf.label + f" ({leaf.factor} != {factor})")
    return changed


def equal(constraints, *nodes):
    if not nodes:
        return
    for other in nodes[1:]:
        a, b = nodes[0].numeric(), other.numeric()
        if len(a) != len(b):
            raise ControlRigContractError("target numeric layouts do not match")
        constraints.extend((x, y) for x, y in zip(a, b))


def unit_dimensions(name, args, constraints, constant_args):
    stem = name.split("::")[0]
    if stem == "FAlsRigUnit_FootOffsetTrace":
        if "TraceChannel" not in constant_args:
            raise ControlRigContractError("target requires a constant trace channel")
        if args["TraceChannel"].data != "ECC_Visibility":
            raise ControlRigContractError("target rig trace channel is unsupported: " + args["TraceChannel"].data)
    fixed = {
        "FRigUnit_GetCurveValue": {"Value": 1},
        "FRigUnit_SetScale": {"Scale": 1, "Weight": 1},
        "FRigUnit_SetTranslation": {"Value": LENGTH, "Weight": 1},
        "FRigUnit_SetRotation": {"Weight": 1},
        "FAlsRigUnit_ChainLength": {"Length": LENGTH},
        "FAlsRigUnit_CalculatePoleVector": {"ItemBLocation": LENGTH, "ItemBProjectionLocation": LENGTH, "PoleDirection": 1},
        "FAlsRigVMFunction_DamperExactVector": {"HalfLife": 1},
        "FRigUnit_SpringInterpV2": {"Strength": 1, "CriticalDamping": 1, "TargetVelocityAmount": 1},
        "FAlsRigUnit_FootOffsetTrace": {"FootTargetLocation": LENGTH, "TraceDistanceUpward": LENGTH,
            "TraceDistanceDownward": LENGTH, "FootHeight": LENGTH, "OffsetLocationZ": LENGTH, "WalkableFloorAngle": 1, "OffsetNormal": 1},
        "FAlsRigUnit_ApplyFootOffsetLocation": {**{key: LENGTH for key in ("FootTargetLocation", "FootOffsetLocationZ",
            "PelvisOffset", "LegLength", "MinPelvisToFootDistanceZ", "OffsetSpringState", "OffsetLocationZ", "FootLocation")},
            **{key: 1 for key in ("MaxLegStretchRatio", "OffsetInterpolationFrequency", "OffsetInterpolationDampingRatio", "OffsetInterpolationTargetVelocityAmount")}},
        "FAlsRigUnit_ApplyFootOffsetRotation": {"FootOffsetNormal": 1, "Swing1LimitAngle": -1,
            "Swing2LimitAngle": 1, "TwistLimitAngle": 1, "OffsetInterpolationHalfLife": 1, "OffsetNormal": 1},
        "FRigUnit_TwoBoneIKSimplePerItem": {**{key: 1 for key in ("PrimaryAxis", "SecondaryAxis", "SecondaryAxisWeight",
            "StretchStartRatio", "StretchMaximumRatio", "Weight")}, "ItemALength": LENGTH, "ItemBLength": LENGTH},
    }
    for key, factor in fixed.get(stem, {}).items():
        assign(args[key], factor)
    if stem == "FAlsRigVMFunction_DamperExactVector":
        equal(constraints, args["Target"], args["Current"])
    elif stem == "FRigUnit_SpringInterpV2":
        equal(constraints, *(args[key] for key in ("Target", "Force", "Current", "Result", "Velocity", "SimulatedResult")))
        for child in args["SpringState"].children.values():
            if child.numeric():
                equal(constraints, args["Target"], child)
    elif stem == "FRigUnit_TwoBoneIKSimplePerItem":
        kind = args["PoleVectorKind"]
        if "PoleVectorKind" not in constant_args or kind.data not in {"Location", "Direction"}:
            raise ControlRigContractError("target pole vector units require a constant vector kind")
        assign(args["PoleVector"], LENGTH if kind.data == "Location" else 1)


def compile_target(contract):
    if not contract.get("unit_defaults_complete") or not contract.get("execution_contract_complete"):
        raise ControlRigContractError("target requires complete native defaults and execution")
    program = contract["program"]
    constraints = []
    products = []
    positive = []
    memories = {bank: [Value(row, bank + ":" + str(index), constraints) for index, row in enumerate(rows)]
        for bank, rows in program["memories"].items() if bank != "Literal"}
    literal_nodes = []
    references = []

    def reference(ref):
        bank, index = ref["memory"], ref["register"]
        if bank == "Literal":
            root = Value(program["memories"][bank][index], f"Literal:{index}:{len(literal_nodes)}", constraints)
            literal_nodes.append(root)
        else:
            root = memories[bank][index]
        node = root
        path = []
        for name in ([str(ref["slice"])] if "slice" in ref else []) + ref["path"]:
            if name not in node.children and node.kind.startswith("TArray<TArray<") and name == "0" and not node.children:
                node.children[name] = Value({"cpp_type": node.kind[7:-1], "default_text": "()"}, node.label + "[0]", constraints)
            if name not in node.children or node.kind == "FQuat":
                raise ControlRigContractError("target field path is unsupported: " + node.label + "." + name)
            path.append(node.field_order().index(name))
            node = node.children[name]
        target_ref = {"register": root, "path": path}
        references.append(target_ref)
        return target_ref, node

    instructions = []
    for instruction in program["instructions"]:
        opcode = instruction["opcode"]
        if opcode == "Execute":
            function = program["functions"][instruction["function"]]
            name = function["name"]
            graph, node_id = instruction["model"]
            node = contract["graphs"][graph]["nodes"][node_id]
            pins = {pin["name"]: pin for pin in node["pins"] if pin["parent"] is None and not pin["cpp_type"].endswith("ExecuteContext")}
            if name in MATH_OPERATIONS:
                operation, inputs, output = MATH_OPERATIONS[name]
                schema = [(key, kind, "Input") for key, kind in inputs] + [("Result", output, "Output")]
            elif name in UNIT_ARGUMENTS:
                schema = [tuple(arg.split(":")) for arg in UNIT_ARGUMENTS[name].split()]
            else:
                raise ControlRigContractError("target unit is unsupported: " + name)
            if [(arg["name"], cpp_type(arg["cpp_type"])) for arg in function["arguments"]] != [(key, kind) for key, kind, _ in schema] or {
                    key: (cpp_type(pin["cpp_type"]), pin["direction"]) for key, pin in pins.items()} != {key: (kind, direction) for key, kind, direction in schema}:
                raise ControlRigContractError("target unit ABI does not match: " + name)
            pairs = [reference(ref) for ref in instruction["operands"]]
            if any(value.kind != kind for (_, kind, _), (_, value) in zip(schema, pairs)):
                raise ControlRigContractError("target operand type does not match: " + name)
            args = {key: pair[1] for (key, _, _), pair in zip(schema, pairs)}
            if name in MATH_OPERATIONS:
                ordered = [args[key] for key, _, _ in schema]
                if operation in {"F32Mul", "F64Mul", "VectorScale"}:
                    left, right, output = ordered
                    products.extend((a, right.numeric()[0], c) for a, c in zip(left.numeric(), output.numeric()))
                elif operation == "QuaternionFromEulerYzx":
                    pass
                elif operation == "F32Clamp01":
                    for value in ordered:
                        assign(value, 1)
                else:
                    if operation == "F64GreaterEqual":
                        ordered = ordered[:-1]
                    elif operation == "F64Lerp":
                        assign(args["T"], 1)
                        ordered = [args[key] for key in ("A", "B", "Result")]
                    equal(constraints, *ordered)
                    if operation in {"F64GreaterEqual", "F32Min", "F64Abs", "F64Clamp"}:
                        positive.extend(ordered[0].numeric())
                instructions.append({"Math": {"operation": operation, "inputs": [ref for ref, _ in pairs[:-1]], "output": pairs[-1][0]}})
            else:
                unit_dimensions(name, args, constraints, {key for (key, _, _), ref in zip(schema, instruction["operands"]) if ref["memory"] == "Literal"})
                instructions.append({"Call": {"function": name, "arguments": [ref for ref, _ in pairs],
                    "outputs": [i for i, (_, _, direction) in enumerate(schema) if direction in {"Output", "IO", "Hidden"}]}})
        elif opcode == "Copy":
            a, b = reference(instruction["input"]), reference(instruction["output"])
            equal(constraints, a[1], b[1])
            instructions.append({"Copy": {"input": a[0], "output": b[0]}})
        elif opcode == "Zero":
            instructions.append({"Zero": {"output": reference(instruction["output"])[0]}})
        elif opcode == "Jump":
            instructions.append({"Jump": {"instruction": instruction["instruction"]}})
        elif opcode == "JumpToBranch":
            instructions.append({"Branch": {"selector": reference(instruction["selector"])[0], "branches": instruction["branches"]}})
        elif opcode == "Exit":
            instructions.append("Exit")
        else:
            raise ControlRigContractError("target instruction is unsupported: " + opcode)
    changed = True
    while changed:
        changed = False
        for a, b in constraints:
            if a.factor is not None:
                changed |= assign(b, a.factor)
            elif b.factor is not None:
                changed |= assign(a, b.factor)
        for a, b, c in products:
            if a.factor is not None and b.factor is not None:
                changed |= assign(c, a.factor * b.factor)
            elif a.factor is not None and c.factor is not None:
                changed |= assign(b, c.factor / a.factor)
            elif b.factor is not None and c.factor is not None:
                changed |= assign(a, c.factor / b.factor)
        if not changed:
            free = next((leaf for value in literal_nodes for leaf in value.numeric() if leaf.factor is None), None)
            if free is not None:
                changed = assign(free, 1)
    invalid = [leaf.label + ":" + str(leaf.factor) for leaf in positive if leaf.factor is None or leaf.factor <= 0]
    if invalid:
        raise ControlRigContractError("target ordered math units are unsupported: " + ", ".join(invalid))
    initial_values = [value.render() for value in literal_nodes]
    registers = {value: index for index, value in enumerate(literal_nodes)}
    literal_count = len(initial_values)
    for bank in ("Work", "External"):
        for value in memories[bank]:
            registers[value] = len(initial_values)
            initial_values.append(value.render())
    for ref in references:
        ref["register"] = registers[ref["register"]]
    externals = {row["name"]: {"register": registers[value], "path": []}
        for row, value in zip(program["memories"]["External"], memories["External"])}
    hierarchy = contract["rig_hierarchy"]
    elements = hierarchy.get("elements", [])
    if len(elements) != hierarchy.get("element_count") or not elements or any(value["type"] not in {"Bone", "Curve"} for value in elements) or len({
            (value["type"], value["name"].casefold()) for value in elements}) != len(elements):
        raise ControlRigContractError("target hierarchy inventory is incomplete or unsupported")
    native_bones = hierarchy["bones"]
    names = [bone["name"] for bone in native_bones]
    if len(set(name.casefold() for name in names)) != len(names) or set(names) != {value["name"] for value in elements if value["type"] == "Bone"}:
        raise ControlRigContractError("target bone inventory does not match")

    def transform(value):
        return {"translation": source_direction_to_runtime([value["translation"][key] for key in "xyz"]),
            "rotation": source_rotation_to_runtime([value["rotation"][key] for key in "xyzw"]),
            "scale": source_direction_to_runtime([value["scale"][key] for key in "xyz"])}

    bones = [{"name": bone["name"], "parent": None if bone["parent"] == "None" else names.index(bone["parent"]),
        "initial_local": transform(bone["initial_local_transform"]), "initial_global": transform(bone["initial_global_transform"])} for bone in native_bones]
    if any(bone["parent"] is not None and bone["parent"] >= index for index, bone in enumerate(bones)):
        raise ControlRigContractError("target bone parents must precede children")
    curves = {value["name"]: f32(value["value"]) if value["value_set"] else None for value in elements if value["type"] == "Curve"}
    return {"format": "control_rig_target_v1", "program": {"initial_values": initial_values, "literal_count": literal_count,
        "entries": program["entries"], "instructions": instructions}, "externals": externals, "bones": bones, "curves": curves,
        "translation_tolerance": f32(1e-4), "geometry": {"meters_per_unit": LENGTH,
            "near_zero": f32(1e-4), "normalize_squared": f32(1e-8), "minimum_length": f32(1e-8),
            "solve_near_zero": 1e-4, "basis": [source_direction_to_runtime(value) for value in ([1, 0, 0], [0, 1, 0], [0, 0, 1])]}}


def definition_ron(target):
    def atom(value):
        return json.dumps(value, ensure_ascii=False, allow_nan=False)

    def sequence(values):
        return "[" + ", ".join(values) + "]"

    def record(values):
        return "(" + ", ".join(key + ": " + value for key, value in values.items()) + ")"

    def mapping(values, render):
        return "{" + ", ".join(atom(key) + ": " + render(value) for key, value in values.items()) + "}"

    def optional(value):
        return "None" if value is None else "Some(" + atom(value) + ")"

    def vector(value):
        return "(" + ", ".join(atom(number) for number in value) + ")"

    def reference(value):
        return record({"register": atom(value["register"]), "path": atom(value["path"])})

    def rig_value(value):
        kind, data = next(iter(value.items()))
        inner = sequence(rig_value(item) for item in data) if kind == "List" else (
            vector(data) if kind in {"Vector", "EulerDegrees", "Quaternion"} else atom(data))
        return kind + "(" + inner + ")"

    def instruction(value):
        if value == "Exit":
            return "Exit"
        kind, data = next(iter(value.items()))
        fields = {}
        for key, value in data.items():
            if key == "operation":
                fields[key] = value
            elif key in {"input", "output", "selector"}:
                fields[key] = reference(value)
            elif key in {"inputs", "arguments"}:
                fields[key] = sequence(reference(item) for item in value)
            elif key == "branches":
                fields[key] = mapping(value, atom)
            else:
                fields[key] = atom(value)
        return kind + record(fields)

    def transform(value):
        return record({key: vector(value[key]) for key in ("translation", "rotation", "scale")})

    program = target["program"]
    return record({
        "program": record({"initial_values": sequence(rig_value(value) for value in program["initial_values"]),
            "literal_count": atom(program["literal_count"]), "entries": mapping(program["entries"], atom),
            "instructions": sequence(instruction(value) for value in program["instructions"])}),
        "externals": mapping(target["externals"], reference),
        "bones": sequence(record({"name": atom(bone["name"]), "parent": optional(bone["parent"]),
            "initial_local": transform(bone["initial_local"]), "initial_global": transform(bone["initial_global"])}) for bone in target["bones"]),
        "curves": mapping(target["curves"], optional), "translation_tolerance": atom(target["translation_tolerance"]),
        "geometry": record({key: "(" + ", ".join(vector(axis) for axis in value) + ")" if key == "basis" else atom(value)
            for key, value in target["geometry"].items()}),
    })


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--contract", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if args.output.resolve() == args.contract.resolve():
        raise ControlRigContractError("target output cannot overwrite its source contract")
    result = compile_target(json.loads(args.contract.read_text(encoding="utf-8")))
    args.output.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"success": True, "instructions": len(result["program"]["instructions"])}))


if __name__ == "__main__":
    main()
