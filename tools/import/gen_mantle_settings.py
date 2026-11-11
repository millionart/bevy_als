import argparse
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'tools/animgraph_authoring'))
from ue_animation_asset_resolver import asset_property, rich_curve_key, source_sequence_curves, source_sequence_timing
from ue_animgraph_pose_renderer import pose_curves_ron
from ue_exported_text import field, parse_bool, parse_list, parse_number, parse_quoted, parse_struct
from gen_als_ragdoll_settings import object_path


def trace_debug_settings(control):
    channel = field(control, 'MantlingTraceChannel')
    if channel != 'ECC_Visibility':
        raise ValueError('Unsupported mantle trace channel: ' + channel)
    return {
        'trace_layer': 'Visibility',
        'draw_failed_grounded_traces': parse_bool(field(parse_struct(field(control, 'GroundedTrace')), 'bDrawFailedTraces', 'False')),
        'draw_failed_in_air_traces': parse_bool(field(parse_struct(field(control, 'InAirTrace')), 'bDrawFailedTraces', 'False')),
    }


def selection_routes(source):
    graph, = [graph for graph in source['graphs'] if graph['name'] == 'SelectMantlingSettings']
    nodes = {node['object_path']: node for node in graph['nodes']}
    routes = {'types': {}, 'overlays': {}}
    for node in nodes.values():
        if node['class_path'] != '/Script/BlueprintGraph.K2Node_FunctionResult':
            continue
        pins = {pin['name']: pin for pin in node['pins']}
        result = pins['ReturnValue']['default_object']
        if pins['ReturnValue']['linked_to']:
            raise ValueError('mantle selection result is not a direct asset')
        for link in pins['execute']['linked_to']:
            parent = nodes[link['node_object_path']]['class_path'].split('.')[-1]
            if parent == 'K2Node_SwitchEnum':
                routes['types'][link['pin_name']] = result
            elif parent == 'GameplayTagsK2Node_SwitchGameplayTag':
                routes['overlays'][link['pin_name'].split('.')[-1]] = result
            else:
                raise ValueError('mantle selection flow is unsupported')
    if set(routes['types']) != {'High', 'InAir'} or 'Default' not in routes['overlays']:
        raise ValueError('mantle selection routes are incomplete')
    return routes


def montage_curves(source):
    data = parse_struct(asset_property(source, 'RawCurveData'))
    curves = []
    for raw in parse_list(field(data, 'FloatCurves', '()')):
        curve = parse_struct(raw)
        floats = parse_struct(field(curve, 'FloatCurve'))
        keys = [rich_curve_key(key) for key in parse_list(field(floats, 'Keys', '()'))]
        record = {'name': parse_quoted(field(curve, 'CurveName')), 'flags': int(field(curve, 'CurveTypeFlags', '0')),
                  'pre_infinity': field(floats, 'PreInfinityExtrap', 'RCCE_Constant'),
                  'post_infinity': field(floats, 'PostInfinityExtrap', 'RCCE_Constant')}
        for target, origin in [('times', 't'), ('values', 'v'), ('tangent_modes', 'tangent_mode'),
                               ('weight_modes', 'tangent_weight_mode'), ('arrive_tangents', 'arrive_tangent'),
                               ('leave_tangents', 'leave_tangent'), ('arrive_weights', 'arrive_weight'),
                               ('leave_weights', 'leave_weight')]:
            record[target] = [key[origin] for key in keys]
        record['interps'] = [key['interpolation'].upper() for key in keys]
        record['tangent_modes'] = [['Auto', 'User', 'Break', 'None', 'SmartAuto'].index(mode) for mode in record['tangent_modes']]
        record['weight_modes'] = [['None', 'Arrive', 'Leave', 'Both'].index(mode) for mode in record['weight_modes']]
        curves.append(record)
    if len({curve['name'] for curve in curves}) != len(curves):
        raise ValueError('mantle montage curve name is duplicated')
    return curves


