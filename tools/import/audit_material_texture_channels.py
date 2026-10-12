import argparse
import json
import re
from pathlib import Path

from PIL import Image

from import_camera_component import record
from ue_animation_asset_resolver import source_object_path


def required_channels(material, sample_nodes=None):
    objects = {obj['object_path']: obj for obj in material['asset_objects']}
    editor = next(obj for obj in objects.values() if obj['class_path'].endswith('.MaterialEditorOnlyData'))
    result = {}
    visited = set()

    def visit(obj):
        if obj['object_path'] in visited:
            return
        visited.add(obj['object_path'])
        for prop in obj['properties']:
            if prop['name'] == 'Declaration' and prop['exported_text'] != 'None':
                visit(objects[source_object_path(prop['exported_text'])])
            fields = prop.get('fields', [])
            reference = next((field['exported_text'] for field in fields if field['name'] == 'Expression'), 'None')
            if reference == 'None':
                continue
            node = objects[source_object_path(reference)]
            if node['class_path'].rsplit('.', 1)[-1].startswith('MaterialExpressionTextureSample'):
                texture = source_object_path(record(node['properties'], 'Texture')['exported_text'])
                index = int(record(fields, 'OutputIndex')['exported_text'])
                channels = ('RGB', 'R', 'G', 'B', 'A', 'RGBA')[index]
                if record(fields, 'Mask')['exported_text'] not in ('0', 'False'):
                    channels = ''.join(channel for channel in 'RGBA' if record(fields, 'Mask' + channel)['exported_text'] not in ('0', 'False'))
                result.setdefault(texture, set()).update(channels)
                if sample_nodes is not None:
                    sample_nodes.setdefault(texture,set()).add(node['object_path'])
            visit(node)

    visit(editor)
    return result


def audit(material, texture, image, original, shader, binding):
    sample_nodes = {}
    required = required_channels(material,sample_nodes).get(texture['asset_path'])
    if not required:
        raise ValueError('Source material does not consume this texture')
    actual = set(image.getbands())
    missing = required - actual
    if missing:
        raise ValueError('Exported texture is missing source channels: ' + ''.join(sorted(missing)) + '; check the source exporter')
    if len(image.getbands()) != texture['source_channels']:
        raise ValueError('Exported channel count differs from Texture.Source')
    expected_format = 'PNG' if texture['source_pixel_format'] == 'RGBA16' else {4: 'TGA', 3: 'PNG'}[texture['source_channels']]
    if image.format != expected_format:
        raise ValueError('Texture container does not match its source channel count')
    if not texture.get('source_image_pixel_exact') or image.size != original.size or image.tobytes() != original.tobytes():
        raise ValueError('Runtime texture pixels differ from the verified source export')
    samples = re.findall(r'\blet\s+(\w+)\s*=\s*textureSample(?:Grad|Level|Bias)?\s*\(\s*' + re.escape(binding) + r'\b', shader)
    expected_samples = len(sample_nodes[texture['asset_path']])
    if len(samples) != expected_samples:
        raise ValueError('Shader texture sample count differs from the source material')
    swizzles = ''.join(''.join(re.findall(r'\b' + sample + r'\.([rgbaxyzw]{1,4})\b', shader)) for sample in samples)
    consumed = set(swizzles.translate(str.maketrans('rgbaxyzw', 'RGBARGBA')))
    if required - consumed:
        raise ValueError('Shader omits source texture channels: ' + ''.join(sorted(required - consumed)))
    return {'texture': texture['asset_path'], 'required_channels': ''.join(sorted(required)), 'image_channels': ''.join(image.getbands()), 'shader_channels': ''.join(sorted(consumed)), 'samples': len(samples), 'source_pixel_exact': True}


def self_test(material, texture, image, shader, binding):
    if 'A' in required_channels(material)[texture['asset_path']]:
        try:
            audit(material, texture, image.convert('RGB'), image, shader, binding)
        except ValueError as error:
            assert 'missing source channels: A' in str(error)
        else:
            raise AssertionError('Missing alpha was accepted')
        sample = re.search(r'\blet\s+(\w+)\s*=\s*textureSample(?:Grad|Level|Bias)?\s*\(\s*' + re.escape(binding) + r'\b', shader)[1]
        changed = shader.replace(sample + '.a', sample + '.r')
        try:
            audit(material, texture, image, image, changed, binding)
        except ValueError as error:
            assert 'Shader omits source texture channels: A' in str(error)
        else:
            raise AssertionError('Missing shader alpha consumption was accepted')


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--material-source', type=Path, required=True)
    parser.add_argument('--texture-source', type=Path, required=True)
    parser.add_argument('--image', type=Path, required=True)
    parser.add_argument('--shader', type=Path, required=True)
    parser.add_argument('--binding', required=True)
    parser.add_argument('--self-test', action='store_true')
    args = parser.parse_args()
    material = json.loads(args.material_source.read_text(encoding='utf-8-sig'))
    texture = json.loads(args.texture_source.read_text(encoding='utf-8-sig'))
    image = Image.open(args.image)
    original = Image.open(args.texture_source.parent / texture['source_image'])
    if texture['source_pixel_format']=='RGBA16':
        for path in [args.image,args.texture_source.parent/texture['source_image']]:
            if path.read_bytes()[24:26]!=bytes([16,6]):
                raise ValueError('RGBA16 texture lost its bit depth or alpha')
        if args.image.read_bytes()!=(args.texture_source.parent/texture['source_image']).read_bytes():
            raise ValueError('RGBA16 texture differs from its verified source export')
    shader = args.shader.read_text(encoding='utf-8')
    result = audit(material, texture, image, original, shader, args.binding)
    if args.self_test:
        self_test(material, texture, image, shader, args.binding)
        result['alpha_loss_and_shader_ablation'] = 'PASS' if 'A' in required_channels(material)[texture['asset_path']] else 'NOT_APPLICABLE'
    print(json.dumps(result))


if __name__ == '__main__':
    main()
