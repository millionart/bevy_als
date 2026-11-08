import copy
import importlib.util
import sys
import unittest
from pathlib import Path

directory = Path(__file__).resolve().parents[1] / "import"
sys.path.insert(0, str(directory))
spec = importlib.util.spec_from_file_location("import_trimesh_contact_topology", directory / "import_trimesh_contact_topology.py")
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class TrimeshContactTopologyTest(unittest.TestCase):
    def sample(self):
        vector = lambda x, y, z: {"x": x, "y": y, "z": z}
        return {"input": {"mesh_asset_path": "/ALS/Mesh", "mesh_shape_index": 0, "mesh_scale": [1, 1, 1]},
                "other_geometry": {"kind": "trimesh", "shape_index": 0, "trimesh_core_scale": vector(1, 1, 1),
                    "shape_to_body": {"translation": vector(0, 0, 0), "scale": vector(1, 1, 1),
                                      "rotation": {"x": 0, "y": 0, "z": 0, "w": 1}},
                    "trimesh_core_vertices_cm": [vector(1, 2, 3), vector(4, 2, 3), vector(1, 5, 3)],
                    "trimesh_core_triangles": [[2, 0, 1]]}}

    def test_preserves_source_face_indices_and_reflected_basis(self):
        result = module.convert({"physics_contact_probe": {"samples": [self.sample()]}})
        self.assertEqual(result["Mesh"]["vertices_m"][0], [0.01, 0.03, 0.02])
        self.assertEqual(result["Mesh"]["triangles"], [[2, 0, 1]])
        self.assertEqual(result["Mesh"]["winding"], -1)
        vertices = result["Mesh"]["vertices_m"]
        a, b, c = (vertices[index] for index in result["Mesh"]["triangles"][0])
        normal_y = (b[2] - a[2]) * (c[0] - a[0]) - (b[0] - a[0]) * (c[2] - a[2])
        self.assertLess(normal_y, 0.0)

    def test_rejects_duplicate_scaled_and_invalid_source(self):
        sample = self.sample()
        with self.assertRaises(ValueError):
            module.convert({"physics_contact_probe": {"samples": [sample, copy.deepcopy(sample)]}})
        sample["other_geometry"]["trimesh_core_triangles"] = [[0, 1, 3]]
        with self.assertRaises(ValueError):
            module.convert({"physics_contact_probe": {"samples": [sample]}})
        sample = self.sample()
        sample["other_geometry"]["trimesh_core_triangles"] = [[0, 1, True]]
        with self.assertRaises(ValueError):
            module.convert({"physics_contact_probe": {"samples": [sample]}})
        sample = self.sample()
        sample["input"]["mesh_scale"] = [1, 2, 1]
        with self.assertRaises(ValueError):
            module.convert({"physics_contact_probe": {"samples": [sample]}})


if __name__ == "__main__":
    unittest.main()
