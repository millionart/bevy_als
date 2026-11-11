import gzip
import json
import sys
import unittest
from pathlib import Path

root = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(root / 'tools' / 'import'))
from import_mesh_contact_settings import extract


class MeshContactSettingsTest(unittest.TestCase):
    def test_native_scalars_and_ordering_mode(self):
        audit = root / 'docs' / 'audit' / 'bf144'
        facts = json.loads((audit / 'd100b4b3a' / 'source-facts.json').read_text())
        native = gzip.decompress((audit / 'd100b' / 'mesh.native.json.gz').read_bytes())
        physics = json.loads(native)['physics_asset']
        target = (root / 'src' / 'als' / 'settings' / 'mesh_contact.rs').read_text()
        self.assertEqual(extract(facts, physics), target)
        settings = physics['constraint_console_variables']
        settings['p.Chaos.Collision.ConvexTriMeshSortByPhi'] = 'true'
        with self.assertRaises(ValueError):
            extract(facts, physics)


if __name__ == '__main__':
    unittest.main()
