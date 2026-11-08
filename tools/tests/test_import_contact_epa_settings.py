import importlib.util
import math
from pathlib import Path
import sys
import unittest

directory = Path(__file__).resolve().parents[1] / "import"
sys.path.insert(0, str(directory))
spec = importlib.util.spec_from_file_location("import_contact_epa_settings", directory / "import_contact_epa_settings.py")
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class ContactEpaSettingsTest(unittest.TestCase):
    def test_source_units_and_limits(self):
        facts = [{"text": """
const T Eps = TEPAEntry::SelectEpsilon(1.e-4f * 1.e-4f, 1.e-8 * 1.e-8 * 1.e-8 * 1.e-8);
if (Dir.SizeSquared() > 1e-4) {}
if (Entry.DistanceToPlane(W) <= TEPAEntry<T>::SelectEpsilon(1.e-4f, 1.e-8)) {}
if (OutPenetration < 1e-4) {}
const int32 MaxIteration = 10000;
int32 constexpr MaxIterations = 128;
"""}]
        native = {"physics_asset": {"constraint_console_variables": {"p.Chaos.Collision.EPAEpsilon": "1e-6"}}}
        area, initial_edge, visibility, witness, maximum, flood, relative = module.settings(facts, native)
        self.assertTrue(math.isclose(area, 1e-40, rel_tol=1e-12))
        self.assertTrue(math.isclose(initial_edge, 1e-8, rel_tol=1e-12))
        self.assertTrue(math.isclose(visibility, 1e-10, rel_tol=1e-12))
        self.assertTrue(math.isclose(witness, 1e-6, rel_tol=1e-12))
        self.assertEqual((maximum, flood), (128, 10000))
        self.assertAlmostEqual(relative, 1e-6)


if __name__ == "__main__":
    unittest.main()
