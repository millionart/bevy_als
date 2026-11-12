from pathlib import Path
import argparse,gzip,json,struct

parser = argparse.ArgumentParser()
parser.add_argument('source', type=Path)
parser.add_argument('target', type=Path)
args = parser.parse_args()

def load(path):
    if path.suffix == '.gz':
        with gzip.open(path, 'rb') as data:
            return json.load(data)
    return json.loads(path.read_bytes())

n=load(args.source)['character_animation_probe']['cases'][0]
t=load(args.target)
fields=lambda p:{f['name']:f for f in p['fields']}
vec=lambda p:[fields(p)[k]['numeric_value'] for k in 'XZY']
f32=lambda v: struct.unpack('<f',struct.pack('<f',v))[0]

def rotate(v,q):
 x,y,z,w=q; i,j,k=v
 u=[2*(y*k-z*j),2*(z*i-x*k),2*(x*j-y*i)]
 a=[i+w*u[0],j+w*u[1],k+w*u[2]]
 return [a[0]+y*u[2]-z*u[1],a[1]+z*u[0]-x*u[2],a[2]+x*u[1]-y*u[0]]

native=n['frames'][1]; feet=fields(next(v for v in native['source_state_after_tick'] if v['name']=='FeetState'))
world=native['mesh_to_world']; q=[-world['rotation'][k] for k in 'xzy']+[world['rotation']['w']]
position=[world['translation'][k] for k in 'xzy']
a=[]
for i,side in enumerate(('Left','Right')):
 field=fields(feet[side]); target=t[2]['raw_rig']['feet'][side.lower()]; expected=vec(field['FinalLocation']); native_world=vec(field['TargetLocationWorldSpace']);
 assert vec(field['LockLocationWorldSpace'])==native_world and field['LockAmount']['numeric_value']==target['amount']==1.0
 assert target['target']==target['lock']==target['final']
 def local(world_point):
  return list(map(f32,rotate([x-y for x,y in zip(world_point,position)],[-q[0],-q[1],-q[2],q[3]])))
 assert local(native_world)==expected,(local(native_world),expected)
 reduced=[f32(x*.01)*100 for x in native_world]
 only_world_quantized=local(reduced)
 only_inverse_quantized=[f32(x*.01)*100 for x in expected]
 raw=t[2]['raw_rig']['externals']['RigInput']['List'][7+i*2]['Vector']
 a.append({'side':side,'source_world_cm':native_world,'target_world_m':target['target'],'source_component_cm':expected,
  'native_world_f32_m_only_control_cm':only_world_quantized,'native_component_f32_m_only_control_cm':only_inverse_quantized,'current_rig_cm':raw})
 assert only_world_quantized!=expected and only_inverse_quantized!=expected
print(json.dumps({'index':2,'source_mesh_cm':position,'source_rotation':q,'sides':a},indent=2))
