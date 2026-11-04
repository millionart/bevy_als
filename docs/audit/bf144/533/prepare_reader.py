import hashlib
import json
from pathlib import Path

root=Path(__file__).resolve().parent
previous=root.parent/'532'/'prepare_reader.py'
script=previous.read_text().replace('tools/tmp/bf144/532','tools/tmp/bf144/533')
exec(compile(script,previous.name,'exec'))
path=Path('tools/tmp/bf144/533/reader-source')/relative
header=path.with_name('AlsAuditBlockingHitObserver.h')
text=header.read_text()
anchor='    TArray<FHitResult> Hits;'
assert text.count(anchor)==1
text=text.replace('#include "Engine/HitResult.h"',
    '#include "Engine/HitResult.h"\n#include "GameFramework/Character.h"\n#include "GameFramework/CharacterMovementComponent.h"')
header.write_text(text.replace(anchor,anchor+'\n'+(root/'mode_observer.inc').read_text()))
text=path.read_text()
for anchor,replacement in (
    ('falling_initial_cm','walking_initial_cm'),
    ('                FallingMovement->SetMovementMode(MOVE_Falling);',
     '''                if (!FallingMovement->IsMovingOnGround() || !FallingMovement->CurrentFloor.IsWalkableFloor()) { return nullptr; }
                FallingMovement->FindFloor(Character->GetActorLocation(), FallingMovement->CurrentFloor, false);
                if (!FallingMovement->CurrentFloor.IsWalkableFloor()) { return nullptr; }
                FallingMovement->SetBaseFromFloor(FallingMovement->CurrentFloor);'''),
    ('            HitObserver->Landings.Reset();',
     '''            HitObserver->Landings.Reset();
            HitObserver->Modes.Reset();
            if (bObserveHits) { Character->MovementModeChangedDelegate.AddDynamic(HitObserver.Get(), &UAlsAuditBlockingHitObserver::CaptureMode); }'''),
    ('            World->Tick(LEVELTICK_All, static_cast<float>(Delta));',
     '''            World->Tick(LEVELTICK_All, static_cast<float>(Delta));
            if (bObserveHits) { Character->MovementModeChangedDelegate.RemoveDynamic(HitObserver.Get(), &UAlsAuditBlockingHitObserver::CaptureMode); }'''),
    ('const auto CaptureFloorQuery = [&](const FVector& Location)',
     'const auto CaptureFloorQuery = [&](const FVector& Location, bool bWalkingRange = false)'),
    ('                    const float MaxFloor = FMath::Max(UCharacterMovementComponent::MAX_FLOOR_DIST,',
     '''                    const bool bWalking = bWalkingRange || Movement->IsMovingOnGround();
                    Value->SetBoolField(TEXT("walking_range"), bWalking);
                    const float MaxFloor = FMath::Max(UCharacterMovementComponent::MAX_FLOOR_DIST,'''),
    ('Movement->MaxStepHeight + (Movement->IsMovingOnGround() ?','Movement->MaxStepHeight + (bWalking ?'),
    ('const float MaxPerch = MaxFloor + (Movement->IsMovingOnGround() ?','const float MaxPerch = MaxFloor + (bWalking ?'),
    ('                Row->SetArrayField(TEXT("landed_events"), Landings);',
     '''                Row->SetArrayField(TEXT("landed_events"), Landings);
                TArray<TSharedPtr<FJsonValue>> ModeEvents;
                for (const auto& Event : HitObserver->Modes)
                {
                    TSharedRef<FJsonObject> Change = MakeShared<FJsonObject>();
                    Change->SetStringField(TEXT("previous"), UEnum::GetValueAsString(Event.Previous));
                    Change->SetStringField(TEXT("current"), UEnum::GetValueAsString(Event.Current));
                    Change->SetObjectField(TEXT("location_cm"), CaptureVector(Event.Location));
                    Change->SetObjectField(TEXT("velocity_cm_s"), CaptureVector(Event.Velocity));
                    Change->SetObjectField(TEXT("walking_floor"), CaptureFloorQuery(Event.Location, true));
                    ModeEvents.Add(MakeShared<FJsonValueObject>(Change));
                }
                Row->SetArrayField(TEXT("mode_events"), ModeEvents);'''),
):
    expected=2 if anchor=='falling_initial_cm' else 1
    assert text.count(anchor)==expected,anchor
    text=text.replace(anchor,replacement)
data=text.encode()
path.write_bytes(data)
assert (source/relative).read_bytes()==original
(root/'reader-source.json').write_text(json.dumps(dict(
    canonical_sha256=hashlib.sha256(original).hexdigest(),diagnostic_sha256=hashlib.sha256(data).hexdigest(),
    diagnostic_header_sha256=hashlib.sha256(header.read_bytes()).hexdigest()),indent=2)+'\n')
print('Prepared isolated native Walking mode handoff observation',flush=True)
