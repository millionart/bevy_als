import hashlib
import json
import shutil
from pathlib import Path

root = Path(__file__).resolve().parent
source = Path('tools/ue_graph_source_reader')
copy = Path('tools/tmp/bf144/516/reader-source')
shutil.copytree(source,copy,dirs_exist_ok=True)
relative = Path('Source/AlsGraphSourceReader/Private/AlsGraphSourceReaderCommandlet.cpp')
original = (source/relative).read_bytes()
text = original.decode()
anchor = '        Box->SetBoxExtent(FVector(1500, 1500, 50));'
assert text.count(anchor)==1
replacement = '''        double FloorHalfExtentX = 1500;
        if ((*Case)->HasField(TEXT("floor_half_extent_x_cm")) &&
            (!(*Case)->HasTypedField<EJson::Number>(TEXT("floor_half_extent_x_cm")) ||
             !(*Case)->TryGetNumberField(TEXT("floor_half_extent_x_cm"), FloorHalfExtentX) ||
             !FMath::IsFinite(FloorHalfExtentX) || FloorHalfExtentX <= 0)) { return nullptr; }
        Box->SetBoxExtent(FVector(FloorHalfExtentX, 1500, 50));'''
body = text.replace(anchor,replacement).encode()
(copy/relative).write_bytes(body)
assert (source/relative).read_bytes()==original
(root/'reader-source.json').write_text(json.dumps(dict(
    canonical_sha256=hashlib.sha256(original).hexdigest(),
    diagnostic_sha256=hashlib.sha256(body).hexdigest()),indent=2)+'\n')
print('Prepared isolated character reader with optional static floor width',flush=True)
