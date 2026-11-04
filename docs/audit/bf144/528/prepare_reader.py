import hashlib
import json
from pathlib import Path

root = Path(__file__).resolve().parent
previous = root.parent / '526' / 'prepare_reader.py'
script = previous.read_text().replace('tools/tmp/bf144/526', 'tools/tmp/bf144/528')
exec(compile(script, previous.name, 'exec'))
path = Path('tools/tmp/bf144/528/reader-source') / relative
header = path.with_name('AlsAuditBlockingHitObserver.h')
header_text = header.read_text()
anchor = '    TArray<FHitResult> Hits;'
assert header_text.count(anchor) == 1
header.write_text(header_text.replace(anchor, anchor + '''
    TArray<FHitResult> Landings;

    UFUNCTION()
    void CaptureLanded(const FHitResult& Hit)
    {
        Landings.Add(Hit);
    }'''))
text = path.read_text()
for anchor, replacement in (
    ('            HitObserver->Hits.Reset();', '''            HitObserver->Hits.Reset();
            HitObserver->Landings.Reset();
            if (bObserveHits) { Character->LandedDelegate.AddDynamic(HitObserver.Get(), &UAlsAuditBlockingHitObserver::CaptureLanded); }'''),
    ('            World->Tick(LEVELTICK_All, static_cast<float>(Delta));',
     '''            World->Tick(LEVELTICK_All, static_cast<float>(Delta));
            if (bObserveHits) { Character->LandedDelegate.RemoveDynamic(HitObserver.Get(), &UAlsAuditBlockingHitObserver::CaptureLanded); }'''),
    ('                const auto CaptureFloorQuery =',
     (root / 'floor_hit.inc').read_text() + '                const auto CaptureFloorQuery ='),
    ('                    Value->SetStringField(TEXT("component"), GetNameSafe(Floor.HitResult.GetComponent()));',
     '''                    Value->SetStringField(TEXT("component"), GetNameSafe(Floor.HitResult.GetComponent()));
                    Value->SetStringField(TEXT("query_movement_mode"), UEnum::GetValueAsString(Movement->MovementMode.GetValue()));
                    Value->SetObjectField(TEXT("floor_hit"), CaptureLandingHit(Floor.HitResult));
                    UCharacterMovementComponent* MutableMovement = Character->GetCharacterMovement();
                    const bool SavedForceFloor = MutableMovement->bForceNextFloorCheck;
                    ON_SCOPE_EXIT { MutableMovement->bForceNextFloorCheck = SavedForceFloor; };
                    Value->SetBoolField(TEXT("valid_landing_spot"), Movement->IsValidLandingSpot(Location, Floor.HitResult));'''),
    ('                Row->SetArrayField(TEXT("blocking_hits"), BlockingHits);',
     '''                Row->SetArrayField(TEXT("blocking_hits"), BlockingHits);
                TArray<TSharedPtr<FJsonValue>> Landings;
                for (const FHitResult& Hit : HitObserver->Landings)
                {
                    Landings.Add(MakeShared<FJsonValueObject>(CaptureLandingHit(Hit)));
                }
                Row->SetArrayField(TEXT("landed_events"), Landings);'''),
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
print('Prepared read-only floor qualifiers and actual Landed delegate capture', flush=True)
