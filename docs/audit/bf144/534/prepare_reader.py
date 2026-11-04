import hashlib
import json
from pathlib import Path

root=Path(__file__).resolve().parent
previous=root.parent/'533'/'prepare_reader.py'
script=previous.read_text().replace('tools/tmp/bf144/533','tools/tmp/bf144/534')
script=script.replace("(root/'mode_observer.inc')","(root.parent/'533'/'mode_observer.inc')")
exec(compile(script,previous.name,'exec'))
path=Path('tools/tmp/bf144/534/reader-source')/relative
text=path.read_text()
first=json.loads((root/'walking-first-cm.json').read_bytes())
point=', '.join(repr(first[axis]) for axis in ('x','y','z'))
for anchor,replacement in (
    ('                        const bool bValidPerch = Movement->ComputePerchResult(PerchRadius, Broad.HitResult, MaxPerch, Perch);',
     (root/'perch_distance.inc').read_text()+
     '                        const bool bValidPerch = Movement->ComputePerchResult(PerchRadius, Broad.HitResult, MaxPerch, Perch);'),
    ('                Row->SetArrayField(TEXT("mode_events"), ModeEvents);',
     '                Row->SetArrayField(TEXT("mode_events"), ModeEvents);\n'
     '                if (Rows.Num() == 40) { Row->SetObjectField(TEXT("walking_first_floor"), '
     f'CaptureFloorQuery(FVector({point}), true)); }}'),
):
    assert text.count(anchor)==1
    text=text.replace(anchor,replacement)
data=text.encode()
path.write_bytes(data)
assert (source/relative).read_bytes()==original
(root/'reader-source.json').write_text(json.dumps(dict(
    canonical_sha256=hashlib.sha256(original).hexdigest(),diagnostic_sha256=hashlib.sha256(data).hexdigest(),
    diagnostic_header_sha256=hashlib.sha256(header.read_bytes()).hexdigest()),indent=2)+'\n')
print('Prepared native raw narrow hit before the perch distance check',flush=True)
