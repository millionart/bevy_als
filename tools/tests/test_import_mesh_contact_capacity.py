import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'import'))
from import_mesh_contact_capacity import capacity


class MeshContactCapacityTest(unittest.TestCase):
    def test_preserves_quantized_capacity_and_rejects_wrong_request(self):
        facts = [{'text': 'MaxContactsBufferSize = 1000;'}]
        sample = {'input': {'mesh_contact_buffer_request': 1000}, 'mesh_contact_buffer_capacity': 1016}
        report = {'physics_contact_probe': {'samples': [sample]}}
        self.assertEqual(capacity(facts, report), 1016)
        sample['input']['mesh_contact_buffer_request'] = 999
        with self.assertRaises(ValueError):
            capacity(facts, report)
        sample['input']['mesh_contact_buffer_request'] = 1000
        sample['mesh_contact_buffer_capacity'] = 999
        with self.assertRaises(ValueError):
            capacity(facts, report)


if __name__ == '__main__':
    unittest.main()
