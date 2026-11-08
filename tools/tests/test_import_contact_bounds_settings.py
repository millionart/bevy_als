import importlib.util
from pathlib import Path
import unittest

path = Path(__file__).resolve().parents[1] / "import" / "import_contact_bounds_settings.py"
spec = importlib.util.spec_from_file_location("import_contact_bounds_settings", path)
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class ContactBoundsSettingsTest(unittest.TestCase):
    def test_native_override_and_source_default(self):
        facts = [{"text": "bool bChaos_Collision_EnableBoundsChecks = true;"}]
        native = {"physics_asset": {"constraint_console_variables": {}}}
        self.assertTrue(module.settings(facts, native))
        native["physics_asset"]["constraint_console_variables"]["p.Chaos.Collision.EnableBoundsChecks"] = "false"
        self.assertFalse(module.settings(facts, native))
        native["physics_asset"]["constraint_console_variables"]["p.Chaos.Collision.EnableBoundsChecks"] = "invalid"
        with self.assertRaises(ValueError):
            module.settings(facts, native)


if __name__ == "__main__":
    unittest.main()