def montage_record(source, sequence, timing, sequence_curves, assets_root):
    track, = parse_list(asset_property(source, 'SlotAnimTracks'))
    track = parse_struct(track)
    if parse_quoted(field(track, 'SlotName')) != 'PostLocomotion':
        raise ValueError('mantle montage slot is unsupported')
    segment, = parse_list(field(parse_struct(field(track, 'AnimTrack')), 'AnimSegments'))
    segment = parse_struct(segment)
    if object_path(field(segment, 'AnimReference')) != sequence['asset_path']:
        raise ValueError('mantle sequence identity differs from its segment')
    for key, expected in [('StartPos', 0.0), ('AnimStartTime', 0.0), ('AnimPlayRate', 1.0), ('LoopingCount', 1.0)]:
        if parse_number(field(segment, key, str(expected))) != expected:
            raise ValueError('mantle segment setting is unsupported: ' + key)
    duration = timing['duration']
    if abs(parse_number(field(segment, 'AnimEndTime')) - duration) > 1e-5 or abs(float(asset_property(source, 'SequenceLength')) - duration) > 1e-5:
        raise ValueError('mantle montage does not use the complete sequence')
    for asset, expected in [(source, {'BlendModeIn': 'Standard', 'BlendModeOut': 'Standard', 'bEnableAutoBlendOut': 'True',
                                     'BlendProfileIn': 'None', 'BlendProfileOut': 'None', 'bEnableRootMotionTranslation': 'False',
                                     'bEnableRootMotionRotation': 'False'}),
                            (sequence, {'bEnableRootMotion': 'False', 'bForceRootLock': 'True', 'RootMotionRootLock': 'RefPose', 'AdditiveAnimType': 'AAT_None'})]:
        for key, value in expected.items():
            if asset_property(asset, key) != value:
                raise ValueError('mantle source setting is unsupported: ' + key)
    if float(asset_property(sequence, 'RateScale')) != 1.0 or float(asset_property(source, 'BlendOutTriggerTime')) != 0.0:
        raise ValueError('mantle timing policy is unsupported')
    blend_in = parse_struct(asset_property(source, 'BlendIn'))
    blend_out = parse_struct(asset_property(source, 'BlendOut'))
    if field(blend_in, 'BlendOption') != 'HermiteCubic' or field(blend_out, 'BlendOption') not in {'HermiteCubic', 'Cubic'}:
        raise ValueError('mantle alpha blend mode is unsupported')
    if any(field(blend, 'CustomCurve', 'None') != 'None' for blend in [blend_in, blend_out]):
        raise ValueError('mantle custom blend curve is unsupported')
    clip = sequence['asset_path'].split('.', 1)[0].removeprefix('/ALS/') + '.anim.ron'
    if not (assets_root / clip).is_file():
        raise ValueError('mantle target clip is missing: ' + clip)
    values = {'source_montage': source['asset_path'], 'source_sequence': sequence['asset_path'], 'clip': clip,
              'sequence_length': duration, 'play_rate': float(asset_property(source, 'RateScale')),
              'blend_in_seconds': parse_number(field(blend_in, 'BlendTime')),
              'blend_out_seconds': parse_number(field(blend_out, 'BlendTime')),
              'blend_out_mode': field(blend_out, 'BlendOption')}
    if values['play_rate'] <= 0.0 or min(values['blend_in_seconds'], values['blend_out_seconds']) < 0.0:
        raise ValueError('mantle timing values are invalid')
    curves = montage_curves(source)
    fields = [name + ': ' + (value if name == 'blend_out_mode' else json.dumps(value)) for name, value in values.items()]
    fields += ['sequence_curves: ' + pose_curves_ron(sequence_curves), 'montage_curves: ' + pose_curves_ron(curves)]
    return values, '(' + ', '.join(fields) + ')', curves


