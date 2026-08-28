import json
import re
import struct
import sys
import uuid
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SRC = ROOT / "assets" / "animations" / "Base" / "A_Als_Idle.gltf"
OUT_RAG = ROOT / "assets" / "ragdolls" / "als_manny.rag.ron"
OUT_BM = ROOT / "assets" / "ragdoll_bone_maps" / "als_manny.bm.ron"
TOTAL_MASS = 75.0

BODIES = [
    ("head", "head", 0.11, 0.06),
    ("chest", "spine_03", 0.13, 0.19),
    ("stomach", "spine_01", 0.12, 0.15),
    ("pelvis", "pelvis", 0.13, 0.12),
    ("upperarm.L", "upperarm_l", 0.055, 0.14),
    ("forearm.L", "lowerarm_l", 0.05, 0.13),
    ("hand.L", "hand_l", 0.045, 0.05),
    ("upperarm.R", "upperarm_r", 0.055, 0.14),
    ("forearm.R", "lowerarm_r", 0.05, 0.13),
    ("hand.R", "hand_r", 0.045, 0.05),
    ("thigh.L", "thigh_l", 0.075, 0.20),
    ("calf.L", "calf_l", 0.06, 0.20),
    ("foot.L", "foot_l", 0.05, 0.08),
    ("thigh.R", "thigh_r", 0.075, 0.20),
    ("calf.R", "calf_r", 0.06, 0.20),
    ("foot.R", "foot_r", 0.05, 0.08),
]

JOINTS = [
    ("neck", "chest", "head", "head"),
    ("spine_top", "stomach", "chest", "spine_03"),
    ("spine_bottom", "pelvis", "stomach", "spine_01"),
    ("shoulder.L", "chest", "upperarm.L", "upperarm_l"),
    ("elbow.L", "upperarm.L", "forearm.L", "lowerarm_l"),
    ("wrist.L", "forearm.L", "hand.L", "hand_l"),
    ("shoulder.R", "chest", "upperarm.R", "upperarm_r"),
    ("elbow.R", "upperarm.R", "forearm.R", "lowerarm_r"),
    ("wrist.R", "forearm.R", "hand.R", "hand_r"),
    ("hip.L", "pelvis", "thigh.L", "thigh_l"),
    ("knee.L", "thigh.L", "calf.L", "calf_l"),
    ("ankle.L", "calf.L", "foot.L", "foot_l"),
    ("hip.R", "pelvis", "thigh.R", "thigh_r"),
    ("knee.R", "thigh.R", "calf.R", "calf_r"),
    ("ankle.R", "calf.R", "foot.R", "foot_r"),
]

BODY_ANCHOR = {
    "head": "head", "chest": "spine_03", "stomach": "spine_01", "pelvis": "pelvis",
    "upperarm.L": "upperarm_l", "forearm.L": "lowerarm_l", "hand.L": "hand_l",
    "upperarm.R": "upperarm_r", "forearm.R": "lowerarm_r", "hand.R": "hand_r",
    "thigh.L": "thigh_l", "calf.L": "calf_l", "foot.L": "foot_l",
    "thigh.R": "thigh_r", "calf.R": "calf_r", "foot.R": "foot_r",
}

BONE_TO_BODY = {
    "head": "head",
    "neck_01": "head",
    "spine_03": "chest",
    "spine_02": "chest",
    "spine_01": "stomach",
    "pelvis": "pelvis",
    "clavicle_l": "chest",
    "clavicle_r": "chest",
    "upperarm_l": "upperarm.L",
    "lowerarm_l": "forearm.L",
    "hand_l": "hand.L",
    "upperarm_r": "upperarm.R",
    "lowerarm_r": "forearm.R",
    "hand_r": "hand.R",
    "thigh_l": "thigh.L",
    "calf_l": "calf.L",
    "foot_l": "foot.L",
    "ball_l": "foot.L",
    "thigh_r": "thigh.R",
    "calf_r": "calf.R",
    "foot_r": "foot.R",
    "ball_r": "foot.R",
}


