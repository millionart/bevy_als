import hashlib
import json
from pathlib import Path

root = Path(__file__).resolve().parent
previous = root.parent / '553' / 'prepare_reader.py'
script = previous.read_text().replace('tools/tmp/bf144/553', 'tools/tmp/bf144/564')
exec(compile(script, previous.name, 'exec'))
root = Path(__file__).resolve().parent
path = Path('tools/tmp/bf144/564/reader-source') / relative
text = path.read_text()
for anchor, name in (
    ('            TSharedPtr<FJsonObject> ControllerBefore;\n            if (bLocalController)', 'native_initial.inc'),
):
    assert text.count(anchor) == 1
    snippet = root.parent / '523' / name if name == 'native_initial.inc' else root / name
    text = text.replace(anchor, snippet.read_text() + anchor)
anchor = '            Value->SetNumberField(TEXT("max_acceleration_cm_s2"), Movement->GetMaxAcceleration());'
assert text.count(anchor) == 1
text = text.replace(anchor, anchor + '\n' + (root/'native_speed.inc').read_text().rstrip())
data = text.encode()
path.write_bytes(data)
assert (source / relative).read_bytes() == original
(root / 'reader-source.json').write_text(json.dumps(dict(
    canonical_sha256=hashlib.sha256(original).hexdigest(),
    diagnostic_sha256=hashlib.sha256(data).hexdigest(),
    diagnostic_header_sha256=hashlib.sha256(path.with_name('AlsAuditBlockingHitObserver.h').read_bytes()).hexdigest()), indent=2) + '\n')
print('Prepared native actual stance and cached maximum speed readout', flush=True)
