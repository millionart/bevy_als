import hashlib
import json
from pathlib import Path

root = Path(__file__).resolve().parent
previous = root.parent / '553' / 'prepare_reader.py'
script = previous.read_text().replace('tools/tmp/bf144/553', 'tools/tmp/bf144/569')
exec(compile(script, previous.name, 'exec'))
path = Path('tools/tmp/bf144/569/reader-source') / relative
text = path.read_text()
for anchor, replacement in (
    ('        if ((*Case)->HasField(TEXT("wall_x_cm")))', (root / 'native_walls.inc').read_text() + '        if ((*Case)->HasField(TEXT("wall_x_cm")))'),
    ('            Value->SetNumberField(TEXT("floor_distance_cm"), Movement->CurrentFloor.FloorDist);',
     '''            Value->SetBoolField(TEXT("just_teleported"), Movement->bJustTeleported);
            Value->SetStringField(TEXT("base_component"), GetNameSafe(Character->GetMovementBase()));
            Value->SetNumberField(TEXT("floor_distance_cm"), Movement->CurrentFloor.FloorDist);'''),
):
    assert text.count(anchor) == 1, anchor
    text = text.replace(anchor, replacement)
data = text.encode()
path.write_bytes(data)
assert (source / relative).read_bytes() == original
(root / 'reader-source.json').write_text(json.dumps(dict(
    canonical_sha256=hashlib.sha256(original).hexdigest(),
    diagnostic_sha256=hashlib.sha256(data).hexdigest(),
    diagnostic_header_sha256=hashlib.sha256(path.with_name('AlsAuditBlockingHitObserver.h').read_bytes()).hexdigest()), indent=2) + '\n')
print('Prepared native recoverable Walking penetration and velocity-guard observation', flush=True)
