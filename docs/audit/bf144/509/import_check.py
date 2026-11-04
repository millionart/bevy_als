import copy
import json
import re
import sys
from pathlib import Path
from unittest.mock import patch

root = Path(__file__).resolve().parent
sys.path.insert(0, str(Path('tools/import').resolve()))
import gen_in_air_settings as importer

assets = {}
for label in ('CF_Als_LeanAmount_Air', 'CF_Als_GroundPredictionAmount'):
    report = json.loads((root/f'{label}.native.json.reader.report.json').read_bytes())
    assert report['success']
    obj = json.loads((root/f'{label}.native.json').read_bytes())
    assets[obj['asset_path']] = obj
settings = json.loads((root.parent/'508/settings.native.json').read_bytes())
env = json.loads(Path('tools/env.local.json').read_bytes())
source = Path(env['ue_project']).parent/'Plugins/ALS-Refactored/Source/ALS/Private/AlsAnimationInstance.cpp'
before = json.dumps(settings, sort_keys=True)
runs = {}
for value in (0.2, 0.0, 0.6):
    variant = copy.deepcopy(settings)
    general = next(row for row in variant['asset_properties'] if row['name'] == 'General')
    general['exported_text'], count = re.subn(r'LeanInterpolationHalfLife=[0-9.]+',
        'LeanInterpolationHalfLife=' + str(value), general['exported_text'])
    assert count == 1
    with patch.object(importer, 'native_source_asset', side_effect=lambda key, _: assets[key]):
        output, references = importer.generate(variant, None, source.read_text())
    assert output['lean_interpolation_half_life'] == value
    runs[str(value)] = output
    print(f'Converted exact source field {value}', flush=True)
assert json.dumps(settings, sort_keys=True) == before
generated = json.loads(Path('assets/ALS/Data/AnimationInstance/Air/als_in_air_settings.json').read_bytes())
assert runs['0.2'] == generated
for variant in runs.values():
    assert {k:v for k,v in variant.items() if k != 'lean_interpolation_half_life'} == {
        k:v for k,v in generated.items() if k != 'lean_interpolation_half_life'}
(root/'import-check.json').write_text(json.dumps(dict(references=references, default_full_asset_exact=True,
    configured_values={key:value['lean_interpolation_half_life'] for key,value in runs.items()}),indent=2)+'\n')
