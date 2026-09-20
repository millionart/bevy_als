import argparse
import copy
import json
import math
from pathlib import Path


COUNTS = (0, 1, 2, 3, 2, 1, 0, 3, 3, 0)


def plan():
    return [(stage, count, receiver, index if count else None)
            for stage, count in enumerate(COUNTS)
            for index in range(max(count, 1)) for receiver in (1, 2)]


def audit(report, source):
    issues = []
    probes = report.get('probes', [])
    expected = plan()
    by_id = {probe.get('id'): probe for probe in probes}
    if len(by_id) != len(probes) or set(by_id) != set(range(1, len(expected) + 1)):
        issues.append('GPU request coverage is incomplete or duplicated')
    if report.get('probe_errors'):
        issues.append('Continuous GPU data validation failed')
    frames = []
    for request_id, (stage, count, receiver, index) in enumerate(expected, 1):
        probe = by_id.get(request_id)
        if not probe:
            continue
        samples = probe.get('samples', [])
        if probe.get('receiver') != receiver or probe.get('index') != index or probe.get('count') != 16 or len(samples) != 16:
            issues.append(f'Request {request_id} has wrong receiver, index, or sample count')
            continue
        plane = probe.get('plane', [])
        if len(plane) != 4 or not all(math.isfinite(value) for value in plane) or plane[3] <= 0:
            issues.append(f'Request {request_id} lacks its receiver plane')
            continue
        for sample in samples:
            if len(sample) != 40 or not all(math.isfinite(value) for value in sample):
                issues.append(f'Request {request_id} contains invalid GPU data')
                break
            frames.append(sample[28])
            if sample[28] != sample[19] + 1 or sample[28] <= 0 or sample[30] < max(1, count):
                issues.append(f'Request {request_id} read a stale or undersized buffer')
                break
            if abs(sample[37] - plane[0]) > 1e-4 or math.hypot(sample[36] - plane[1], sample[38] - plane[2]) > plane[3] + 1e-4:
                issues.append(f'Request {request_id} sampled outside its receiver plane')
                break
            before = sample[:8]
            scale = before[0] / 0.8 if receiver == 2 else 1.0
            wanted = [0.8 * scale, 0.7 * scale, 0.6 * scale, 0.8, 0.1, 0.7, 0.7, 0.7]
            if not 0 <= scale <= 1.00001 or any(abs(a - b) > 1e-5 for a, b in zip(before, wanted)):
                issues.append(f'Request {request_id} sampled the wrong receiving material')
                break
            if index is None:
                if sample[29] != -1 or sample[31] != 0 or any(abs(a - b) > 1e-5 for a, b in zip(before, sample[20:28])):
                    issues.append(f'Request {request_id} did not restore the empty receiver')
                    break
                continue
            alpha = max(0, min(1, sample[11] * sample[15]))
            opacity = source['opacity'] * (0.5 if stage == 8 else 1.0)
            source_values = [source['base_color']] * 3 + [opacity, source['roughness_scale'], source['metallic'], source['specular']]
            if any(abs(a - b) > 1e-5 for a, b in zip(sample[12:19], source_values)):
                issues.append(f'Request {request_id} read the wrong decal material or fade')
                break
            if sample[29] != index or alpha <= 1e-4 or max(abs(sample[32] - sample[34]), abs(sample[33] - sample[35])) > 1e-4:
                issues.append(f'Request {request_id} mismatched the native decal projection')
                break
            blend = lambda a, b: a * (1 - alpha) + b * alpha
            after = [blend(before[channel], sample[12 + channel]) for channel in range(3)]
            after += [blend(before[3], sample[9] * source['roughness_scale']), blend(before[4], source['metallic'])]
            after += [math.sqrt(blend(value * value, source['specular'] * 0.5)) for value in before[5:8]]
            if any(abs(a - b) > 1e-5 for a, b in zip(sample[20:28], after)):
                issues.append(f'Request {request_id} failed receiver material composition')
                break
    bindings = report.get('bindings', [])
    if len({entry.get('logical_buffer') for entry in bindings}) != 1 or len({entry.get('gpu_buffer') for entry in bindings}) < 3:
        issues.append('Buffer identity evidence does not cover real growth with a stable handle')
    for kind in ('standard', 'playground'):
        groups = {item for entry in bindings for item in entry.get('receiver_bind_groups', []) if item.startswith(kind + ':')}
        if len(groups) < 3:
            issues.append(f'{kind} receiver binding evidence is incomplete')
    if not frames or report.get('positive_frame', 0) <= max(frames):
        issues.append('GPU observation did not continue into real animated footsteps')
    return {'pass': not issues, 'issues': issues, 'requests': len(by_id), 'samples': sum(len(probe.get('samples', [])) for probe in probes)}


