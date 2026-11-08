import importlib.util
from pathlib import Path
import sys
import unittest

directory = Path(__file__).resolve().parents[1] / "import"
sys.path.insert(0, str(directory))
spec = importlib.util.spec_from_file_location("import_contact_restore_settings", directory / "import_contact_restore_settings.py")
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
probe_spec = importlib.util.spec_from_file_location("export_contact_restore_probe", directory / "export_contact_restore_probe.py")
probe = importlib.util.module_from_spec(probe_spec)
probe_spec.loader.exec_module(probe)


class ContactRestoreSettingsTest(unittest.TestCase):
    def test_source_thresholds_and_missing_field(self):
        source = """
ContactPositionToleranceScale = FReal(0.8);
ShapePositionToleranceScale0 = FReal(0.5);
ShapePositionToleranceScaleN = FReal(0.2);
ShapeRotationThreshold0 = FReal(0.9998);
ShapeRotationThresholdN = FReal(0.9999);
const FRealSingle ToleranceScale = 0.1f;
ExpectedNumManifoldPoints = FMath::Min(ManifoldPoints.Num(), 4);
"""
        values, scale, count = module.settings([{"text": source}])
        self.assertEqual(values, [0.8, 0.5, 0.2, 0.9998, 0.9999])
        self.assertEqual((scale, count), (0.1, 4))
        with self.assertRaises(ValueError):
            module.settings([{"text": source.replace("ShapeRotationThresholdN", "Missing")}])

    def test_probe_retains_two_frame_contact_identity(self):
        sample = {"input": {"label": "sample"}, "geometry": {"kind": "box"}, "points": [1],
                  "next_points": [2], "restore_allowed": True, "restored": True,
                  "other_geometry": {"kind": "convex", "bounds_min_cm": [0], "bounds_max_cm": [1],
                                     "shape_to_body": {}, "convex_core_vertices_cm": [3]}}
        exported = probe.samples([{"physics_contact_probe": {"samples": [sample]}}])["samples"][0]
        self.assertEqual((exported["points"], exported["next_points"]), ([1], [2]))
        self.assertNotIn("convex_core_vertices_cm", exported["other_geometry"])


if __name__ == "__main__":
    unittest.main()
