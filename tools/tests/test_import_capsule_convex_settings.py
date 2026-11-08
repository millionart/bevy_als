import importlib.util
import sys
import unittest
from pathlib import Path

directory = Path(__file__).resolve().parents[1] / "import"
sys.path.insert(0, str(directory))
spec = importlib.util.spec_from_file_location("import_capsule_convex_settings", directory / "import_capsule_convex_settings.py")
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class CapsuleConvexSettingsTest(unittest.TestCase):
    def test_source_defaults_and_native_override(self):
        facts = [{"text": "Chaos_Collision_Manifold_CapsuleMinContactDistanceFraction = 0.1f;\n"
                          "const FReal CapsuleEndCapAxisNormalThreshold = FReal(0.01);\n"
                          "const FReal CapsuleCylinderTolerance = FReal(0.707);\n"
                          "const FReal PlaneTolerance = FReal(1e-2);\n"
                          "const FReal PlaneTolerance = FReal(1e-2);"}]
        native = {"physics_asset": {"constraint_console_variables": {}}}
        self.assertEqual(module.settings(facts, native), (0.01, 0.707, 0.01, module.f32(0.1)))
        native["physics_asset"]["constraint_console_variables"]["p.Chaos.Collision.Manifold.CapsuleMinContactDistanceFraction"] = "0.2"
        self.assertEqual(module.settings(facts, native)[3], module.f32(0.2))
        native["physics_asset"]["constraint_console_variables"]["p.Chaos.Collision.Manifold.CapsuleMinContactDistanceFraction"] = "0"
        self.assertEqual(module.settings(facts, native)[3], 0.0)
        native["physics_asset"]["constraint_console_variables"]["p.Chaos.Collision.Manifold.CapsuleMinContactDistanceFraction"] = "-0.1"
        self.assertEqual(module.settings(facts, native)[3], module.f32(-0.1))
        native["physics_asset"]["constraint_console_variables"]["p.Chaos.Collision.Manifold.CapsuleMinContactDistanceFraction"] = "nan"
        with self.assertRaises(ValueError):
            module.settings(facts, native)
        facts[0]["text"] = facts[0]["text"].replace("FReal(1e-2);\n", "FReal(2e-2);\n", 1)
        with self.assertRaises(ValueError):
            module.settings(facts, {"physics_asset": {"constraint_console_variables": {}}})


if __name__ == "__main__":
    unittest.main()
