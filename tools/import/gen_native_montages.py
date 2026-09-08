import argparse
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'tools/animgraph_authoring'))
from ue_animation_asset_resolver import UAsset, PropReader, source_export_class, source_sequence_curves, source_sequence_timing
from ue_animgraph_pose_renderer import pose_curves_ron


def sequence_record(uasset, content_root, mount, target_assets):
    relative = uasset.resolve().relative_to(content_root.resolve()).with_suffix('')
    source = mount + '/' + relative.as_posix() + '.' + uasset.stem
    asset = UAsset(str(uasset))
    index = next(i for i, export in enumerate(asset.exports)
                 if export.object_name == uasset.stem and source_export_class(asset, export) == 'AnimSequence')
    properties, ok, _ = PropReader(asset).read_export(index)
    if not ok:
        raise ValueError('source sequence properties are unreadable')
    additive = properties.get('AdditiveAnimType', {}).get('value', 'AAT_None')
    if additive not in {'AAT_None', 'AAT_RotationOffsetMeshSpace'}:
        raise ValueError('source montage additive type is unsupported')
    reference = None
    if additive != 'AAT_None':
        if properties.get('RefPoseType', {}).get('value') != 'ABPT_AnimFrame' or properties.get('RefFrameIndex', 0) != 0:
            raise ValueError('source montage reference frame is unsupported')
        reference = str(properties['RefPoseSeq']).split('/')[-1]
    if properties.get('RateScale', 1.0) != 1.0:
        raise ValueError('source montage rate scale is unsupported')

    def target(name):
        matches = [path for path in target_assets if path.name == name + '.anim.ron']
        if len(matches) != 1:
            raise ValueError('source montage target clip is missing or ambiguous: ' + name)
        return matches[0].relative_to(ROOT / 'assets').as_posix()

    timing = source_sequence_timing(content_root, mount, source)
    curves = source_sequence_curves(content_root, mount, source)
    record = {'source': source, 'clip': target(uasset.stem), 'duration': timing['duration'],
              'reference_clip': target(reference) if reference else None}
    quoted = json.dumps
    reference_ron = 'None' if reference is None else 'Some(' + quoted(record['reference_clip']) + ')'
    ron = ('(clip: ' + quoted(record['clip']) + ', duration: ' + repr(record['duration'])
           + ', reference_clip: ' + reference_ron + ', curves: ' + pose_curves_ron(curves) + ')')
    return record, ron


def self_test():
    from types import SimpleNamespace
    from unittest.mock import patch
    source = ROOT / 'target/Fixture.uasset'
    properties = {'AdditiveAnimType': {'value': 'AAT_RotationOffsetMeshSpace'},
                  'RefPoseType': {'value': 'ABPT_AnimFrame'}, 'RefPoseSeq': 'None/Base/Reference'}
    asset = SimpleNamespace(exports=[SimpleNamespace(object_name='Fixture')])
    targets = [ROOT / 'assets/animations/Fixture.anim.ron', ROOT / 'assets/animations/Reference.anim.ron']
    with patch(__name__ + '.UAsset', return_value=asset), \
         patch(__name__ + '.source_export_class', return_value='AnimSequence'), \
         patch(__name__ + '.PropReader', return_value=SimpleNamespace(read_export=lambda index: (properties, True, None))), \
         patch(__name__ + '.source_sequence_timing', return_value={'duration': 1.5}), \
         patch(__name__ + '.source_sequence_curves', return_value=[]):
        record, ron = sequence_record(source, ROOT / 'target', '/Fixture', targets)
        assert record['reference_clip'] == 'animations/Reference.anim.ron' and 'curves: ({})' in ron
        original = json.dumps(properties)
        sequence_record(source, ROOT / 'target', '/Fixture', targets)
        assert json.dumps(properties) == original
        for key, value in [('RefFrameIndex', 1), ('RateScale', 2.0), ('AdditiveAnimType', {'value': 'Unknown'})]:
            with patch.dict(properties, {key: value}):
                try:
                    sequence_record(source, ROOT / 'target', '/Fixture', targets)
                except ValueError:
                    pass
                else:
                    raise AssertionError('unsupported source setting was accepted: ' + key)
    print('Native montage generator self-test PASS', flush=True)


def main():
    if sys.argv[1:] == ['--self-test']:
        self_test()
        return
    parser = argparse.ArgumentParser()
    parser.add_argument('--uasset', type=Path, action='append', required=True)
    parser.add_argument('--content-root', type=Path, required=True)
    parser.add_argument('--mount', default='/ALS')
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--report', type=Path, required=True)
    args = parser.parse_args()
    report = {'format': 'als_native_montages_v1', 'success': False, 'sequences': {}}
    try:
        targets = list((ROOT / 'assets/animations').rglob('*.anim.ron'))
        entries = []
        for uasset in args.uasset:
            record, ron = sequence_record(uasset, args.content_root, args.mount, targets)
            if uasset.stem in report['sequences']:
                raise ValueError('source montage name is duplicated')
            report['sequences'][uasset.stem] = record
            entries.append(json.dumps(uasset.stem) + ': ' + ron)
            print('Converted ' + record['source'], flush=True)
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text('{\n' + ',\n'.join(entries) + '\n}\n', encoding='utf-8')
        report['success'] = True
    except (ValueError, OSError, KeyError, RuntimeError, StopIteration) as error:
        report['error'] = str(error)
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')
    print(json.dumps(report), flush=True)
    if not report['success']:
        raise SystemExit(1)


if __name__ == '__main__':
    main()
