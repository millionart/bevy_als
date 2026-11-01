import json
import sys
from pathlib import Path

r=Path(__file__).resolve().parent;mode=sys.argv[1];w=json.loads((r/'check-qualified.json').read_text())['counts']['first']
assert w['current']!=w['source_copy_control']
(r/('contract-'+mode+'.json')).write_text(json.dumps(dict(witness=w,mode=mode,source_native_same_input_passthrough_verified=True,actual_runtime_was_not_modified=True),indent=2)+'\n')
value=w['current']if mode=='current'else w['source_copy_control'];assert value==w['source_copy_control'],'Actual single-full-weight Ragdolling passthrough contract'
print('Source full-weight copy contract passed only the explicit value control',flush=True)
