import copy
import importlib.util
from pathlib import Path
import sys
import unittest

directory = Path(__file__).resolve().parents[1] / "import"
sys.path.insert(0, str(directory))
spec = importlib.util.spec_from_file_location("import_trimesh_contact_bounds", directory / "import_trimesh_contact_bounds.py")
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class TrimeshContactBoundsTest(unittest.TestCase):
    def test_source_basis_and_identity_validation(self):
        point = lambda x, y, z: {"x": x, "y": y, "z": z}
        geometry = {"kind": "trimesh", "shape_index": 0, "bounds_min_cm": point(-100, -200, -300),
                    "bounds_max_cm": point(100, 200, 300),
                    "shape_to_body": {"translation": point(0, 0, 0), "scale": point(1, 1, 1),
                                      "rotation": {"x": 0, "y": 0, "z": 0, "w": 1}}}
        sample = {"input": {"mesh_asset_path": "/ALS/Mesh", "mesh_shape_index": 0, "mesh_scale": [1, 1, 1]},
                  "other_geometry": geometry}
        report = {"physics_contact_probe": {"samples": [sample]}}
        self.assertEqual(module.convert(report)["Mesh"], [[-1, -3, -2], [1, 3, 2]])
        invalid = copy.deepcopy(report)
        invalid["physics_contact_probe"]["samples"][0]["input"]["mesh_scale"] = [2, 1, 1]
        with self.assertRaises(ValueError):
            module.convert(invalid)
        invalid = copy.deepcopy(report)
        invalid["physics_contact_probe"]["samples"].append(sample)
        with self.assertRaises(ValueError):
            module.convert(invalid)


if __name__ == "__main__":
    unittest.main()
