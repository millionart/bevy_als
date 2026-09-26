import contextlib
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import gen_curves
import gen_ron


class AssetImportLayoutTests(unittest.TestCase):
    def test_curve_tables_stay_with_their_source_module(self):
        with tempfile.TemporaryDirectory() as directory:
            assets = Path(directory) / 'assets'
            curve = assets / 'ALSCamera/Data/CF_Test.json'
            curve.parent.mkdir(parents=True)
            curve.write_text(json.dumps({'sampling': {'mode': 'uniform'}, 'keys': [{'t': 0, 'v': 1}]}))
            other = assets / 'actors.json'
            other.write_text('[]')
            rich = assets / 'ALSCamera/Data/CF_Test.rich.json'
            rich.write_text('{"keys": [{"time": 0, "value": 1}]}')
            with patch.object(gen_curves, 'CURVES', assets), contextlib.redirect_stdout(io.StringIO()):
                self.assertEqual(gen_curves.main(), 0)
            self.assertEqual(curve.with_suffix('.curve.ron').read_text(), '(mode: Linear, keys: [(t: 0.0, v: 1.0)])\n')
            self.assertFalse(other.with_suffix('.curve.ron').exists())
            self.assertFalse(rich.with_suffix('.curve.ron').exists())

    def test_clip_import_includes_overlay_objects_and_skips_static_meshes(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            assets = root / 'assets'
            clips = ['ALS/Animations/Base/A_Test', 'ALS/OverlayObjects/Bow/A_Test_Bow']
            for clip in clips:
                path = assets / (clip + '.gltf')
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(json.dumps({'scenes': [{'nodes': [0]}], 'nodes': [{}], 'animations': [{
                    'name': path.stem + '_0', 'samplers': [{}], 'channels': [{'sampler': 0, 'target': {'node': 0}}],
                }]}))
            mesh = assets / 'ALS/Character/SM_Static.gltf'
            mesh.parent.mkdir(parents=True, exist_ok=True)
            mesh.write_text('{"scenes": [{"nodes": [0]}], "nodes": [{}]}')
            with patch.multiple(gen_ron, REPO=root, ASSETS=assets, MODELS=assets, ANIMS=assets/'ALS', SKELS=assets/'ALS/Character',
                                SKELETON_SOURCES={'SKM_Als': (clips[0] + '.gltf', 'Scene0')}), contextlib.redirect_stdout(io.StringIO()):
                self.assertEqual(gen_ron.main(), 0)
            self.assertIn('ALS/Character/SKM_Als.skn.ron', (assets / (clips[0] + '.anim.ron')).read_text())
            self.assertIn('ALS/OverlayObjects/Bow/SKM_Als_Bow.skn.ron', (assets / (clips[1] + '.anim.ron')).read_text())
            self.assertFalse(mesh.with_suffix('.anim.ron').exists())
            self.assertFalse((assets / 'skeletons').exists())


if __name__ == '__main__':
    unittest.main()
