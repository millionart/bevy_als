import hashlib
import json
from pathlib import Path

root = Path(__file__).resolve().parent
previous = root.parent / '523' / 'prepare_reader.py'
script = previous.read_text().replace('tools/tmp/bf144/523', 'tools/tmp/bf144/524')
script = script.replace('(root / name).read_text()', "(root.parent / '523' / name).read_text()")
exec(compile(script, previous.name, 'exec'))
path = Path('tools/tmp/bf144/524/reader-source') / relative
text = path.read_text()
anchor = '            Barrier->SetWorldLocationAndRotation(FVector(SpawnXY.X + WallX, SpawnXY.Y, 100), FRotator(0, 45, 0));'
assert text.count(anchor) == 1
text = text.replace(anchor, '''            double WallPitch = 0;
            if ((*Case)->HasField(TEXT("wall_pitch_degrees")) &&
                (!(*Case)->HasTypedField<EJson::Number>(TEXT("wall_pitch_degrees")) ||
                 !(*Case)->TryGetNumberField(TEXT("wall_pitch_degrees"), WallPitch) ||
                 !FMath::IsFinite(WallPitch))) { return nullptr; }
            Barrier->SetWorldLocationAndRotation(FVector(SpawnXY.X + WallX, SpawnXY.Y, 100), FRotator(WallPitch, 45, 0));''')
data = text.encode()
path.write_bytes(data)
assert (source / relative).read_bytes() == original
(root / 'reader-source.json').write_text(json.dumps(dict(
    canonical_sha256=hashlib.sha256(original).hexdigest(),
    diagnostic_sha256=hashlib.sha256(data).hexdigest()), indent=2) + '\n')
print('Prepared isolated pitch parameter for the native wall fixture', flush=True)
