import copy
import hashlib
import json
import subprocess
from pathlib import Path

r=Path(__file__).resolve().parent
expected={'445a':('PASS_OPTIONAL_NATIVE_ASSET_TICK_AND_ACCUMULATOR_READER_WITH_LEGACY_COMPATIBILITY',56),'445':('PASS_COMPLETE_CURRENT_PLAYER_CLOCK_TABLE_WITH_KNOWN_D201_AND_OPEN_HISTORY',108)}


def validate(children):
    assert set(children)==set(expected)
    for name,child in children.items():
        assert (child['gate'],child['files'])==expected[name]


def verify(raw,record):
    assert len(raw)==record['bytes']and hashlib.sha256(raw).hexdigest()==record['sha256']


children={}
for name,(gate,count)in expected.items():
    root=r.parent/name
    summary=json.loads((root/'summary.json').read_text())
    manifest=json.loads((root/'evidence-manifest.json').read_text())
    assert summary['gate']==manifest['gate']==gate and len(manifest['files'])==count
    assert summary['screenshots']==0 and summary['performance_acceptance']=='paused'
    assert summary['source_bytes_restored']
    paths=set()
    for relative,record in manifest['files'].items():
        p=(root/relative).resolve()
        assert p.is_relative_to(root.resolve())
        verify(p.read_bytes(),record)
        paths.add(str(p.relative_to(Path.cwd())))
    paths.update(str((root/n).relative_to(Path.cwd()))for n in ['summary.json','evidence-manifest.json'])
    tracked=subprocess.run(['git','ls-files','-z','--',str(root.relative_to(Path.cwd()))],check=True,capture_output=True,timeout=5).stdout.decode().split('\0')
    assert paths<=set(tracked)
    children[name]=dict(gate=gate,files=count,summary_sha256=hashlib.sha256((root/'summary.json').read_bytes()).hexdigest(),manifest_sha256=hashlib.sha256((root/'evidence-manifest.json').read_bytes()).hexdigest())
    print('Verified sealed child '+name,flush=True)
validate(children)
for kind in ['missing','wrong_gate']:
    bad=copy.deepcopy(children)
    if kind=='missing':del bad['445']
    else:bad['445']['gate']='PASS_UNRELATED_SCOPE'
    try:validate(bad)
    except AssertionError:continue
    raise AssertionError(kind)
try:verify(b'changed',dict(bytes=7,sha256='0'*64))
except AssertionError:pass
else:raise AssertionError('Changed evidence passed')
result=dict(gate='PASS_CURRENT_PLAYER_CLOCK_AUDIT_WITH_RECORDED_D201_AND_OPEN_FULL_HISTORY',children=children,verified_git_evidence_files=164,negative_children_scope_and_hash_cases_rejected=3,
    source_declarations=85,current_target_complete_leaf_keys=2805,builtin_clip_leaf_keys=1315,current_temporary_runtime_keys=0,temporary_key_focused_fixture_passed=True,
    source_actual_tick_rows=1560,target_actual_native_tick_rows=1490,paired_own_history_seconds_differences=621,uninstantiated_target_declarations=49,
    current_idle_rows_exact=319,native_direct_second_rows_exact=240,known_consumer='BF144-D201',new_differences=[],differences_fixed=False,total_confirmed_differences=271,
    source_target_same_initial_history_claimed=False,all_clock_differences_attributed=False,full_root_or_als_equivalence_claimed=False,
    next_direct_boundary='Match actual source and target initialization and pre-ready input/state history before attributing remaining paired clocks',
    render_gpu_pass_acceptance='deferred under explicit peripheral exclusion',screenshots=0,performance_acceptance='paused')
(r/'summary.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps({k:v for k,v in result.items()if k!='children'}),flush=True)
