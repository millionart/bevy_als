import hashlib
import json
from pathlib import Path

root = Path(__file__).resolve().parent
previous = root.parent / '566' / 'prepare_reader.py'
script = previous.read_text().replace('tools/tmp/bf144/566', 'tools/tmp/bf144/567')
script = script.replace("(root/'native_base.inc')", "(root.parent/'566'/'native_base.inc')")
script = script.replace("(root/'native_velocity.inc')", "(root.parent/'566'/'native_velocity.inc')")
exec(compile(script, previous.name, 'exec'))

text = path.read_text()
for anchor, replacement in (
    ('Box->SetMobility(bDynamicBase ? EComponentMobility::Movable : EComponentMobility::Static);',
     'Box->SetMobility(EComponentMobility::Movable);'),
    ('FVector(7, 100, 10)', 'FVector(32, 100, 10)'),
    ('SpawnXY.X + 127', 'SpawnXY.X + 152'),
    ('        bool bAuditStepMoves;', '''        bool bMoveOldBaseLate;
        if (!(*Case)->TryGetBoolField(TEXT("late_old_base_move"), bMoveOldBaseLate)) { return nullptr; }
        bool bAuditStepMoves;'''),
    ('            World->Tick(LEVELTICK_All, static_cast<float>(Delta));', (root / 'native_late.inc').read_text()),
    ('            Row->SetBoolField(TEXT("based_on_floor"), Character->GetMovementBase() == Box);',
     '''            if (LateBase.IsValid()) { Row->SetObjectField(TEXT("late_old_base"), LateBase); }
            Row->SetBoolField(TEXT("based_on_floor"), Character->GetMovementBase() == Box);'''),
):
    assert text.count(anchor) == 1, anchor
    text = text.replace(anchor, replacement)
path.write_text(text)
header = path.with_name('AlsAuditBlockingHitObserver.h')
text = header.read_text()
anchor = '    TArray<FHitResult> Hits;'
assert text.count(anchor) == 1
header.write_text(text.replace(anchor, '''    TFunction<void()> AuditAfterMovement;
    UFUNCTION()
    void CaptureAfterMovement(float Delta, FVector OldLocation, FVector OldVelocity)
    {
        if (AuditAfterMovement) { AuditAfterMovement(); }
    }

''' + anchor))
assert (source / relative).read_bytes() == original
(root / 'reader-source.json').write_text(json.dumps(dict(
    canonical_sha256=hashlib.sha256(original).hexdigest(),
    diagnostic_sha256=hashlib.sha256(path.read_bytes()).hexdigest(),
    diagnostic_header_sha256=hashlib.sha256(header.read_bytes()).hexdigest()), indent=2) + '\n')
print('Prepared old-base transform change in the native movement update callback', flush=True)
