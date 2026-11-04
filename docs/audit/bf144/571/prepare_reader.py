import hashlib
import json
from pathlib import Path

root = Path(__file__).resolve().parent
previous = root.parent / '570' / 'prepare_reader.py'
script = previous.read_text().replace('tools/tmp/bf144/570', 'tools/tmp/bf144/571')
exec(compile(script, previous.name, 'exec'))

text = path.read_text()
anchor = '            Value->SetNumberField(TEXT("floor_distance_cm"), Movement->CurrentFloor.FloorDist);'
assert text.count(anchor) == 1
path.write_text(text.replace(anchor, '''            Value->SetNumberField(TEXT("braking_factor"), Movement->BrakingFrictionFactor);
''' + anchor))
assert (source / relative).read_bytes() == original
(root / 'reader-source.json').write_text(json.dumps(dict(
    canonical_sha256=hashlib.sha256(original).hexdigest(),
    diagnostic_sha256=hashlib.sha256(path.read_bytes()).hexdigest(),
    diagnostic_header_sha256=hashlib.sha256(path.with_name('AlsAuditBlockingHitObserver.h').read_bytes()).hexdigest()), indent=2) + '\n')
print('Prepared native braking factor before movement and after the world timer', flush=True)
