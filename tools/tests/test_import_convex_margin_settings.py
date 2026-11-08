import copy
import importlib.util
from pathlib import Path
import sys
import unittest

directory = Path(__file__).resolve().parents[1] / "import"
sys.path.insert(0, str(directory))
spec = importlib.util.spec_from_file_location("import_convex_margin_settings", directory / "import_convex_margin_settings.py")
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class ConvexMarginSettingsTest(unittest.TestCase):
    def test_source_settings_override_and_native_margin(self):
        facts = [{"text": "float Chaos_Collision_MarginFraction = -1.0f; float Chaos_Collision_MarginMax = -1.0f; "
                           "FRealSingle Chaos_Collision_ConvexZeroMargin = 0.0f;"}]
        settings = {"asset_properties": [{"name": "SolverOptions", "fields": [
            {"name": "CollisionMarginFraction", "numeric_value": 0.05},
            {"name": "CollisionMarginMax", "numeric_value": 10.0}]}]}
        point = lambda x, y, z: {"x": x, "y": y, "z": z}
        sample = {"geometry": {"kind": "box", "minimum_cm": point(-6, -14, -6),
                               "maximum_cm": point(6, 14, 6)},
                  "collision_margin_0_cm": 0.6, "collision_margin_1_cm": 0}
        report = {"physics_asset": {"constraint_console_variables": {}},
                  "physics_contact_probe": {"samples": [sample]}}
        self.assertEqual(module.parameters(facts, settings, [report]), (0.05, 0.1, 0.0))
        changed = copy.deepcopy(report)
        changed["physics_asset"]["constraint_console_variables"]["p.Chaos.Collision.MarginFraction"] = "0.1"
        changed["physics_contact_probe"]["samples"][0]["collision_margin_0_cm"] = 1.2
        self.assertEqual(module.parameters(facts, settings, [changed]), (0.1, 0.1, 0.0))
        with self.assertRaises(ValueError):
            module.parameters(facts, settings, [report, changed])


if __name__ == "__main__":
    unittest.main()
