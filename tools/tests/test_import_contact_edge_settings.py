import importlib.util
from pathlib import Path
import unittest

path = Path(__file__).resolve().parents[1] / "import" / "import_contact_edge_settings.py"
spec = importlib.util.spec_from_file_location("import_contact_edge_settings", path)
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class ContactEdgeSettingsTest(unittest.TestCase):
    def test_native_override_and_source_default(self):
        facts = [{"text": "bool ForceOneShotManifoldEdgeEdgeCaseZeroCullDistance = false;"}]
        native = {"physics_asset": {"constraint_console_variables": {}}}
        self.assertFalse(module.settings(facts, native))
        key = "p.Chaos.Collision.Manifold.ForceOneShotManifoldEdgeEdgeCaseZeroCullDistance"
        native["physics_asset"]["constraint_console_variables"][key] = "true"
        self.assertTrue(module.settings(facts, native))
        native["physics_asset"]["constraint_console_variables"][key] = "unexpected"
        with self.assertRaises(ValueError):
            module.settings(facts, native)


if __name__ == "__main__":
    unittest.main()
