import hashlib
import json
from pathlib import Path

root=Path(__file__).resolve().parent
previous=root.parent/'528'/'prepare_reader.py'
script=previous.read_text().replace('tools/tmp/bf144/528','tools/tmp/bf144/530')
script=script.replace("(root / 'floor_hit.inc')","(root.parent / '528' / 'floor_hit.inc')")
exec(compile(script,previous.name,'exec'))
path=Path('tools/tmp/bf144/530/reader-source')/relative
text=path.read_text()
for anchor,replacement in (
    ('FVector(SpawnXY.X + 230, SpawnXY.Y - 30, Top - 10)',
     'FVector(SpawnXY.X + 224, SpawnXY.Y - 30, Top - 10)'),
    ('        FActorSpawnParameters Spawn;\n        Spawn.SpawnCollisionHandlingOverride = ESpawnActorCollisionHandlingMethod::AlwaysSpawn;\n        FObservedPose HeadObservation',
     (root/'native_blocker.inc').read_text()+'        FActorSpawnParameters Spawn;\n        Spawn.SpawnCollisionHandlingOverride = ESpawnActorCollisionHandlingMethod::AlwaysSpawn;\n        FObservedPose HeadObservation'),
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
print('Prepared an isolated native floor blocker fixture',flush=True)
