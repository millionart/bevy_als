import hashlib
import json
import sys
from pathlib import Path
r=Path(__file__).resolve().parent
p=Path('tools/ue_graph_source_reader/Source/AlsGraphSourceReader/Private/AlsGraphSourceReaderCommandlet.cpp')
mode=sys.argv[1]
if mode=='save':
    (r/'final-reader-sha256.json').write_text(json.dumps({'source_sha256':hashlib.sha256(p.read_bytes()).hexdigest()},indent=2)+'\n')
    Path('tools/tmp/bf144/442b0/reader.final.cpp').write_bytes(p.read_bytes())
else:
    s=Path('tools/tmp/bf144/442b0/reader.final.cpp').read_bytes()
    if mode=='clamp':
        old=b'            const double MagnitudeSquared = ClampedAxes.SizeSquared();\n            if (MagnitudeSquared > 1.0) { ClampedAxes *= FMath::InvSqrt(MagnitudeSquared); }\n';assert s.count(old)==1;s=s.replace(old,b'')
    elif mode=='yaw-guard':
        old=b'!(*Movement)->HasTypedField<EJson::Number>(TEXT("yaw_degrees")) || ';assert s.count(old)==1;s=s.replace(old,b'')
    else:assert mode=='restore'
    p.write_bytes(s)
print('Prepared diagnostic source variant '+mode,flush=True)
