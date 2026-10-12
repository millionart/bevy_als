import argparse
import json
import sys
from copy import deepcopy
from pathlib import Path

sys.path.insert(0,str(Path(__file__).resolve().parents[1]/'animgraph_authoring'))
from gen_ragdoll import target_material


def compile_definition(source):
    if source['asset_class']!='/Script/PhysicsCore.PhysicalMaterial':
        raise ValueError('Source default physical material class differs')
    material=target_material(source['asset_properties'])
    if material['friction_combine']!='Average' or material['restitution_combine']!='Average':
        raise ValueError('Source default physical material requires a different combine rule')
    return [material['dynamic_friction'],material['static_friction'],material['restitution']]


def main():
    parser=argparse.ArgumentParser()
    parser.add_argument('--source',type=Path,required=True)
    parser.add_argument('--output',type=Path,required=True)
    parser.add_argument('--check',action='store_true')
    parser.add_argument('--self-test',action='store_true')
    args=parser.parse_args()
    source=json.loads(args.source.read_text(encoding='utf-8-sig'))
    result=compile_definition(source)
    if args.self_test:
        changed=deepcopy(source)
        next(p for p in changed['asset_properties'] if p['name']=='StaticFriction')['exported_text']='0.9'
        assert compile_definition(changed)==[result[0],0.9,result[2]]
        next(p for p in changed['asset_properties'] if p['name']=='FrictionCombineMode')['exported_text']='Max'
        try:compile_definition(changed)
        except ValueError:pass
        else:raise AssertionError('A different source combine rule was silently ignored')
    text=json.dumps(result,indent=2)+'\n'
    if args.check:
        if args.output.read_text(encoding='utf-8')!=text:raise ValueError('Target default physical material differs')
    else:
        args.output.parent.mkdir(parents=True,exist_ok=True)
        args.output.write_text(text,encoding='utf-8')
    print(json.dumps(dict(success=True,coefficients=result,self_test=args.self_test,check=args.check)))


if __name__=='__main__':main()
