import argparse
import json
from pathlib import Path

from PIL import Image

from generate_als_notify_manifest import manifest
from import_camera_component import record
from ue_animation_asset_resolver import asset_property


def source_contract(directory):
    sources = {}
    for path in directory.glob('*.ue.json'):
        value = json.loads(path.read_text(encoding='utf-8-sig'))
        if value.get('asset_class', '').endswith('.AlsFootstepEffectsSettings') and not record(value['asset_properties'], 'Effects').get('elements'):
            continue
        if value.get('asset_class', '').endswith('.Texture2D') and 'source_image' not in value:
            continue
        sources[value['asset_path'].rsplit('.', 1)[-1]] = value
    settings = sources['FES_Als_Default']
    effects = record(settings['asset_properties'], 'Effects')['elements']
    assert len(effects) == 1 and effects[0]['key'] == 'SurfaceType_Default'
    decal = record(effects[0]['fields'], 'Decal')['fields']
    assert record(decal, 'Duration')['exported_text'] == '4.000000'
    assert record(decal, 'FadeOutDuration')['exported_text'] == '2.000000'
    assert record(decal, 'SpawnMode')['exported_text'] == 'SpawnAttachedToTraceHitComponent'
    defaults = sources['AlsAnimNotify_FootstepEffects']['class_default_properties']
    assert record(defaults, 'bSpawnDecal')['exported_text'] == 'True'
    assert record(defaults, 'bSkipEffectsWhenInAir')['exported_text'] == 'False'
    texture = sources['T_Als_Footprint_M']
    image = Image.open(directory / texture['source_image'])
    assert image.mode == 'RGBA' and image.size == (texture['source_width'], texture['source_height'])
    assert image.size == (512, 512) and image.getextrema()[3] == (0, 255)
    assert asset_property(texture, 'SRGB') == 'False'
    material = sources['M_Als_Footprint_Decal']
    assert asset_property(material, 'MaterialDomain') == 'MD_DeferredDecal'
    assert asset_property(material, 'BlendMode') == 'BLEND_Translucent'
    assert asset_property(sources['BS_Als_WalkRun_Forward'], 'NotifyTriggerMode') == 'HighestWeightedAnimation'
    audit = json.loads((directory / 'notifies.json').read_text())
    assert audit['summary']['failure_count'] == 0
    assert audit['summary']['event_classes']['AlsAnimNotify_FootstepEffects'] == 154
    generated = manifest(audit, Path('assets'))
    assert generated == json.loads(Path('assets/animation_events/als_notify_manifest.json').read_text())
    return sources


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--source-directory', required=True, type=Path)
    args = parser.parse_args()
    sources = source_contract(args.source_directory)
    print(json.dumps({'source_assets': len(sources), 'footstep_notifies': 154, 'native_map_defaults': 'PASS', 'native_class_defaults': 'PASS', 'source_texture': 'PASS', 'existing_notify_manifest': 'PASS'}))


if __name__ == '__main__':
    main()
