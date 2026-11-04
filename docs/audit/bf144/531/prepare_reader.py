import hashlib
import json
from pathlib import Path

root=Path(__file__).resolve().parent
previous=root.parent/'530'/'prepare_reader.py'
script=previous.read_text().replace('tools/tmp/bf144/530','tools/tmp/bf144/531')
script=script.replace("(root/'native_blocker.inc')","(root.parent/'530'/'native_blocker.inc')")
exec(compile(script,previous.name,'exec'))
path=Path('tools/tmp/bf144/531/reader-source')/relative
text=path.read_text()
for anchor,replacement in (
    ('FVector(SpawnXY.X + 224, SpawnXY.Y - 30, Top - 10)',
     'FVector(SpawnXY.X + 239, SpawnXY.Y - 30, Top - 10)'),
    ('                    UCharacterMovementComponent* MutableMovement = Character->GetCharacterMovement();',
     (root/'perch_query.inc').read_text()+'                    UCharacterMovementComponent* MutableMovement = Character->GetCharacterMovement();'),
):
    assert text.count(anchor)==1
    text=text.replace(anchor,replacement)
data=text.encode()
path.write_bytes(data)
assert (source/relative).read_bytes()==original
(root/'reader-source.json').write_text(json.dumps(dict(
    canonical_sha256=hashlib.sha256(original).hexdigest(),
    diagnostic_sha256=hashlib.sha256(data).hexdigest(),
    diagnostic_header_sha256=hashlib.sha256(header.read_bytes()).hexdigest()),indent=2)+'\n')
print('Prepared isolated read-only native broad, perch and merged floor evidence',flush=True)
