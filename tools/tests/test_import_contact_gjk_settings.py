import importlib.util
from pathlib import Path
import sys
import unittest

directory = Path(__file__).resolve().parents[1] / "import"
sys.path.insert(0, str(directory))
spec = importlib.util.spec_from_file_location("import_contact_gjk_settings", directory / "import_contact_gjk_settings.py")
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class ContactGjkSettingsTest(unittest.TestCase):
    def test_source_variant_epsilon_and_iteration_limit(self):
        facts = [{"text": "const int32 MaxIterations = 32;"}]
        variables = {"p.Chaos.Collision.UseGJK2": "false", "p.Chaos.Collision.GJKEpsilon": "1e-6"}
        native = {"physics_asset": {"constraint_console_variables": variables}}
        epsilon, iterations = module.settings(facts, native)
        self.assertAlmostEqual(epsilon, 1e-8)
        self.assertEqual(iterations, 32)
        variables["p.Chaos.Collision.UseGJK2"] = "true"
        with self.assertRaises(ValueError):
            module.settings(facts, native)


if __name__ == "__main__":
    unittest.main()
