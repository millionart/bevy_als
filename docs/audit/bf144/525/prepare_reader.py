import hashlib
import json
from pathlib import Path

root = Path(__file__).resolve().parent
previous = root.parent / '524' / 'prepare_reader.py'
script = previous.read_text().replace('tools/tmp/bf144/524', 'tools/tmp/bf144/525')
exec(compile(script, previous.name, 'exec'))
root = Path(__file__).resolve().parent
path = Path('tools/tmp/bf144/525/reader-source') / relative
header = path.with_name('AlsAuditBlockingHitObserver.h')
header.write_bytes((root / header.name).read_bytes())
text = path.read_text()
for anchor, replacement in (
    ('#include "AlsGraphSourceReaderCommandlet.h"',
     '#include "AlsGraphSourceReaderCommandlet.h"\n#include "AlsAuditBlockingHitObserver.h"\n#include "Components/CapsuleComponent.h"\n#include "UObject/StrongObjectPtr.h"'),
    ('NewObject<UBoxComponent>(Wall)', 'NewObject<UBoxComponent>(Wall, TEXT("AuditWall"))'),
    ('NewObject<UBoxComponent>(Deck)', 'NewObject<UBoxComponent>(Deck, TEXT("AuditLandingDeck"))'),
    ('        const auto CaptureMovement = [&]()', '''        TStrongObjectPtr<UAlsAuditBlockingHitObserver> HitObserver(NewObject<UAlsAuditBlockingHitObserver>());
        ON_SCOPE_EXIT { Character->GetCapsuleComponent()->OnComponentHit.RemoveDynamic(HitObserver.Get(), &UAlsAuditBlockingHitObserver::Capture); };
        const auto CaptureMovement = [&]()'''),
    ('            World->Tick(LEVELTICK_All, static_cast<float>(Delta));', '''            bool bObserveHits = false;
            if ((*Frame)->HasField(TEXT("observe_blocking_hits")) &&
                !(*Frame)->TryGetBoolField(TEXT("observe_blocking_hits"), bObserveHits)) { return nullptr; }
            HitObserver->Hits.Reset();
            if (bObserveHits) { Character->GetCapsuleComponent()->OnComponentHit.AddDynamic(HitObserver.Get(), &UAlsAuditBlockingHitObserver::Capture); }
            World->Tick(LEVELTICK_All, static_cast<float>(Delta));
            if (bObserveHits) { Character->GetCapsuleComponent()->OnComponentHit.RemoveDynamic(HitObserver.Get(), &UAlsAuditBlockingHitObserver::Capture); }'''),
    ('            Row->SetObjectField(TEXT("movement_after_tick"), CaptureMovement());',
     '            Row->SetObjectField(TEXT("movement_after_tick"), CaptureMovement());\n' + (root / 'capture_hits.inc').read_text()),
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
print('Prepared native scoped blocking-hit and read-only floor observation', flush=True)
