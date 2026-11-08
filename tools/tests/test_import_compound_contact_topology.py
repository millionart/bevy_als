import copy
import importlib.util
from pathlib import Path
import sys
import unittest

directory = Path(__file__).resolve().parents[1] / "import"
sys.path.insert(0, str(directory))
spec = importlib.util.spec_from_file_location("import_compound_contact_topology", directory / "import_compound_contact_topology.py")
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class CompoundContactTopologyTest(unittest.TestCase):
    def test_source_basis_winding_and_shape_identity(self):
        point = lambda x, y, z: {"x": x, "y": y, "z": z}
        geometry = {"kind": "convex", "shape_index": 0, "convex_core_scale": point(1, 1, 1),
                    "shape_to_body": {"translation": point(0, 0, 0), "scale": point(1, 1, 1),
                                      "rotation": {"x": 0, "y": 0, "z": 0, "w": 1}},
                    "convex_core_vertices_cm": [point(100, 0, 0), point(0, 100, 0), point(0, 0, 100), point(0, 0, 0)],
                    "convex_core_planes": [{"normal": point(0, 0, 1), "origin_cm": point(0, 0, 100), "vertex_indices": [0, 2, 1]}],
                    "margin_cm": 2}
        sample = {"input": {"mesh_asset_path": "/ALS/Mesh", "mesh_shape_index": 0}, "other_geometry": geometry}
        native = {"physics_contact_probe": {"samples": [sample]}}
        result = module.compile_topology(native, {"Mesh": [[]]})["Mesh"][0]
        self.assertEqual(result["vertices_m"][1], [0, 0, 1])
        self.assertEqual(result["faces"][0]["normal"], [0, 1, 0])
        self.assertEqual(result["faces"][0]["vertices"], [0, 2, 1])
        self.assertEqual(result["winding"], -1)
        self.assertEqual(result["margin_m"], 0.02)
        invalid = copy.deepcopy(native)
        invalid["physics_contact_probe"]["samples"][0]["other_geometry"]["convex_core_planes"][0]["vertex_indices"][0] = 4
        with self.assertRaises(ValueError):
            module.compile_topology(invalid, {"Mesh": [[]]})
        with self.assertRaises(ValueError):
            module.compile_topology(native, {"Mesh": [[], []]})


if __name__ == "__main__":
    unittest.main()
