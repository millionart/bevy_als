import gzip
import json
import struct
import sys
from pathlib import Path

r=Path(__file__).resolve().parent;v=json.loads((r/'replay-baseline.json').read_bytes()if(r/'replay-baseline.json').exists()else gzip.decompress((r/'replay-baseline.json.gz').read_bytes()));f=lambda x:struct.unpack('f',struct.pack('f',x))[0]
def vector(row,key):return row[key]['translation']
def pair(rows,a,b):
 bad=[x for x in rows if vector(x,a)!=vector(x,b)];f32=[x for x in rows if list(map(f,vector(x,a)))!=list(map(f,vector(x,b)))];peak=max((max(abs(i-j)for i,j in zip(vector(x,a),vector(x,b),strict=True))for x in rows),default=0.0)
 return dict(double_differences=len(bad),f32_differences=len(f32),peak_meters=peak,first=bad[0]if bad else None,first_f32=f32[0]if f32 else None)
def effects(rows):
 def unequal(row,key):return list(map(f,vector(row,key)))!=list(map(f,vector(row,'native')))
 return dict(recovered=sum(unequal(x,'current_meter')and not unequal(x,'current_centimeter')for x in rows),regressed=sum(not unequal(x,'current_meter')and unequal(x,'current_centimeter')for x in rows),changed=sum(list(map(f,vector(x,'current_meter')))!=list(map(f,vector(x,'current_centimeter')))for x in rows),current_meter_native_different=sum(unequal(x,'current_meter')for x in rows),current_centimeter_native_different=sum(unequal(x,'current_centimeter')for x in rows))
common=v['common'];assert len(common)==696 and len(v['actual'])==1272
for row in common+v['actual']:
 for prefix in['current','source']:
  assert row[prefix+'_meter']['rotation']==row[prefix+'_centimeter']['rotation']and row[prefix+'_meter']['scale']==row[prefix+'_centimeter']['scale']
source_phase=pair(common,'source_meter','source_centimeter');assert source_phase['double_differences']==693 and source_phase['f32_differences']==691
summary=dict(gate='PASS_INDEPENDENT_PUBLIC_UNIT_PHASE_CONTROLS_WITH_PRESERVED_ALGORITHM_Q_S_AND_CACHE',common_current=pair(common,'current_meter','current_centimeter'),common_source=source_phase,common_current_effects=effects(common),actual={str(mode):dict(current=pair([x for x in v['actual']if x['mode']==mode],'current_meter','current_centimeter'),source=pair([x for x in v['actual']if x['mode']==mode],'source_meter','source_centimeter'),effects=effects([x for x in v['actual']if x['mode']==mode]))for mode in[0,1]},source_native_cm_endpoints_all_exact=True,changed_f32_means_numeric_not_proven_visible_problem=True,no_f32_state_or_coefficient_cast_in_control=True)
(r/'analysis.json').write_text(json.dumps(summary,indent=2)+'\n');print(json.dumps({k:{j:d[j]for j in['double_differences','f32_differences','peak_meters']}for k,d in [('common_current',summary['common_current']),('common_source',source_phase)]}),flush=True);print(json.dumps({k:x['effects']for k,x in summary['actual'].items()}),flush=True)
if len(sys.argv)>1 and sys.argv[1]=='no-unit-control':assert summary['common_current']['double_differences']==0,'Public cm/m control cannot be deleted with observable independent phase signal'
