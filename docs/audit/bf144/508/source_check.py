import gzip
import hashlib
import json
from pathlib import Path

root = Path(__file__).resolve().parent
binding = json.loads(gzip.decompress((root.parent / '506/ab_als.native.json.gz').read_bytes()))
settings = json.loads((root / 'settings.native.json').read_bytes())
report = json.loads((root / 'settings.native.json.reader.report.json').read_bytes())
assert report['success']
link = next(p for p in binding['blueprint_cdo_properties'] if p['name'] == 'Settings')
assert link['exported_text'].split("'")[1] == settings['asset_path']
grounded = next(p for p in settings['asset_properties'] if p['name'] == 'Grounded')
half_life = next(p for p in grounded['fields'] if p['name'] == 'VelocityBlendInterpolationHalfLife')
assert half_life['property_class'] == 'FloatProperty' and half_life['numeric_value'] == 0.10000000149011612

env = json.loads(Path('tools/env.local.json').read_bytes())
source = Path(env['ue_project']).parent / 'Plugins/ALS-Refactored/Source/ALS'
paths = {
    'source:Private/AlsAnimationInstance.cpp': source / 'Private/AlsAnimationInstance.cpp',
    'source:Public/Settings/AlsGroundedSettings.h': source / 'Public/Settings/AlsGroundedSettings.h',
    'src/als/character/animation_instances/source_native_refresh.rs': Path('src/als/character/animation_instances/source_native_refresh.rs'),
    'tools/import/gen_standing_curves.py': Path('tools/import/gen_standing_curves.py'),
}
facts = {}
for key, path in paths.items():
    body = path.read_bytes()
    facts[key] = hashlib.sha256(body).hexdigest()
    if key.startswith('source:Private/'):
        assert 'Settings->Grounded.VelocityBlendInterpolationHalfLife <= 0.0f' in body.decode()
    elif key.startswith('source:Public/'):
        assert 'float VelocityBlendInterpolationHalfLife{0.1f};' in body.decode()
    elif key.startswith('src/'):
        refresh = body.decode().split('fn refresh_grounded(', 1)[1].split('fn refresh_standing(', 1)[0]
        assert 'alpha(0.1)' in refresh and 'half_life' not in refresh.split('let velocity_alpha', 1)[1].split('let target_lean', 1)[0]
    else:
        assert "grounded = parse_struct(asset_property(settings, 'Grounded'))" in body.decode()
        assert 'VelocityBlendInterpolationHalfLife' not in body.decode()

result = dict(gate='PASS_ACTUAL_SOURCE_DEFAULT_AND_CONFIGURATION_ROUTE_STATIC', asset=settings['asset_path'],
              half_life=half_life['numeric_value'], target_refresh_uses_literal=0.1,
              native_variant_gate='pending', files=facts)
(root / 'source-check.json').write_text(json.dumps(result, indent=2) + '\n')
print('Verified active native settings asset and target constant', flush=True)
