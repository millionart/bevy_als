import hashlib
import json
from pathlib import Path

root=Path(__file__).resolve().parent
previous=root.parent/'536'/'prepare_reader.py'
script=previous.read_text().replace('tools/tmp/bf144/536','tools/tmp/bf144/549')
script=script.replace("(root/'step_down_floor.inc')","(root.parent/'536'/'step_down_floor.inc')")
exec(compile(script,previous.name,'exec'))
path=Path('tools/tmp/bf144/549/reader-source')/relative
text=path.read_text()
for anchor,replacement in (
    ('            Value->SetNumberField(TEXT("floor_distance_cm"), Movement->CurrentFloor.FloorDist);',
     '''            Value->SetNumberField(TEXT("floor_distance_cm"), Movement->CurrentFloor.FloorDist);
            Value->SetBoolField(TEXT("floor_blocking"), Movement->CurrentFloor.bBlockingHit);
            Value->SetStringField(TEXT("floor_component"), GetNameSafe(Movement->CurrentFloor.HitResult.GetComponent()));
            Value->SetObjectField(TEXT("floor_normal"), CaptureVector(Movement->CurrentFloor.HitResult.Normal));'''),

    ('        FActorSpawnParameters Spawn;\n        Spawn.SpawnCollisionHandlingOverride = ESpawnActorCollisionHandlingMethod::AlwaysSpawn;\n        FObservedPose HeadObservation',
     (root/'native_height.inc').read_text()+'        FActorSpawnParameters Spawn;\n        Spawn.SpawnCollisionHandlingOverride = ESpawnActorCollisionHandlingMethod::AlwaysSpawn;\n        FObservedPose HeadObservation'),
):
    assert text.count(anchor)==1
    text=text.replace(anchor,replacement)
data=text.encode()
path.write_bytes(data)
assert (source/relative).read_bytes()==original
(root/'reader-source.json').write_text(json.dumps(dict(
    canonical_sha256=hashlib.sha256(original).hexdigest(),diagnostic_sha256=hashlib.sha256(data).hexdigest(),
    diagnostic_header_sha256=hashlib.sha256(header.read_bytes()).hexdigest()),indent=2)+'\n')
print('Prepared native near-limit step heights',flush=True)
