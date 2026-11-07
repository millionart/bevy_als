import argparse
import hashlib
import json
import struct
from pathlib import Path


PROFILES = ('normal', 'responsive', 'sluggish')
CHANNELS = ('acceleration', 'braking_deceleration', 'ground_friction')


def f32(value):
    return struct.unpack('<f', struct.pack('<f', value))[0]


def curve_references(value):
    references = set()
    if isinstance(value, dict):
        if value.get('name') == 'AccelerationAndDecelerationAndGroundFrictionCurve':
            text = value.get('exported_text', '')
            if "'" in text:
                references.add(text.split("'")[1])
        for child in value.values():
            references.update(curve_references(child))
    elif isinstance(value, list):
        for child in value:
            references.update(curve_references(child))
    return references


def convert_keys(keys, scale):
    if len(keys) < 2 or any(key['weight_mode'] != 0 for key in keys):
        raise ValueError('unsupported weighted or empty curve')
    output = [{'time': f32(key['time']), 'value': key['value'] * scale,
               'arrive_tangent': key['arrive_tangent'] * scale,
               'leave_tangent': key['leave_tangent'] * scale} for key in keys]
    for index, (start, end) in enumerate(zip(keys, keys[1:])):
        duration = f32(end['time'] - start['time'])
        if duration <= 0.0:
            raise ValueError('curve key times are not increasing')
        if start['interp_mode'] == 0:
            slope = f32(f32(end['value'] - start['value']) / duration)
            output[index]['leave_tangent'] = slope * scale
            output[index + 1]['arrive_tangent'] = slope * scale
        elif start['interp_mode'] != 2:
            raise ValueError('unsupported curve interpolation')
    return {'keys': output}


def generate(native_path, settings_paths, baseline_path, output_path):
    protected = {path.resolve() for path in (native_path, baseline_path, *settings_paths.values())}
    if output_path.resolve() in protected:
        raise ValueError('output would overwrite source data')
    native = json.loads(native_path.read_text())
    settings = {name: curve_references(json.loads(path.read_text())['asset_properties'])
                for name, path in settings_paths.items()}
    allowed = {'normal': settings['normal'], 'responsive': settings['normal'] & settings['responsive'],
               'sluggish': settings['sluggish']}
    result = {profile: {} for profile in PROFILES}
    identities = {}
    for sample in native['blend_profile_probe']['samples']:
        profile, axis_text = sample['name'].rsplit('-', 1)
        axis = int(axis_text)
        if profile not in result or axis not in range(3) or axis != sample['custom_curve_axis']:
            raise ValueError('curve sample identity is invalid')
        curve = sample['custom_curve']
        if curve not in allowed[profile] or sample['custom_curve_class'] != '/Script/Engine.CurveVector':
            raise ValueError('curve is not referenced by the source settings')
        if sample['custom_curve_pre_extrap'] != 4 or sample['custom_curve_post_extrap'] != 4:
            raise ValueError('unsupported curve extrapolation')
        if profile in identities and identities[profile] != curve:
            raise ValueError('profile axes use different curves')
        identities[profile] = curve
        channel = CHANNELS[axis]
        if channel in result[profile]:
            raise ValueError('duplicate curve axis')
        result[profile][channel] = convert_keys(sample['custom_curve_keys'], 0.01 if axis < 2 else 1.0)
    if any(set(result[profile]) != set(CHANNELS) for profile in PROFILES):
        raise ValueError('source curve axes are incomplete')
    baseline = json.loads(baseline_path.read_text())['original']
    changes = []
    for profile in PROFILES:
        for channel in CHANNELS:
            before = baseline[profile][channel]['keys']
            after = result[profile][channel]['keys']
            if len(before) != len(after):
                raise ValueError('baseline key count differs')
            for index, (old, new) in enumerate(zip(before, after)):
                for field in ('time', 'value', 'arrive_tangent', 'leave_tangent'):
                    if f32(old[field]) != f32(new[field]):
                        changes.append({'profile': profile, 'channel': channel, 'key': index,
                                        'field': field, 'before': f32(old[field]), 'after': f32(new[field])})
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(json.dumps(result, indent=2) + '\n')
    return {'source_curves': identities, 'changed_fields': changes,
            'native_sha256': hashlib.sha256(native_path.read_bytes()).hexdigest(),
            'baseline_sha256': hashlib.sha256(baseline_path.read_bytes()).hexdigest(),
            'output_sha256': hashlib.sha256(output_path.read_bytes()).hexdigest()}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--native', type=Path, required=True)
    for profile in PROFILES:
        parser.add_argument('--' + profile + '-settings', type=Path, required=True)
    parser.add_argument('--baseline', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--report', type=Path, required=True)
    args = parser.parse_args()
    settings = {profile: getattr(args, profile + '_settings') for profile in PROFILES}
    protected = {path.resolve() for path in (args.native, args.baseline, *settings.values(), args.output)}
    if args.report.resolve() in protected:
        raise ValueError('report would overwrite source data')
    report = generate(args.native, settings, args.baseline, args.output)
    args.report.write_text(json.dumps(report, indent=2) + '\n')
    print(f"Generated 9 source curve axes; {len(report['changed_fields'])} fields differ", flush=True)


if __name__ == '__main__':
    main()