def mat_of(n):
    if "matrix" in n:
        m = n["matrix"]
        return [[m[0], m[4], m[8], m[12]],
                [m[1], m[5], m[9], m[13]],
                [m[2], m[6], m[10], m[14]],
                [m[3], m[7], m[11], m[15]]]
    t = n.get("translation", [0, 0, 0])
    r = n.get("rotation", [0, 0, 0, 1])
    x, y, z, w = r
    return [[1 - 2 * (y * y + z * z), 2 * (x * y - z * w), 2 * (x * z + y * w), 0],
            [2 * (x * y + z * w), 1 - 2 * (x * x + z * z), 2 * (y * z - x * w), 0],
            [2 * (x * z - y * w), 2 * (y * z + x * w), 1 - 2 * (x * x + y * y), 0],
            [t[0], t[1], t[2], 1]]


def mmul(a, b):
    return [[sum(a[i][k] * b[k][j] for k in range(4)) for j in range(4)] for i in range(4)]


def rigid_inverse(m):
    r = [[m[j][i] for j in range(3)] for i in range(3)]
    t = [m[0][3], m[1][3], m[2][3]]
    inv = [[r[i][j] for j in range(3)] + [-(r[i][0] * t[0] + r[i][1] * t[1] + r[i][2] * t[2])]
           for i in range(3)]
    inv.append([0, 0, 0, 1])
    return inv


def quat_of(m):
    tr = m[0][0] + m[1][1] + m[2][2]
    if tr > 0:
        s = math_sqrt(tr + 1.0) * 2
        w = 0.25 * s
        x = (m[2][1] - m[1][2]) / s
        y = (m[0][2] - m[2][0]) / s
        z = (m[1][0] - m[0][1]) / s
    elif m[0][0] > m[1][1] and m[0][0] > m[2][2]:
        s = math_sqrt(1.0 + m[0][0] - m[1][1] - m[2][2]) * 2
        w = (m[2][1] - m[1][2]) / s
        x = 0.25 * s
        y = (m[0][1] + m[1][0]) / s
        z = (m[0][2] + m[2][0]) / s
    else:
        s = math_sqrt(1.0 + m[1][1] - m[0][0] - m[2][2]) * 2
        w = (m[0][2] - m[2][0]) / s
        x = (m[0][1] + m[1][0]) / s
        y = 0.25 * s
        z = (m[1][2] + m[2][1]) / s
    return (w, x, y, z)


def math_sqrt(v):
    import math
    return math.sqrt(max(v, 0.0))


def load_ibms(doc):
    buffers = []
    base = SRC.parent
    for b in doc["buffers"]:
        data = (base / b["uri"]).read_bytes()
        buffers.append((data, b.get("byteOffset", 0)))

    def read_accessor(idx):
        acc = doc["accessors"][idx]
        bv = doc["bufferViews"][acc["bufferView"]]
        data, boff = buffers[bv["buffer"]]
        start = boff + bv.get("byteOffset", 0) + acc.get("byteOffset", 0)
        out = []
        for i in range(acc["count"]):
            vals = struct.unpack_from("<16f", data, start + i * 64)
            out.append([[vals[0], vals[4], vals[8], vals[12]],
                        [vals[1], vals[5], vals[9], vals[13]],
                        [vals[2], vals[6], vals[10], vals[14]],
                        [vals[3], vals[7], vals[11], vals[15]]])
        return out

    return read_accessor


