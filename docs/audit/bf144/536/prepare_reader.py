import hashlib
import json
import re
from pathlib import Path

root=Path(__file__).resolve().parent
previous=root.parent/'534'/'prepare_reader.py'
script=previous.read_text().replace('tools/tmp/bf144/534','tools/tmp/bf144/536')
script=script.replace("(root/'perch_distance.inc')","(root.parent/'534'/'perch_distance.inc')")
exec(compile(script,previous.name,'exec'))
path=Path('tools/tmp/bf144/536/reader-source')/relative
text=path.read_text()
text,count=re.subn(r'^                if \(Rows.Num\(\) == 40\) \{ Row->SetObjectField\(TEXT\("walking_first_floor"\).*?\n','',text,flags=re.M)
assert count==1
for anchor,replacement in (
    ('                TArray<TSharedPtr<FJsonValue>> BlockingHits;',
     (root/'step_down_floor.inc').read_text()+'                TArray<TSharedPtr<FJsonValue>> BlockingHits;'),
    ('                    Value->SetObjectField(TEXT("floor_at_move_stop"), CaptureFloorQuery(MoveStop));',
     '''                    Value->SetObjectField(TEXT("floor_at_move_stop"), CaptureFloorQuery(MoveStop));
                    const FVector Travel = Hit.TraceEnd - Hit.TraceStart;
                    if (Movement->GetGravitySpaceZ(Travel) < 0 &&
                        Movement->ProjectToGravityFloor(Travel).SizeSquared() <= UE_KINDA_SMALL_NUMBER)
                    {
                        FFindFloorResult Reused;
                        UCharacterMovementComponent* MutableMovement = Character->GetCharacterMovement();
                        const bool SavedForceFloor = MutableMovement->bForceNextFloorCheck;
                        ON_SCOPE_EXIT { MutableMovement->bForceNextFloorCheck = SavedForceFloor; };
                        Movement->FindFloor(MoveStop, Reused, false, &Hit);
                        Value->SetObjectField(TEXT("reused_down_hit_floor"), CaptureActualFloor(Reused));
                    }'''),
):
    assert text.count(anchor)==1
    text=text.replace(anchor,replacement)
data=text.encode()
path.write_bytes(data)
assert (source/relative).read_bytes()==original
(root/'reader-source.json').write_text(json.dumps(dict(
    canonical_sha256=hashlib.sha256(original).hexdigest(),diagnostic_sha256=hashlib.sha256(data).hexdigest(),
    diagnostic_header_sha256=hashlib.sha256(header.read_bytes()).hexdigest()),indent=2)+'\n')
print('Prepared native actual CurrentFloor and StepDown hit reuse observation',flush=True)
