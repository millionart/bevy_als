import importlib.util
import json
from pathlib import Path
import sys
import unittest

directory = Path(__file__).resolve().parents[1] / "import"
sys.path.insert(0, str(directory))
spec = importlib.util.spec_from_file_location("import_contact_face_search", directory / "import_contact_face_search.py")
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
box_spec = importlib.util.spec_from_file_location("import_box_contact_adjacency", directory / "import_box_contact_adjacency.py")
box = importlib.util.module_from_spec(box_spec)
box_spec.loader.exec_module(box)


class ContactFaceSearchTest(unittest.TestCase):
    def test_default_and_native_search_distance(self):
        facts = [{"text": "FRealSingle Chaos_Collision_Manifold_MinFaceSearchDistance = 1.0f;"}]
        native = {"physics_asset": {"constraint_console_variables": {}}}
        self.assertEqual(module.search_distance(facts, native), 0.01)
        native["physics_asset"]["constraint_console_variables"]["p.Chaos.Collision.Manifold.MinFaceSearchDistance"] = "2"
        self.assertEqual(module.search_distance(facts, native), 0.02)

    def test_source_box_adjacency(self):
        facts = json.loads((directory.parent.parent / "docs/audit/bf144/d100b3b2/source-facts.json").read_text())
        self.assertEqual(box.adjacency(facts)[0], [0, 1, 2])
        self.assertEqual(box.adjacency(facts)[1], [1, 3, 2])


if __name__ == "__main__":
    unittest.main()
