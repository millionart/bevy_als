import hashlib
import json
from pathlib import Path

root = Path(__file__).resolve().parent
previous = root.parent / '553' / 'prepare_reader.py'
script = previous.read_text().replace('tools/tmp/bf144/553', 'tools/tmp/bf144/566')
exec(compile(script, previous.name, 'exec'))
path = Path('tools/tmp/bf144/566/reader-source') / relative
text = path.read_text()
for anchor, before, after in (
    ('        if ((*Case)->HasField(TEXT("wall_x_cm")))', (root/'native_base.inc').read_text(), ''),
    ('            TSharedPtr<FJsonObject> ControllerBefore;', (root/'native_velocity.inc').read_text(), ''),
    ('            Value->SetNumberField(TEXT("floor_distance_cm"), Movement->CurrentFloor.FloorDist);', '',
     '''            Value->SetNumberField(TEXT("floor_distance_cm"), Movement->CurrentFloor.FloorDist);
            Value->SetStringField(TEXT("base_component"), GetNameSafe(Character->GetMovementBase()));
            Value->SetObjectField(TEXT("old_box_velocity_cm_s"), CaptureVector(Box->GetComponentVelocity()));
            Value->SetObjectField(TEXT("new_box_velocity_cm_s"), CaptureVector(AuditStepBase->GetComponentVelocity()));'''),
):
    assert text.count(anchor) == 1, anchor
    text = text.replace(anchor, before + (after or anchor))
data = text.encode()
path.write_bytes(data)
assert (source / relative).read_bytes() == original
(root / 'reader-source.json').write_text(json.dumps(dict(
    canonical_sha256=hashlib.sha256(original).hexdigest(),
    diagnostic_sha256=hashlib.sha256(data).hexdigest(),
    diagnostic_header_sha256=hashlib.sha256(path.with_name('AlsAuditBlockingHitObserver.h').read_bytes()).hexdigest()), indent=2) + '\n')
print('Prepared native within-frame support selection observation', flush=True)
