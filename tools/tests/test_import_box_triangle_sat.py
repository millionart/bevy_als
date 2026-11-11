import json
import sys
import unittest
from pathlib import Path

root = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(root / 'tools' / 'import'))
from import_box_triangle_sat import sat_data


class BoxTriangleSatImportTest(unittest.TestCase):
    def test_source_topology_and_tolerances(self):
        path = root / 'docs' / 'audit' / 'bf144' / 'd100b4b2b1' / 'source-facts.json'
        facts = json.loads(path.read_text())
        expected = (root / 'src' / 'avian_ext' / 'source_mesh_box_sat_data.rs').read_text()
        self.assertEqual(sat_data(facts), expected)
        reversed_face = [{**row, 'text': row['text'].replace('{ 0, 4, 6, 2 }', '{ 0, 2, 6, 4 }')}
                         for row in facts]
        with self.assertRaises(ValueError):
            sat_data(reversed_face)
        without_bias = [{**row, 'text': row['text'].replace('const FReal TriFaceBias', 'const FReal OtherBias')}
                        for row in facts]
        with self.assertRaises(ValueError):
            sat_data(without_bias)


if __name__ == '__main__':
    unittest.main()
