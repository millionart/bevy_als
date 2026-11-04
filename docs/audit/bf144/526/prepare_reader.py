import hashlib
import json
from pathlib import Path

root = Path(__file__).resolve().parent
previous = root.parent / '525' / 'prepare_reader.py'
script = previous.read_text().replace('tools/tmp/bf144/525', 'tools/tmp/bf144/526')
script = script.replace('(root / header.name)', "(root.parent / '525' / header.name)")
script = script.replace("(root / 'capture_hits.inc')", "(root.parent / '525' / 'capture_hits.inc')")
exec(compile(script, previous.name, 'exec'))
path = Path('tools/tmp/bf144/526/reader-source') / relative
text = path.read_text()
for anchor, replacement in (
    ('            Barrier->SetBoxExtent(FVector(2, 100, 200));', '''            double WallTop = 300;
            if ((*Case)->HasField(TEXT("wall_top_cm")) &&
                (!(*Case)->HasTypedField<EJson::Number>(TEXT("wall_top_cm")) ||
                 !(*Case)->TryGetNumberField(TEXT("wall_top_cm"), WallTop) ||
                 !FMath::IsFinite(WallTop) || WallTop <= -100)) { return nullptr; }
            Barrier->SetBoxExtent(FVector(2, 100, (WallTop + 100) * .5));'''),
    ('FVector(SpawnXY.X + WallX, SpawnXY.Y, 100), FRotator(WallPitch, 45, 0)',
     'FVector(SpawnXY.X + WallX, SpawnXY.Y, (WallTop - 100) * .5), FRotator(WallPitch, 45, 0)'),
    ('                    Value->SetBoolField(TEXT("walkable_hit"), Movement->IsWalkable(Hit));',
     '''                    const FVector MoveStop = FMath::Lerp(Hit.TraceStart, Hit.TraceEnd, static_cast<double>(Hit.Time));
                    Value->SetObjectField(TEXT("move_stop_cm"), CaptureVector(MoveStop));
                    Value->SetBoolField(TEXT("edge_within_at_move_stop"),
                        Movement->IsWithinEdgeTolerance(MoveStop, Hit.ImpactPoint,
                            Character->GetCapsuleComponent()->GetScaledCapsuleRadius()));
                    Value->SetObjectField(TEXT("floor_at_move_stop"), CaptureFloorQuery(MoveStop));
                    Value->SetBoolField(TEXT("walkable_hit"), Movement->IsWalkable(Hit));'''),
):
    assert text.count(anchor) == 1, anchor
    text = text.replace(anchor, replacement)
data = text.encode()
path.write_bytes(data)
assert (source / relative).read_bytes() == original
(root / 'reader-source.json').write_text(json.dumps(dict(
    canonical_sha256=hashlib.sha256(original).hexdigest(),
    diagnostic_sha256=hashlib.sha256(data).hexdigest(),
    diagnostic_header_sha256=hashlib.sha256(header.read_bytes()).hexdigest()), indent=2) + '\n')
print('Prepared native wall edge geometry and pulled-back impact point observations', flush=True)
