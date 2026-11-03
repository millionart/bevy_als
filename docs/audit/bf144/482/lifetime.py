import gzip
import json
import sys
from pathlib import Path

root = Path(__file__).resolve().parent
audit = root.parent
sys.path.insert(0, 'tools/animgraph_authoring')
from ue_control_rig_contract import compile_contract
from ue_control_rig_target import compile_target

def read(path):
    return json.loads(gzip.decompress(path.read_bytes()) if path.suffix == '.gz' else path.read_bytes())

rig = read(audit / '476/rig.native.json.gz')
contract = compile_contract(rig)
program = contract['program']
definition = read(audit / '476/inputs.json')['definition']
assert compile_target(contract)['program']['instructions'] == definition['program']['instructions']
instructions = program['instructions']
functions = program['functions']

def writes(op):
    if op['opcode'] in ('Copy', 'Zero'):
        return [op['output']]
    if op['opcode'] != 'Execute':
        return []
    graph, index = op['model']
    pins = {pin['name']: pin for pin in contract['graphs'][graph]['nodes'][index]['pins'] if pin['parent'] is None}
    return [ref for arg, ref in zip(functions[op['function']]['arguments'], op['operands'], strict=True)
            if pins[arg['name']]['direction'] in ('Output', 'IO', 'Hidden')]

def aliases(a, b):
    if (a['memory'], a['register']) != (b['memory'], b['register']):
        return False
    if 'slice' in a and 'slice' in b and a['slice'] != b['slice']:
        return False
    n = min(len(a['path']), len(b['path']))
    return a['path'][:n] == b['path'][:n]

def scalar(ref, frame):
    assert ref['memory'] in ('Literal', 'Work') and not ref['path']
    memory = rig['rig_vm']['literal'] if ref['memory'] == 'Literal' else frame['work_after']
    record = memory['properties'][ref['register']]
    if 'slice' in ref:
        record = record['elements'][ref['slice']]
    return record['numeric_value']

READ_ONLY = {
    'FAlsRigVMFunction_Clamp01Float::Execute',
    'FAlsRigVMFunction_IsGameWorld::Execute',
    'FRigUnit_GetCurveValue::Execute',
    'FRigUnit_GetTransform::Execute',
    'FRigVMFunction_ControlFlowBranch::Execute',
    'FRigVMFunction_MathFloatMul::Execute',
}
rows = []
late_functions = set()
for dataset in ('actual', 'weights'):
    print(f'Checking final native writer lifetime in {dataset}', flush=True)
    cases = read(audit / f'476/{dataset}-after.native.json.gz')['rig_node_history_probe']['cases']
    for case_index, case in enumerate(cases):
        for query, frame in enumerate(case['frames'][1:], 1):
            visits = next(e['instructions'] for e in frame['executions'] if e['entry'] == 'Forwards Solve')
            late = visits[visits.index(130) + 1:]
            weights = {}
            for pc in late:
                op = instructions[pc]
                if op['opcode'] != 'Execute':
                    continue
                function = functions[op['function']]['name']
                late_functions.add(function)
                if function != 'FRigUnit_TwoBoneIKSimplePerItem::Execute':
                    assert function in READ_ONLY, (pc, function)
                    continue
                ref = op['operands'][13]
                assert not any(aliases(ref, output) for later in late[late.index(pc)+1:] for output in writes(instructions[later]))
                weights[str(pc)] = scalar(ref, frame)
            assert weights == {'139': 0, '146': 0}
            rows.append(dict(dataset=dataset, case=case_index, query=query, arm_weights=weights))
assert len(rows) == 666
(root/'lifetime.json').write_text(json.dumps(dict(frames=rows, later_functions=sorted(late_functions), no_late_weight_alias=True, late_arms_return_before_pose_write=True), separators=(',', ':'))+'\n')
print('Verified both arm weights and no later pose writer on 666 native frames', flush=True)
