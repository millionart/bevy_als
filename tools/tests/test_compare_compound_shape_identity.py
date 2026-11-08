import importlib.util
from pathlib import Path
import sys
import unittest

directory = Path(__file__).resolve().parents[1] / "import"
sys.path.insert(0, str(directory))
spec = importlib.util.spec_from_file_location("compare_compound_shape_identity", directory / "compare_compound_shape_identity.py")
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class CompoundShapeIdentityTest(unittest.TestCase):
    def test_source_basis_and_swapped_shape(self):
        sample = {"input": {"mesh_asset_path": "/ALS/Mesh", "mesh_shape_index": 0},
                  "other_geometry": {"convex_core_vertices_cm": [{"x": 100, "y": 200, "z": 300}]}}
        native = {"physics_contact_probe": {"samples": [sample]}}
        matching = module.compare(native, {"Mesh": [[[1, 3, 2]]]})[0]
        swapped = module.compare(native, {"Mesh": [[[2, 3, 2]]]})[0]
        self.assertEqual(matching["source_to_runtime_m"], 0)
        self.assertEqual(matching["runtime_to_source_m"], 0)
        self.assertEqual(swapped["source_to_runtime_m"], 1)
        self.assertEqual(swapped["runtime_to_source_m"], 1)


if __name__ == "__main__":
    unittest.main()
