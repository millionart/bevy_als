import json
import sys
from pathlib import Path

r=Path(__file__).resolve().parent;label=sys.argv[1];factor,mode=label.split('_')
key={'product':'10','reciprocal':'01'}[factor]
analysis=json.loads((r/'analysis-baseline-fixed.json').read_text());witness=analysis['component_effects'][key]['first_restored'];assert witness is not None
assert witness['current']!=witness['native']and witness['controlled']==witness['native']
result=dict(factor=factor,mode=mode,same_current_input_native_not_actual_source_data=True,source_frame=witness['source_frame'],target_query=witness['source_frame']+1,bone=witness['bone'],component=witness['component'],current=witness['current'],controlled=witness['controlled'],native=witness['native'],whole_rotation_or_pose_restored=False)
(r/('contract-'+label+'.json')).write_text(json.dumps(result,indent=2)+'\n')
assert witness[mode]==witness['native'],'Same-input native Head quaternion component contract: '+label
print('Same-input native component passed only the explicit method control '+label,flush=True)
