import json
import subprocess
import sys
from pathlib import Path

r=Path(__file__).resolve().parent
rows=[]
for label in ['bad-delta','bad-world','bad-pose','bad-input']:
 result=subprocess.run([sys.executable,str(r/'check_native.py'),label],capture_output=True,text=True,timeout=30)
 assert result.returncode==1 and 'AssertionError' in result.stderr,label
 rows.append(dict(case=label,process_exit_code=result.returncode,error=result.stderr.replace(str(Path.cwd()),'.')))
 print('Rejected controlled native input fixture '+label,flush=True)
(r/'negative-fixtures.json').write_text(json.dumps(rows,indent=2)+'\n')
