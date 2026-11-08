import importlib.util
from pathlib import Path
import sys
import unittest

directory = Path(__file__).resolve().parents[1] / "import"
sys.path.insert(0, str(directory))
spec = importlib.util.spec_from_file_location("import_contact_face_classification", directory / "import_contact_face_classification.py")
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
export_spec = importlib.util.spec_from_file_location("export_convex_classification_probe", directory / "export_convex_classification_probe.py")
exporter = importlib.util.module_from_spec(export_spec)
export_spec.loader.exec_module(exporter)


class ContactFaceClassificationTest(unittest.TestCase):
    def test_native_override_and_source_default(self):
        facts = [{"text": "Chaos_Collision_Manifold_PlaneContactNormalEpsilon = 0.001f;\n"
                          "SmallBiasToPreventFeatureFlipping = 0.002f;"}]
        native = {"physics_asset": {"constraint_console_variables": {}}}
        self.assertEqual(module.settings(facts, native), (module.f32(0.001), module.f32(0.002)))
        native["physics_asset"]["constraint_console_variables"]["p.Chaos.Collision.Manifold.PlaneContactNormalEpsilon"] = "0.004"
        self.assertEqual(module.settings(facts, native)[0], module.f32(0.004))
        native["physics_asset"]["constraint_console_variables"]["p.Chaos.Collision.Manifold.PlaneContactNormalEpsilon"] = -1
        with self.assertRaises(ValueError):
            module.settings(facts, native)

    def test_export_retains_source_geometry_and_contact_order(self):
        keys = ("input", "geometry", "other_geometry", "points", "gjk_simplex", "collision_margin_0_cm", "collision_margin_1_cm")
        sample = {key: [] for key in keys}
        sample["points"] = [{"contact_type": 3}, {"contact_type": 1}]
        sample["unused"] = True
        result = exporter.convert({"physics_contact_probe": {"samples": [sample]}})["samples"][0]
        self.assertEqual(result["points"], [{"contact_type": 3}, {"contact_type": 1}])
        self.assertNotIn("unused", result)


if __name__ == "__main__":
    unittest.main()
