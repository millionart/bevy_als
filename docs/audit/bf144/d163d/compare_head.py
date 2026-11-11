import gzip
import json
import struct
from pathlib import Path


def load(path):
    data = path.read_bytes() if path.is_file() else gzip.decompress(path.with_suffix(".json.gz").read_bytes())
    return json.loads(data)


directory = Path(__file__).resolve().parent
native = load(directory / "head.native.json")["mesh_additive_probe"]
previous = load(directory.parent / "d248b1/head.native.json")["mesh_additive_probe"]
assert native == previous
assert native["node_property"] == "AnimGraphNode_ApplyMeshSpaceAdditive"
witness = load(directory.parent / "454/contract-product_controlled.json")
assert witness["source_frame"] == 0 and witness["component"] == 1
sample = next(row for row in native["samples"] if row["name"] == "000:cc")
bone = next(row for row in sample["pose"]["bones"] if row["name"] == witness["bone"])
source_y = -bone["local_transform"]["rotation"]["z"]
runtime_y = struct.unpack("<f", struct.pack("<f", source_y))[0]
assert runtime_y == witness["native"] == witness["controlled"]
assert runtime_y != witness["current"]
print(json.dumps({"exact_native_samples": [row["name"] for row in native["samples"]],
                  "archived_product_order_witness": witness}, indent=2))
