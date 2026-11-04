import hashlib
import json
from pathlib import Path

root=Path(__file__).resolve().parent
previous=root.parent/'534'/'prepare_reader.py'
script=previous.read_text().replace('tools/tmp/bf144/534','tools/tmp/bf144/535')
script=script.replace("(root/'perch_distance.inc')","(root.parent/'534'/'perch_distance.inc')")
exec(compile(script,previous.name,'exec'))
path=Path('tools/tmp/bf144/535/reader-source')/relative
text=path.read_text()
for anchor,replacement in (
    ('                Character->SetActorLocation(FVector(SpawnXY.X + Values[0], SpawnXY.Y + Values[1], Values[2]),',
     '                Character->SetActorScale3D(FVector(.3));\n'
     '                Character->SetActorLocation(FVector(SpawnXY.X + Values[0], SpawnXY.Y + Values[1], Values[2]),'),
    ('                        Value->SetNumberField(TEXT("above_base_cm"), AboveBase);',
     '                        Value->SetNumberField(TEXT("above_base_cm"), AboveBase);\n'
     '                        Value->SetNumberField(TEXT("capsule_radius_cm"), PawnRadius);\n'
     '                        Value->SetNumberField(TEXT("capsule_half_height_cm"), PawnHalfHeight);'),
):
    assert text.count(anchor)==1
    text=text.replace(anchor,replacement)
data=text.encode()
path.write_bytes(data)
assert (source/relative).read_bytes()==original
(root/'reader-source.json').write_text(json.dumps(dict(
    canonical_sha256=hashlib.sha256(original).hexdigest(),diagnostic_sha256=hashlib.sha256(data).hexdigest(),
    diagnostic_header_sha256=hashlib.sha256(header.read_bytes()).hexdigest()),indent=2)+'\n')
print('Prepared the real native scaled capsule floor and walking observation',flush=True)
