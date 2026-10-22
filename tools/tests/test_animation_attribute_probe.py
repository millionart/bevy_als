import argparse
import copy
import json
from pathlib import Path


def validate(document, request):
    probe = document['animation_attribute_probe']
    assert type(probe['root_motion_provider_available']) is bool
    blueprints = probe['blueprints']
    assert len(blueprints) == len(request['animation_blueprints']) > 0
    assert [row['asset'] for row in blueprints] == request['animation_blueprints']
    assets = {row['asset']: row for row in probe['assets']}
    assert len(assets) == len(probe['assets']) > 0
    roots = set()
    for blueprint in blueprints:
        nodes = blueprint['nodes']
        assert nodes and len({node['property'] for node in nodes}) == len(nodes)
        for node in nodes:
            assert node['type']
            for reference in node['references']:
                assert reference['property'] and reference['asset'] in assets
                roots.add(reference['asset'])
    visited = set()
    pending = list(roots)
    sequences = []
    while pending:
        path = pending.pop()
        if path in visited:
            continue
        visited.add(path)
        row = assets[path]
        assert row['type'] and len(set(row['dependencies'])) == len(row['dependencies'])
        assert all(path in assets for path in row['dependencies'])
        pending.extend(row['dependencies'])
        if row['type'] == '/Script/Engine.AnimSequence':
            sequences.append(row)
            assert type(row['model_attributes']) is int and row['model_attributes'] >= 0
            assert type(row['baked_attributes']) is int and row['baked_attributes'] >= 0
            assert len(row['model_identifiers']) == row['model_attributes']
            assert type(row['has_root_motion']) is bool
            samples = row['samples']
            assert len(samples) == 3
            assert [sample['time'] for sample in samples] == sorted(sample['time'] for sample in samples)
            for sample in samples:
                assert type(sample['raw_data']) is bool
                assert type(sample['attribute_count']) is int and sample['attribute_count'] >= 0
                assert sample['attribute_count'] == len(sample['attributes'])
                for attribute in sample['attributes']:
                    assert attribute['name'] and attribute['type'] and type(attribute['bone_index']) is int
                    assert isinstance(attribute['value'], str)
    assert visited == set(assets) and sequences
    return {'blueprints': len(blueprints), 'compiled_nodes': sum(len(row['nodes']) for row in blueprints), 'assets': len(assets), 'sequences': len(sequences), 'nonempty_model': sum(row['model_attributes'] > 0 for row in sequences), 'nonempty_baked': sum(row['baked_attributes'] > 0 for row in sequences), 'root_motion_sequences': sum(row['has_root_motion'] for row in sequences), 'nonempty_samples': sum(sample['attribute_count'] > 0 for row in sequences for sample in row['samples'])}


def self_test():
    request = {'animation_blueprints': ['bp']}
    sequence = {'asset': 'sequence', 'type': '/Script/Engine.AnimSequence', 'dependencies': [], 'model_attributes': 0, 'baked_attributes': 0, 'model_identifiers': [], 'has_root_motion': False, 'samples': [{'time': time, 'raw_data': False, 'attribute_count': 0, 'attributes': []} for time in [0, .5, 1]]}
    fixture = {'animation_attribute_probe': {'root_motion_provider_available': False, 'blueprints': [{'asset': 'bp', 'nodes': [{'property': 'node', 'type': 'player', 'references': [{'property': 'node.Sequence', 'asset': 'sequence'}]}]}], 'assets': [sequence]}}
    assert validate(fixture, request)['nonempty_samples'] == 0
    nonempty = copy.deepcopy(fixture)
    row = nonempty['animation_attribute_probe']['assets'][0]
    row['model_attributes'] = 1
    row['model_identifiers'] = [{'name': 'attribute', 'bone': 'root', 'type': 'float'}]
    row['samples'][0].update(attribute_count=1, attributes=[{'name': 'attribute', 'type': 'float', 'bone_index': 0, 'value': '(Value=1)'}])
    assert validate(nonempty, request)['nonempty_samples'] == 1
    rejected = 0
    for index in range(7):
        case = copy.deepcopy(fixture)
        probe = case['animation_attribute_probe']
        if index == 0:
            probe['assets'] = []
        elif index == 1:
            probe['assets'][0]['dependencies'] = ['missing']
        elif index == 2:
            probe['assets'][0]['model_attributes'] = 1
        elif index == 3:
            probe['assets'][0]['samples'][0]['attribute_count'] = 1
        elif index == 4:
            probe['blueprints'][0]['nodes'] = []
        elif index == 5:
            probe['assets'][0]['samples'] = []
        else:
            probe['assets'].append({'asset': 'orphan', 'type': 'other', 'dependencies': []})
        try:
            validate(case, request)
        except (KeyError, AssertionError):
            rejected += 1
    assert rejected == 7
    return {'self_test': 'PASS', 'invalid_cases_rejected': rejected, 'nonempty_attribute_preserved': True}


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--input', type=Path)
    parser.add_argument('--request', type=Path)
    parser.add_argument('--self-test', action='store_true')
    args = parser.parse_args()
    if args.self_test:
        result = self_test()
    else:
        assert args.input and args.request
        result = validate(json.loads(args.input.read_text()), json.loads(args.request.read_text()))
    print(json.dumps(result))
