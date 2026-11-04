import hashlib
import json
from pathlib import Path

root = Path(__file__).resolve().parent
previous = root.parent / '553' / 'prepare_reader.py'
script = previous.read_text().replace('tools/tmp/bf144/553', 'tools/tmp/bf144/570')
exec(compile(script, previous.name, 'exec'))

text = path.read_text()
for anchor, replacement in (
    ('        if ((*Case)->HasField(TEXT("landing_deck_top_cm")))', '''        bool bSimulatedLandingDeck;
        bool bEnableLandingImpact;
        if (!(*Case)->TryGetBoolField(TEXT("simulate_landing_deck"), bSimulatedLandingDeck) ||
            !(*Case)->TryGetBoolField(TEXT("enable_impact"), bEnableLandingImpact)) { return nullptr; }
        UBoxComponent* LandingDeck = nullptr;
        if ((*Case)->HasField(TEXT("landing_deck_top_cm")))'''),
    ('DeckBox->SetMobility(EComponentMobility::Static);', 'DeckBox->SetMobility(bSimulatedLandingDeck ? EComponentMobility::Movable : EComponentMobility::Static);'),
    ('            DeckBox->RegisterComponent();', '''            DeckBox->RegisterComponent();
            LandingDeck = DeckBox;
            if (bSimulatedLandingDeck)
            {
                DeckBox->SetEnableGravity(false);
                DeckBox->SetMassOverrideInKg(NAME_None, 100.0f, true);
                DeckBox->SetSimulatePhysics(true);
                DeckBox->WakeAllRigidBodies();
            }'''),
    ('            TSharedPtr<FJsonObject> ControllerBefore;',
     (root.parent / '523' / 'native_initial.inc').read_text() + '            TSharedPtr<FJsonObject> ControllerBefore;'),
    ('        World->BeginPlay();', '''        Character->GetCharacterMovement()->StandingDownwardForceScale = 0.0f;
        Character->GetCharacterMovement()->bEnablePhysicsInteraction = bEnableLandingImpact;
        World->BeginPlay();'''),
    ('            World->Tick(LEVELTICK_All, static_cast<float>(Delta));', '''            const FVector DeckVelocityBefore = LandingDeck->GetPhysicsLinearVelocity();
            const FVector DeckPositionBefore = LandingDeck->GetComponentLocation();
            World->Tick(LEVELTICK_All, static_cast<float>(Delta));'''),
    ('            Row->SetNumberField(TEXT("frame"), Rows.Num());', '''            Row->SetNumberField(TEXT("frame"), Rows.Num());
            TSharedRef<FJsonObject> DeckSample = MakeShared<FJsonObject>();
            DeckSample->SetBoolField(TEXT("simulates"), LandingDeck->IsSimulatingPhysics());
            DeckSample->SetBoolField(TEXT("interaction"), Character->GetCharacterMovement()->bEnablePhysicsInteraction);
            DeckSample->SetNumberField(TEXT("standing_force_scale"), Character->GetCharacterMovement()->StandingDownwardForceScale);
            DeckSample->SetNumberField(TEXT("mass_kg"), LandingDeck->GetMass());
            DeckSample->SetObjectField(TEXT("velocity_before_cm_s"), CaptureVector(DeckVelocityBefore));
            DeckSample->SetObjectField(TEXT("velocity_after_cm_s"), CaptureVector(LandingDeck->GetPhysicsLinearVelocity()));
            DeckSample->SetObjectField(TEXT("position_before_cm"), CaptureVector(DeckPositionBefore));
            DeckSample->SetObjectField(TEXT("position_after_cm"), CaptureVector(LandingDeck->GetComponentLocation()));
            Row->SetObjectField(TEXT("landing_body"), DeckSample);'''),
):
    assert text.count(anchor) == 1, anchor
    text = text.replace(anchor, replacement)
path.write_text(text)
assert (source / relative).read_bytes() == original
(root / 'reader-source.json').write_text(json.dumps(dict(
    canonical_sha256=hashlib.sha256(original).hexdigest(),
    diagnostic_sha256=hashlib.sha256(path.read_bytes()).hexdigest(),
    diagnostic_header_sha256=hashlib.sha256(path.with_name('AlsAuditBlockingHitObserver.h').read_bytes()).hexdigest()), indent=2) + '\n')
print('Prepared native landing deck velocity with standing force disabled', flush=True)
