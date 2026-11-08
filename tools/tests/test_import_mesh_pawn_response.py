import importlib.util
from pathlib import Path
import unittest


spec = importlib.util.spec_from_file_location(
    'import_mesh_pawn_response', Path(__file__).resolve().parents[1] / 'import' / 'import_mesh_pawn_response.py')
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


def actor(response):
    return {'blueprint_cdo_subobjects': [{'object_path': '/Fixture:CharacterMesh0', 'properties': [
        {'name': 'BodyInstance', 'fields': [{'name': 'CollisionResponses', 'fields': [
            {'name': 'ResponseToChannels', 'fields': [{'name': 'Pawn', 'exported_text': response}]}]}]}]}]}


class PawnResponseTest(unittest.TestCase):
    def test_source_response_and_unsupported_overlap(self):
        self.assertTrue(module.pawn_ignored(actor('ECR_Ignore')))
        self.assertFalse(module.pawn_ignored(actor('ECR_Block')))
        with self.assertRaises(ValueError):
            module.pawn_ignored(actor('ECR_Overlap'))


if __name__ == '__main__':
    unittest.main()