def generate(source_dir, content_root, mount, assets_root):
    records = {}
    for path in source_dir.glob('*.ue.json'):
        record = json.loads(path.read_text(encoding='utf-8-sig'))
        if record['asset_path'] in records:
            raise ValueError('source record is duplicated')
        records[record['asset_path']] = record
    character, = [value for value in records.values() if value['asset_path'].endswith('.B_Als_Character')]
    settings, = [value for value in records.values() if value['asset_path'].endswith('.CS_Als_Default')]
    if object_path(asset_property({'asset_properties': character['blueprint_cdo_properties']}, 'Settings')) != settings['asset_path']:
        raise ValueError('mantle settings differ from the source character selection')
    routes = selection_routes(character)
    setting_paths = sorted(set(routes['types'].values()) | set(routes['overlays'].values()))
    if len(setting_paths) != 7:
        raise ValueError('mantle selection closure must contain seven settings')
    montage_paths = sorted({object_path(asset_property(records[path], 'Montage')) for path in setting_paths})
    if len(montage_paths) != 6:
        raise ValueError('mantle selection closure must contain six montages')
    montage_records = []
    montage_ron = []
    for path in montage_paths:
        source = records[path]
        sequence_path = object_path(asset_property(source, 'SlotAnimTracks'))
        sequence = records[sequence_path]
        timing = source_sequence_timing(content_root, mount, sequence_path)
        values, text, curves = montage_record(source, sequence, timing, source_sequence_curves(content_root, mount, sequence_path), assets_root)
        montage_records.append(values)
        montage_ron.append(text)
        print('Converted ' + path + ' curves=' + str(len(curves)), flush=True)
    kind_by_path = {}
    actions = []
    for path in setting_paths:
        source = records[path]
        kind = next((key for key, value in routes['types'].items() if value == path), None)
        if kind is None:
            kind = path.split('.')[-1].removeprefix('MS_Als_').replace('_', '')
        if kind not in {'High', 'InAir', 'Low', 'LowLeft', 'LowRight', 'LowBothHands', 'LowBox'}:
            raise ValueError('mantle setting identity is unsupported')
        kind_by_path[path] = kind
        for key, expected in [('bAutoCalculateStartTime', 'False'), ('MotionWarpingLocationBlendOption', 'Linear'),
                              ('MotionWarpingRotationBlendOption', 'HermiteCubic'), ('MotionWarpingLocationCustomBlendCurve', 'None'),
                              ('MotionWarpingRotationCustomBlendCurve', 'None')]:
            if asset_property(source, key) != expected:
                raise ValueError('mantle warp setting is unsupported: ' + key)
        index = montage_paths.index(object_path(asset_property(source, 'Montage')))
        montage = montage_records[index]
        heights = parse_struct(asset_property(source, 'StartTimeReferenceHeight'))
        times = parse_struct(asset_property(source, 'StartTime'))
        warp = parse_struct(asset_property(source, 'MotionWarpingTimeRange'))
        action = {'sequence_length': montage['sequence_length'], 'rate_scale': montage['play_rate'], 'montage_index': index,
                  'blend_in_seconds': montage['blend_in_seconds'],
                  'start_time_reference_min_height': parse_number(field(heights, 'X')) * 0.01,
                  'start_time_reference_max_height': parse_number(field(heights, 'Y')) * 0.01,
                  'start_time_at_min_height': parse_number(field(times, 'X')), 'start_time_at_max_height': parse_number(field(times, 'Y')),
                  'motion_warping_start_time': parse_number(field(warp, 'Min')), 'motion_warping_end_time': parse_number(field(warp, 'Max'))}
        actions.append(kind + ': (' + ', '.join(key + ': ' + repr(value) for key, value in action.items()) + ')')
    low_kinds = {key: kind_by_path[path] for key, path in routes['overlays'].items()}
    control = parse_struct(asset_property(settings, 'Mantling'))
    debug = trace_debug_settings(control)
    output = ('(\n    actions: {\n        ' + ',\n        '.join(actions) + '\n    },\n    low_kinds: {'
              + ', '.join(json.dumps('Als.OverlayMode.' + key) + ': ' + value for key, value in sorted(low_kinds.items())) + '},\n    montages: [\n        '
              + ',\n        '.join(montage_ron) + '\n    ],\n    stop_blend_out_seconds: ' + repr(parse_number(field(control, 'BlendOutDuration')))
              + ',\n    start_ragdoll_on_target_destruction: ' + str(parse_bool(field(control, 'bStartRagdollingOnTargetPrimitiveDestruction'))).lower()
              + ',\n    trace_layer: ' + debug['trace_layer']
              + ',\n    draw_failed_grounded_traces: ' + str(debug['draw_failed_grounded_traces']).lower()
              + ',\n    draw_failed_in_air_traces: ' + str(debug['draw_failed_in_air_traces']).lower() + ',\n)\n')
    return output, {'success': True, 'settings': setting_paths, 'montages': montage_records, 'low_kinds': low_kinds}


