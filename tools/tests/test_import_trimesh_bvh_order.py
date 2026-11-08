import copy
import importlib.util
import sys
import unittest
from pathlib import Path

directory = Path(__file__).resolve().parents[1] / "import"
sys.path.insert(0, str(directory))
spec = importlib.util.spec_from_file_location("import_trimesh_bvh_order", directory / "import_trimesh_bvh_order.py")
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class TrimeshBvhOrderTest(unittest.TestCase):
    def sample(self):
        return {"input": {"mesh_asset_path": "/ALS/Mesh", "mesh_shape_index": 0, "mesh_scale": [1, 1, 1]},
                "other_geometry": {"kind": "trimesh", "shape_index": 0,
                    "trimesh_core_vertices_cm": [{"x": 1, "y": 2, "z": 3}, {"x": 4, "y": 5, "z": 6},
                                                  {"x": 7, "y": 8, "z": 9}],
                    "trimesh_core_triangles": [[0, 1, 2], [2, 1, 0], [1, 2, 0]],
                    "trimesh_core_bvh_face_order": [2, 0, 1],
                    "trimesh_core_bvh_face_bounds_cm": [
                        {"min": {"x": 0, "y": 1, "z": 2}, "max": {"x": 8, "y": 9, "z": 10}},
                        {"min": {"x": 0, "y": 1, "z": 2}, "max": {"x": 8, "y": 9, "z": 10}},
                        {"min": {"x": 0, "y": 1, "z": 2}, "max": {"x": 8, "y": 9, "z": 10}}],
                    "trimesh_core_bvh_nodes": [
                        [{"index": 1, "face_count": 0, "min": {"x": 0, "y": 1, "z": 2}, "max": {"x": 8, "y": 9, "z": 10}},
                         {"index": 2, "face_count": 1, "min": {"x": 0, "y": 1, "z": 2}, "max": {"x": 8, "y": 9, "z": 10}}],
                        [{"index": 0, "face_count": 1, "min": {"x": 0, "y": 1, "z": 2}, "max": {"x": 8, "y": 9, "z": 10}},
                         {"index": 1, "face_count": 1, "min": {"x": 0, "y": 1, "z": 2}, "max": {"x": 8, "y": 9, "z": 10}}]]}}

    def topology(self):
        return {"Mesh": {"vertices_m": [[0.01, 0.03, 0.02], [0.04, 0.06, 0.05], [0.07, 0.09, 0.08]],
                         "triangles": [[0, 1, 2], [2, 1, 0], [1, 2, 0]], "winding": -1}}

    def test_preserves_native_order_across_scale_samples(self):
        first = self.sample()
        second = copy.deepcopy(first)
        second["input"]["mesh_scale"] = [-1, 2, 1]
        reports = [{"physics_contact_probe": {"samples": [first, second]}}]
        expected_bounds = [[[0.0, 0.02, 0.01], [0.08, 0.1, 0.09]]] * 3
        bound = {"min_m": [0.0, 0.02, 0.01], "max_m": [0.08, 0.1, 0.09]}
        nodes = [[{"index": 1, "face_count": 0, **bound}, {"index": 2, "face_count": 1, **bound}],
                 [{"index": 0, "face_count": 1, **bound}, {"index": 1, "face_count": 1, **bound}]]
        self.assertEqual(module.convert(reports, self.topology()),
                         {"Mesh": {"face_order": [2, 0, 1], "face_bounds_m": expected_bounds, "nodes": nodes}})

    def test_rejects_missing_invalid_and_inconsistent_orders(self):
        first = self.sample()
        for order in (None, [2, 0, 0], [2, 0, 3], [2, 0, True]):
            sample = copy.deepcopy(first)
            if order is None:
                del sample["other_geometry"]["trimesh_core_bvh_face_order"]
            else:
                sample["other_geometry"]["trimesh_core_bvh_face_order"] = order
            with self.assertRaises(ValueError):
                module.convert([{"physics_contact_probe": {"samples": [sample]}}], self.topology())
        second = copy.deepcopy(first)
        second["other_geometry"]["trimesh_core_bvh_face_order"] = [1, 0, 2]
        with self.assertRaises(ValueError):
            module.convert([{"physics_contact_probe": {"samples": [first, second]}}], self.topology())
        second = copy.deepcopy(first)
        del second["other_geometry"]["trimesh_core_bvh_face_bounds_cm"]
        with self.assertRaises(ValueError):
            module.convert([{"physics_contact_probe": {"samples": [second]}}], self.topology())
        second = copy.deepcopy(first)
        del second["other_geometry"]["trimesh_core_bvh_nodes"]
        with self.assertRaises(ValueError):
            module.convert([{"physics_contact_probe": {"samples": [second]}}], self.topology())
        second = copy.deepcopy(first)
        second["other_geometry"]["trimesh_core_bvh_face_bounds_cm"][0]["min"]["x"] = 9
        with self.assertRaises(ValueError):
            module.convert([{"physics_contact_probe": {"samples": [second]}}], self.topology())
        second = copy.deepcopy(first)
        second["other_geometry"]["trimesh_core_bvh_face_bounds_cm"][0]["max"]["x"] = 9
        with self.assertRaises(ValueError):
            module.convert([{"physics_contact_probe": {"samples": [first, second]}}], self.topology())
        second = copy.deepcopy(first)
        second["other_geometry"]["trimesh_core_bvh_nodes"][0][0]["index"] = 0
        with self.assertRaises(ValueError):
            module.convert([{"physics_contact_probe": {"samples": [second]}}], self.topology())
        second = copy.deepcopy(first)
        second["other_geometry"]["trimesh_core_bvh_nodes"][1][1]["index"] = 3
        with self.assertRaises(ValueError):
            module.convert([{"physics_contact_probe": {"samples": [second]}}], self.topology())
        first["other_geometry"]["trimesh_core_triangles"][0] = [0, 2, 1]
        with self.assertRaises(ValueError):
            module.convert([{"physics_contact_probe": {"samples": [first]}}], self.topology())


if __name__ == "__main__":
    unittest.main()
