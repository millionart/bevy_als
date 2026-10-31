import hashlib
import json
import os
from pathlib import Path
r=Path(__file__).resolve().parent
read=lambda p:json.loads(Path(p).read_text())
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
base=read('docs/audit/bf144/441b/identities.start.json');reader='tools/ue_graph_source_reader/Source/AlsGraphSourceReader/Private/AlsGraphSourceReaderCommandlet.cpp';base[reader]=read('docs/audit/bf144/442b0/reader-migration.json')[reader]['after_sha256']
config=read('tools/env.local.json');plugin=Path(config['ue_project']).parent/'Plugins/ALS-Refactored';engine=Path(os.environ['ALS_UE_SOURCE_ROOT'])
for i,(key,digest)in enumerate(base.items()):
    if i%64==0:print('Checked current source identity batch '+str(i),flush=True)
    if key.startswith('source:'):p=plugin/'Source/ALS'/key.split(':',1)[1]
    elif key.startswith('target:'):p=Path('src')/key.split(':',1)[1]
    elif key.startswith('converter:'):p=Path('tools/import')/key.split(':',1)[1]
    elif key.startswith('source_asset:'):p=plugin/'Content'/key.split(':',1)[1]
    elif key.startswith('Engine/'):p=engine/key
    else:p=Path(key)
    assert sha(p)==digest,key
for name,row in read('docs/audit/bf144/441c/library-identities.json').items():assert sha(name)==row['sha256'],name
units=[(plugin/'Source/ALS/Public/Utility/AlsVector.h','source:Source/ALS/Public/Utility/AlsVector.h',[(114,158)]),(plugin/'Source/ALSExtras/Private/AlsCharacterExample.cpp','source:Source/ALSExtras/Private/AlsCharacterExample.cpp',[(141,158)]),(engine/'Engine/Source/Runtime/Core/Public/Math/UnrealMathUtility.h','Engine/Source/Runtime/Core/Public/Math/UnrealMathUtility.h',[(129,153),(760,798)]),(engine/'Engine/Source/Runtime/Core/Public/Math/Vector.h','Engine/Source/Runtime/Core/Public/Math/Vector.h',[(1903,1923)]),(Path('tools/import/coordinate_basis.py'),'tools/import/coordinate_basis.py',[(1,45)]),(Path('src/als/character/movement/motion.rs'),'src/als/character/movement/motion.rs',[(204,216)]),(Path('src/als/character/movement/dynamics.rs'),'src/als/character/movement/dynamics.rs',[(156,190)])]
excerpts=[]
for p,key,ranges in units:
 lines=p.read_text().splitlines();excerpts.append({'key':key,'sha256':sha(p),'ranges':[{'start':a,'end':b,'text':'\n'.join(lines[a-1:b])}for a,b in ranges]})
(r/'source-facts.json').write_text(json.dumps(excerpts,indent=2)+'\n');report={'gate':'PASS_DIRECTION_INPUT_SOURCE_AND_LIBRARY_IDENTITIES','current_base_files_unchanged':len(base),'reader_migration_from_442b0_respected':True,'same_six_actual_libraries_unchanged':True,'source_fact_units':len(excerpts),'matrix_final_source_sha256':sha(r/'matrix.rs'),'matrix_source_bytes_restored':(r/'matrix.rs').read_bytes()==(r/'matrix.final.rs').read_bytes()}
(r/'identity-check.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report),flush=True)
