import hashlib
import json
import sys
from pathlib import Path

root = Path(__file__).resolve().parent
reader = Path('tools/ue_graph_source_reader/Source/AlsGraphSourceReader/Private/AlsGraphSourceReaderCommandlet.cpp')
canonical = (reader if sys.argv[1]=='snapshot' else Path('tools/tmp/bf144/441a/reader.final.cpp')).read_bytes()
if sys.argv[1]=='snapshot':
    (Path('tools/tmp/bf144/441a')/'reader.final.cpp').write_bytes(canonical)
else:
    changes = {
        'axis': (b'SampleCurve = &VectorCurve->FloatCurves[CurveAxis];', b'SampleCurve = &VectorCurve->FloatCurves[0];'),
        'guard': (b'!(*Sample)->HasTypedField<EJson::Number>(TEXT("custom_curve_axis")) || ', b''),
    }
    case = sys.argv[1]
    variant = canonical
    if case in changes:
        old, new = changes[case]
        assert canonical.count(old)==1
        variant = canonical.replace(old,new)
    else:
        assert case=='restore'
    reader.write_bytes(variant)
    (root/f'source-{case}.json').write_text(json.dumps({'sha256':hashlib.sha256(variant).hexdigest()},indent=2)+'\n')
print('Reader source control '+sys.argv[1],flush=True)
