import hashlib
import json
from pathlib import Path

root = Path(__file__).resolve().parent
previous = root.parent / '571' / 'prepare_reader.py'
script = previous.read_text().replace('tools/tmp/bf144/571', 'tools/tmp/bf144/572')
exec(compile(script, previous.name, 'exec'))

text = path.read_text()
anchor = '        FActorSpawnParameters Spawn;\n        Spawn.SpawnCollisionHandlingOverride = ESpawnActorCollisionHandlingMethod::AlwaysSpawn;\n        FObservedPose HeadObservation'
assert text.count(anchor) == 1
path.write_text(text.replace(anchor, (root / 'native_floor.inc').read_text() + anchor))
assert (source / relative).read_bytes() == original
(root / 'reader-source.json').write_text(json.dumps(dict(
    canonical_sha256=hashlib.sha256(original).hexdigest(),
    diagnostic_sha256=hashlib.sha256(path.read_bytes()).hexdigest(),
    diagnostic_header_sha256=hashlib.sha256(path.with_name('AlsAuditBlockingHitObserver.h').read_bytes()).hexdigest()), indent=2) + '\n')
print('Prepared the source lower flat floor and actual native pose boundaries', flush=True)