def self_test():
    control = parse_struct('(MantlingTraceChannel=ECC_Visibility,GroundedTrace=(bDrawFailedTraces=True),InAirTrace=())')
    assert trace_debug_settings(control) == {'trace_layer': 'Visibility', 'draw_failed_grounded_traces': True, 'draw_failed_in_air_traces': False}
    swapped = trace_debug_settings(parse_struct('(MantlingTraceChannel=ECC_Visibility,GroundedTrace=(),InAirTrace=(bDrawFailedTraces=True))'))
    assert not swapped['draw_failed_grounded_traces'] and swapped['draw_failed_in_air_traces']
    control['MantlingTraceChannel'] = 'ECC_Camera'
    try:
        trace_debug_settings(control)
    except ValueError:
        pass
    else:
        raise AssertionError('Camera was aliased to Visibility')
    nodes = [{'object_path': 'types', 'class_path': '/Script/BlueprintGraph.K2Node_SwitchEnum'},
             {'object_path': 'tags', 'class_path': '/Script/GameplayTagsEditor.GameplayTagsK2Node_SwitchGameplayTag'}]
    for name, parent in [('High', 'types'), ('InAir', 'types'), ('Default', 'tags'), ('Als.OverlayMode.Bow', 'tags')]:
        nodes.append({'object_path': name, 'class_path': '/Script/BlueprintGraph.K2Node_FunctionResult', 'pins': [
            {'name': 'ReturnValue', 'default_object': '/Fixture/' + name, 'linked_to': []},
            {'name': 'execute', 'linked_to': [{'node_object_path': parent, 'pin_name': name}]}]})
    graph = {'graphs': [{'name': 'SelectMantlingSettings', 'nodes': nodes}]}
    routes = selection_routes(graph)
    assert routes['types']['High'] == '/Fixture/High' and routes['overlays']['Bow'] == '/Fixture/Als.OverlayMode.Bow'
    nodes[-1]['pins'][0]['linked_to'] = [{'node_object_path': 'unexpected'}]
    try:
        selection_routes(graph)
    except ValueError:
        pass
    else:
        raise AssertionError('indirect mantle selection was accepted')
    source = {'asset_properties': [{'name': 'RawCurveData', 'exported_text': '(FloatCurves=((CurveName="LayerArmLeft",CurveTypeFlags=4,FloatCurve=(Keys=((InterpMode=RCIM_Cubic,Time=0.2,Value=-1.0,LeaveTangent=0.5),(Time=0.8,Value=1.0))))))'}]}
    original = json.dumps(source)
    curve, = montage_curves(source)
    assert json.dumps(source) == original
    assert curve['values'] == [-1.0, 1.0] and curve['interps'] == ['CUBIC', 'LINEAR']
    assert curve['leave_tangents'] == [0.5, 0.0] and 'values: [-1.0, 1.0]' in pose_curves_ron([curve])
    source['asset_properties'][0]['exported_text'] = source['asset_properties'][0]['exported_text'].replace('RCIM_Cubic', 'Unknown')
    try:
        montage_curves(source)
    except ValueError:
        pass
    else:
        raise AssertionError('unsupported mantle interpolation was accepted')
    print('Mantle source generator self-test PASS', flush=True)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--source-json-dir', type=Path)
    parser.add_argument('--content-root', type=Path)
    parser.add_argument('--mount', default='/ALS')
    parser.add_argument('--output', type=Path, default=ROOT / 'assets/ALS/Data/Character/Mantle/als_mantle_settings.ron')
    parser.add_argument('--report', type=Path, default=ROOT / 'target/als_mantle_settings.report.json')
    parser.add_argument('--self-test', action='store_true')
    args = parser.parse_args()
    if args.self_test:
        self_test()
        return
    report = {'success': False}
    try:
        if args.source_json_dir is None or args.content_root is None:
            raise ValueError('native source records and content root are required')
        output, report = generate(args.source_json_dir, args.content_root, args.mount, ROOT / 'assets')
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(output, encoding='utf-8')
    except (ValueError, KeyError, OSError, RuntimeError, StopIteration) as error:
        report['error'] = str(error)
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')
    print(json.dumps(report), flush=True)
    if not report['success']:
        raise SystemExit(1)


if __name__ == '__main__':
    main()
