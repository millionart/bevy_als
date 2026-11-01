import json
from pathlib import Path

r=Path(__file__).resolve().parent;result={}
for label in ['no-output-hook','constant-action']:
    lines=(r/(label+'.native.json.reader.stdout.log')).read_text().splitlines()
    message=next(line.split('LogAlsGraphSourceReader: Error: ',1)[1]for line in lines if 'LogAlsGraphSourceReader: Error:'in line)
    assert not any('/'+s+'/'in message for s in ['Users','Volumes']);result[label]=dict(expected_exit_code=7,failure=message)
result['no-parent-guard']=dict(expected_exit_code=0,failure='Missing required capture_pose_boundaries was accepted after removing the parent guard')
(r/'negative-controls.json').write_text(json.dumps(result,indent=2)+'\n')
print('Persisted three minimal source capability counterexamples without private paths',flush=True)
