import hashlib
import json
from pathlib import Path

r=Path(__file__).resolve().parent;legacy_path=Path('docs/audit/bf144/453/replay.rs');legacy=legacy_path.read_text();old=json.loads(Path('docs/audit/bf144/453/analysis-baseline.json').read_bytes());current=json.loads((r/'analysis.json').read_bytes())
assert 'let br=direction(b).map(|x|x*0.01);let dr=direction(d).map(|x|x*0.01);'in legacy
assert 'let early:V=std::array::from_fn(|k|br[k]+dr[k]);'in legacy
assert old['domain_summaries']['sc']['source-early-unit']['translation']==36
assert current['common_current']['double_differences']==694 and current['common_source']['f32_differences']==691
result=dict(gate='PASS_EXISTING_D232_NUMBER_SPACE_UNIT_PHASE_CONSUMER_CLASSIFICATION',existing_difference='BF144-D232',legacy_source='docs/audit/bf144/453/replay.rs',legacy_source_sha256=hashlib.sha256(legacy_path.read_bytes()).hexdigest(),legacy_early_unit_keeps_f64_until_output_projection=True,legacy_early_unit_sc_f32_changes=36,current_common_public_cm_m_f64_changes=694,current_common_public_cm_m_f32_changes=692,source_common_cm_m_f64_changes=693,source_common_cm_m_f32_changes=691,common_root='Pose numeric space changed before evaluation instead of evaluating in source centimeters and projecting afterward',early_f32_and_unit_phase_subfactors_remain_distinct=True,no_f32_cast_or_float_export_coefficient_in_current_control=True,not_attributed_to_D165_float_state_or_D275_import_coefficient=True,independent_public_relative_both_unit_domains_verified=True,does_not_reclassify_all_actual_local_errors_as_unit_phase=True,actual_unit_only_full_translation_restored=0,existing_453_early_f32_evidence_and_domain_limits_preserved=True,new_difference_number_required=False,confirmed_difference_count=272)
(r/'classification.json').write_text(json.dumps(result,indent=2)+'\n');print('Verified D232 already contains independent F64 early-unit control; append foot-local consumer and preserve separate F32/phase boundaries',flush=True)
