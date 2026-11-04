import hashlib
import json
from pathlib import Path

root=Path(__file__).resolve().parent
previous=root.parent/'537'/'prepare_reader.py'
script=previous.read_text().replace('tools/tmp/bf144/537','tools/tmp/bf144/541')
for name in ('native_fin.inc','down_miss.inc'):
    script=script.replace(f"(root/'{name}')",f"(root.parent/'537'/'{name}')")
exec(compile(script,previous.name,'exec'))
path=Path('tools/tmp/bf144/541/reader-source')/relative
text=path.read_text()
for anchor,replacement in (
    ('        FActorSpawnParameters Spawn;\n        Spawn.SpawnCollisionHandlingOverride = ESpawnActorCollisionHandlingMethod::AlwaysSpawn;\n        FObservedPose HeadObservation',
     (root/'native_slope.inc').read_text()+'        FActorSpawnParameters Spawn;\n        Spawn.SpawnCollisionHandlingOverride = ESpawnActorCollisionHandlingMethod::AlwaysSpawn;\n        FObservedPose HeadObservation'),
    ('                Row->SetArrayField(TEXT("mode_events"), ModeEvents);',
     '                Row->SetArrayField(TEXT("mode_events"), ModeEvents);\n'+(root/'down_replay.inc').read_text()),
):
    assert text.count(anchor)==1,anchor
    text=text.replace(anchor,replacement)
data=text.encode()
path.write_bytes(data)
assert (source/relative).read_bytes()==original
(root/'reader-source.json').write_text(json.dumps(dict(
    canonical_sha256=hashlib.sha256(original).hexdigest(),diagnostic_sha256=hashlib.sha256(data).hexdigest(),
    diagnostic_header_sha256=hashlib.sha256(header.read_bytes()).hexdigest()),indent=2)+'\n')
print('Prepared native steep StepDown slopes and the same-scene down replay',flush=True)