def self_test():
    source = dict(base_color=0.0, opacity=0.3, roughness_scale=0.2, metallic=0.0, specular=0.5)
    probes = []
    for request_id, (stage, count, receiver, index) in enumerate(plan(), 1):
        before = [0.8, 0.7, 0.6, 0.8, 0.1, 0.7, 0.7, 0.7]
        opacity = source['opacity'] * (0.5 if stage == 8 else 1)
        alpha = 0 if index is None else opacity * 0.5
        blend = lambda a, b: a * (1 - alpha) + b * alpha
        after = [blend(value, 0) for value in before[:3]] + [blend(0.8, 0.16), blend(0.1, 0)] + [math.sqrt(blend(0.49, 0.25))] * 3
        sample = before + [0.1, 0.8, 0.3, 0.5] + [0, 0, 0, opacity, 0.2, 0, 0.5, request_id + 40]
        sample += after + [request_id + 41, -1 if index is None else index, 4, int(index is not None), 0.3, 0.4, 0.3, 0.4, 0, 0, 0, 1]
        probes.append(dict(id=request_id, receiver=receiver, index=index, count=16, plane=[0, 0, 0, 2], samples=[sample] * 16))
    report = dict(probes=probes, probe_errors=[], positive_frame=200, bindings=[dict(logical_buffer='handle', gpu_buffer=str(index),
        receiver_bind_groups=[f'standard:{index}', f'playground:{index}']) for index in range(3)])
    assert audit(report, source)['pass']
    for change in ('early_only', 'missing_receiver', 'stale', 'uv', 'fade', 'binding', 'late', 'health', 'empty'):
        bad = copy.deepcopy(report)
        if change == 'early_only': bad['probes'] = bad['probes'][:1]
        elif change == 'missing_receiver': bad['probes'] = [probe for probe in bad['probes'] if probe['receiver'] == 1]
        elif change == 'stale': bad['probes'][4]['samples'][0][19] = 0
        elif change == 'uv': bad['probes'][4]['samples'][0][32] += 0.1
        elif change == 'fade': bad['probes'][-3]['samples'][0][15] = 0.3
        elif change == 'binding': bad['bindings'] = bad['bindings'][:1]
        elif change == 'late': bad['positive_frame'] = 0
        elif change == 'health': bad['probe_errors'] = ['Stale data']
        elif change == 'empty': bad['probes'][-1]['samples'][0][20] = 0
        assert not audit(bad, source)['pass'], change
    return {'pass': True, 'negative_cases': 9}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--report', type=Path)
    parser.add_argument('--self-test', action='store_true')
    args = parser.parse_args()
    if args.self_test:
        result = self_test()
    else:
        library = json.loads((Path(__file__).resolve().parents[1] / 'assets/effects/footprint.runtime.json').read_text())
        definition = next(item for item in library['definitions'] if item['source_settings'].endswith('FES_Als_Default.FES_Als_Default'))
        source = definition['materials'][definition['effects'][0]['material']]
        result = audit(json.loads(args.report.read_text(encoding='utf-8-sig')), source)
    print(json.dumps(result, indent=2))
    return 0 if result['pass'] else 1


if __name__ == '__main__':
    raise SystemExit(main())