def main():
    doc = json.loads(SRC.read_text(encoding="utf-8"))
    nodes = doc["nodes"]
    scenes = doc["scenes"][doc.get("scene", 0)]

    world = {}

    def walk(i, parent):
        wm = mmul(parent, mat_of(nodes[i])) if parent else mat_of(nodes[i])
        world[i] = wm
        for c in nodes[i].get("children", []):
            walk(c, wm)

    ident = [[1 if i == j else 0 for j in range(4)] for i in range(4)]
    for r in scenes["nodes"]:
        walk(r, ident)

    byname = {nodes[i]["name"]: world[i] for i in world if "name" in nodes[i]}
    read_accessor = load_ibms(doc)
    skin = doc["skins"][0]
    ibms = read_accessor(skin["inverseBindMatrices"])
    rest = {}
    for jname, ibm in zip((nodes[j]["name"] for j in skin["joints"]), ibms):
        rest[jname] = rigid_inverse(ibm)

    parents = {}
    names = {}
    for index, node in enumerate(nodes):
        name = node.get("name")
        if name:
            names[name] = index
        for child in node.get("children", []):
            parents[child] = index

    def path_parts(name):
        parts = []
        index = names[name]
        while True:
            current = nodes[index].get("name") or f"GltfNode{index}"
            parts.append(current)
            if index not in parents:
                break
            index = parents[index]
        return list(reversed(parts))

    def path_ron(name):
        return ", ".join(f'"{part}"' for part in path_parts(name))

    missing = [b for _, b, _, _ in BODIES if b not in rest]
    if missing:
        sys.exit(f"missing bones: {missing}")

    def tvec(name):
        m = rest[name]
        return m[0][3], m[1][3], m[2][3]

    bone_map_only = "--bone-map-only" in sys.argv
    if bone_map_only:
        existing_rag = OUT_RAG.read_text(encoding="utf-8")
        uuids = {}
        for label, _bone, _radius, _half_len in BODIES:
            pattern = (
                r'\(\s*uuid:\s*"(?P<uuid>[^"]+)",\s*\):\s*\('
                r'\s*id:\s*\(\s*uuid:\s*"[^"]+",\s*\),\s*'
                + rf'label:\s*"{re.escape(label)}",'
            )
            match = re.search(pattern, existing_rag, re.DOTALL)
            if match is None:
                sys.exit(f"missing body UUID: {label}")
            uuids[label] = match.group("uuid")
    else:
        uuids = {label: str(uuid.uuid4()) for label, _, _, _ in BODIES}

    rag = ["(", "    bodies: {"]
    collider_ids = {}
    for label, bone, _r, _h in BODIES:
        collider_ids[label] = str(uuid.uuid4())

    for label, bone, radius, half_len in BODIES:
        bid = uuids[label]
        bx, by, bz = tvec(bone)
        cid = collider_ids[label]
        rag.append(f"""        (
            uuid: "{bid}",
        ): (
            id: (
                uuid: "{bid}",
            ),
            label: "{label}",
            offset: ({bx:.4}, {by:.4}, {bz:.4}),
            colliders: [
                (
                    uuid: "{cid}",
                ),
            ],
            default_mode: Dynamic,
            use_symmetry: false,
            created_from: None,
        ),""")
    rag.append("    },")

    rag.append("    colliders: {")
    for label, bone, radius, half_len in BODIES:
        cid = collider_ids[label]
        rag.append(f"""        (
            uuid: "{cid}",
        ): (
            id: (
                uuid: "{cid}",
            ),
            local_offset: (
                rotation: (0.0, 0.0, 0.0, 1.0),
                translation: (0.0, 0.0, 0.0),
            ),
            shape: Capsule((
                half_length: {half_len:.4},
                radius: {radius:.4},
            )),
            layer_membership: 1,
            layer_filter: 1,
            override_layers: false,
            mass_mode: ByVolume,
            label: "{label}_cap",
            created_from: None,
        ),""")
    rag.append("    },")

    bm = ["(", "    bones_from_bodies: {"]
    for bone, body in sorted(BONE_TO_BODY.items()):
        bid = uuids[body]
        bw = rest[BODY_ANCHOR[body]]
        bone_w = rest[bone]
        path = path_ron(bone)
        rel = mmul(rigid_inverse(bw), bone_w)
        q = quat_of(rel)
        tx, ty, tz = rel[0][3], rel[1][3], rel[2][3]
        bm.append(f"""        [
            {path},
        ]: (
            bone_id: [
                {path},
            ],
            bodies: [
                (
                    body: (
                        uuid: "{bid}",
                    ),
                    weight: 1.0,
                    offset: (
                        rotation: ({q[0]:.6f}, {q[1]:.6f}, {q[2]:.6f}, {q[3]:.6f}),
                        translation: ({tx:.4}, {ty:.4}, {tz:.4}),
                    ),
                    override_offset: false,
                ),
            ],
            created_from: None,
        ),""")
    bm.append("    },")
    bm.append("    bodies_from_bones: {")
    for label, bone, _radius, _half_len in BODIES:
        bid = uuids[label]
        body_w = rest[BODY_ANCHOR[label]]
        bone_w = rest[bone]
        path = path_ron(bone)
        body_to_bone = mmul(rigid_inverse(body_w), bone_w)
        bone_to_body = rigid_inverse(body_to_bone)
        q = quat_of(bone_to_body)
        tx, ty, tz = bone_to_body[0][3], bone_to_body[1][3], bone_to_body[2][3]
        bm.append(f'''        (
            uuid: "{bid}",
        ): (
            body_id: (
                uuid: "{bid}",
            ),
            bone: (
                bone: [
                    {path},
                ],
                offset: (
                    rotation: ({q[0]:.6f}, {q[1]:.6f}, {q[2]:.6f}, {q[3]:.6f}),
                    translation: ({tx:.4}, {ty:.4}, {tz:.4}),
                ),
                override_offset: false,
            ),
            created_from: None,
        ),''')
    bm.append("    },")
    bm.append('    skeleton: "skeletons/SKM_Als.skn.ron",')
    bm.append('    ragdoll: "ragdolls/als_manny.rag.ron",')
    bm.append("""    skeleton_symmetry: (
        name_mapper: Pattern((
            key_1: "_l",
            key_2: "_r",
            pattern_before: "^.*",
            pattern_after: "$",
        )),
        mode: MirrorX,
    ),
)""")

    joints_block = ["    joints: {"]
    for label, body_a, body_b, anchor_bone in JOINTS:
        jid = str(uuid.uuid4())
        ax, ay, az = tvec(anchor_bone)
        joints_block.append(f"""        (
            uuid: "{jid}",
        ): (
            id: (
                uuid: "{jid}",
            ),
            label: "{label}",
            use_symmetry: false,
            created_from: None,
            variant: Spherical((
                body1: (
                    uuid: "{uuids[body_a]}",
                ),
                body2: (
                    uuid: "{uuids[body_b]}",
                ),
                position: ({ax:.4}, {ay:.4}, {az:.4}),
                twist_axis: (0.0, 0.0, 1.0),
                swing_limit: None,
                twist_limit: None,
                point_compliance: 0.0,
                swing_compliance: 0.0,
                twist_compliance: 0.0,
                swing_axis: (1.0, 0.0, 0.0),
            )),
        ),""")
    joints_block.append("    },")

    rag.extend(joints_block)
    rag.append('    suffixes: (')
    rag.append('        original: ".R",')
    rag.append('        mirror: ".L",')
    rag.append('    ),')
    rag.append(f'    total_mass: {TOTAL_MASS:.3},')
    rag.append(')')

    if not bone_map_only:
        OUT_RAG.write_text("\n".join(rag) + "\n", encoding="utf-8")
    OUT_BM.write_text("\n".join(bm) + "\n", encoding="utf-8")
    if not bone_map_only:
        print(f"wrote {OUT_RAG}")
    print(f"wrote {OUT_BM} ({len(BONE_TO_BODY)} bone mappings)")


if __name__ == "__main__":
    main()
