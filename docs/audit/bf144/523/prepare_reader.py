import hashlib
import json
from pathlib import Path

root = Path(__file__).resolve().parent
previous = root.parent / '519' / 'prepare_reader.py'
script = previous.read_text().replace('tools/tmp/bf144/519', 'tools/tmp/bf144/523')
script = script.replace("(root/'native_wall.inc')", "(root.parent/'519'/'native_wall.inc')")
exec(compile(script, previous.name, 'exec'))
root = Path(__file__).resolve().parent
path = Path('tools/tmp/bf144/523/reader-source') / relative
text = path.read_text()
for anchor, name in (
    ('        FActorSpawnParameters Spawn;\n        Spawn.SpawnCollisionHandlingOverride = ESpawnActorCollisionHandlingMethod::AlwaysSpawn;\n        FObservedPose HeadObservation', 'native_deck.inc'),
    ('            TSharedPtr<FJsonObject> ControllerBefore;\n            if (bLocalController)', 'native_initial.inc'),
):
    assert text.count(anchor) == 1
    text = text.replace(anchor, (root / name).read_text() + anchor)
data = text.encode()
path.write_bytes(data)
assert (source / relative).read_bytes() == original
(root / 'reader-source.json').write_text(json.dumps(dict(
    canonical_sha256=hashlib.sha256(original).hexdigest(),
    diagnostic_sha256=hashlib.sha256(data).hexdigest()), indent=2) + '\n')
print('Prepared isolated landing deck and falling initial conditions', flush=True)
