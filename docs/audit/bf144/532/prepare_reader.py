import hashlib
import json
from pathlib import Path

root=Path(__file__).resolve().parent
previous=root.parent/'531'/'prepare_reader.py'
script=previous.read_text().replace('tools/tmp/bf144/531','tools/tmp/bf144/532')
script=script.replace("(root/'perch_query.inc')","(root.parent/'531'/'perch_query.inc')")
exec(compile(script,previous.name,'exec'))
path=Path('tools/tmp/bf144/532/reader-source')/relative
text=path.read_text()
anchor='            DeckBox->SetWorldLocation(FVector(SpawnXY.X + 239, SpawnXY.Y - 30, Top - 10));'
assert text.count(anchor)==1
text=text.replace(anchor,'''            double DeckX = 239;
            if ((*Case)->HasField(TEXT("landing_deck_x_cm")) &&
                (!(*Case)->HasTypedField<EJson::Number>(TEXT("landing_deck_x_cm")) ||
                 !(*Case)->TryGetNumberField(TEXT("landing_deck_x_cm"), DeckX) ||
                 !FMath::IsFinite(DeckX))) { return nullptr; }
            DeckBox->SetWorldLocation(FVector(SpawnXY.X + DeckX, SpawnXY.Y - 30, Top - 10));''')
data=text.encode()
path.write_bytes(data)
assert (source/relative).read_bytes()==original
(root/'reader-source.json').write_text(json.dumps(dict(
    canonical_sha256=hashlib.sha256(original).hexdigest(),
    diagnostic_sha256=hashlib.sha256(data).hexdigest(),
    diagnostic_header_sha256=hashlib.sha256(header.read_bytes()).hexdigest()),indent=2)+'\n')
print('Prepared the native deck offset as fixture data',flush=True)
