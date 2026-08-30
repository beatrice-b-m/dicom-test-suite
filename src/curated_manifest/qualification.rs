//! Payload-free compatibility projection for plan-first qualifications.

use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;
use serde_json::{Map, Value};

use super::{CuratedManifestError, err, fail};
use crate::corpus_plan::{PlannedArtifact, PlannedQualification, QualificationPayloadPolicy};
use crate::executor::adapters::{ManifestProjectionArtifact, ManifestProjectionInput};
use crate::executor::evidence::{ArtifactKind, ExecutionStatus};

const FUZZ_KIND: &str = "bounded_deterministic_fuzz";
const FUZZ_EVIDENCE_KIND: &str = "bounded_fuzz_run";
const FUZZ_CASE_ID: &str = "fuzz/parser/bounded_seed_corpus";
const FUZZ_PRODUCER_ID: &str = "bounded_deterministic_fuzz";
const EOT_KIND: &str = "checked_eot_u64_overflow";

pub(super) fn project_qualifications(
    input: &ManifestProjectionInput,
) -> Result<Vec<Value>, CuratedManifestError> {
    let mut projected = Vec::new();
    for pair in &input.artifacts {
        let PlannedArtifact::Qualification(planned) = &pair.planned else {
            continue;
        };
        validate_execution_identity(pair, planned, &input.corpus_plan_sha256)?;
        match planned.qualification_kind.as_str() {
            FUZZ_KIND => projected.push(project_fuzz(pair, planned)?),
            EOT_KIND => validate_internal_eot(pair, planned)?,
            kind => return fail(format!("unsupported curated qualification kind {kind}")),
        }
    }
    Ok(projected)
}

fn validate_execution_identity(
    pair: &ManifestProjectionArtifact,
    planned: &PlannedQualification,
    corpus_plan_sha256: &str,
) -> Result<(), CuratedManifestError> {
    let actual = &pair.execution;
    if actual.logical_id != planned.logical_id
        || actual.order != planned.order
        || actual.artifact_kind != ArtifactKind::Qualification
        || actual.corpus_plan_sha256 != corpus_plan_sha256
        || actual.instance_plan_sha256.is_some()
    {
        return fail(format!(
            "qualification plan/execution identity mismatch for {}",
            planned.logical_id
        ));
    }
    if actual.status != ExecutionStatus::Succeeded {
        return fail(format!(
            "qualification execution did not succeed for {}",
            planned.logical_id
        ));
    }
    if actual.output.is_some()
        || actual.resources.planned_output_bytes != 0
        || actual.resources.actual_output_bytes != 0
        || planned.resources.output_bytes != 0
    {
        return fail(format!(
            "qualification retained a payload for {}",
            planned.logical_id
        ));
    }
    let materialization = actual.materialization.as_ref().ok_or_else(|| {
        err(format!(
            "qualification materialization evidence is absent for {}",
            planned.logical_id
        ))
    })?;
    if !materialization.completed || materialization.service_evidence.len() != 1 {
        return fail(format!(
            "qualification service evidence is incomplete for {}",
            planned.logical_id
        ));
    }
    Ok(())
}

fn project_fuzz(
    pair: &ManifestProjectionArtifact,
    planned: &PlannedQualification,
) -> Result<Value, CuratedManifestError> {
    if planned.payload_policy != QualificationPayloadPolicy::NoPayload
        || planned.profile.as_deref() != Some("fuzz")
        || planned.run_seed.is_none()
    {
        return fail("fuzz qualification plan has a payload or run-context mismatch");
    }
    let binding = planned
        .case_binding
        .as_ref()
        .ok_or_else(|| err("fuzz qualification has no case binding"))?;
    if binding.case_id != FUZZ_CASE_ID {
        return fail("fuzz qualification case binding drifted");
    }
    let service = &pair
        .execution
        .materialization
        .as_ref()
        .expect("validated above")
        .service_evidence[0];
    if service.evidence_id != "qualification_record"
        || service.evidence_kind != FUZZ_EVIDENCE_KIND
        || service.producer_id != FUZZ_PRODUCER_ID
        || service.producer_version.is_empty()
        || service.producer_executable_sha256.is_some()
    {
        return fail("fuzz qualification service identity drifted");
    }

    let claims_value = Value::Object(Map::from_iter(service.claims.clone()));
    let claims: FuzzClaims = serde_json::from_value(claims_value.clone())
        .map_err(|error| err(format!("invalid fuzz qualification evidence: {error}")))?;
    validate_fuzz_claims(planned, &claims)?;
    Ok(claims_value)
}

fn validate_fuzz_claims(
    planned: &PlannedQualification,
    claims: &FuzzClaims,
) -> Result<(), CuratedManifestError> {
    let binding = planned.case_binding.as_ref().expect("validated above");
    if claims.case_id != binding.case_id
        || claims.kind != FUZZ_EVIDENCE_KIND
        || claims.contract_version != "0.1.0"
        || claims.profile != planned.profile.as_deref().unwrap()
        || Some(claims.run_seed) != planned.run_seed
        || claims.provider.kind != "mutation_layer"
        || claims.provider.id != FUZZ_PRODUCER_ID
        || claims.target.kind != "same_project_bounded_part10_probe"
        || claims.target.independence != "same_project"
        || claims.target.operation_unit != "input_byte"
        || claims.payload_policy != "generated_payloads_uncommitted"
        || claims.status != "passed"
    {
        return fail("fuzz qualification public identity drifted");
    }

    let parameters: FuzzPlanParameters =
        serde_json::from_value(Value::Object(Map::from_iter(planned.parameters.clone())))
            .map_err(|error| err(format!("invalid planned fuzz parameters: {error}")))?;
    if parameters.qualification_kind != FUZZ_KIND
        || claims.budget != parameters.budget
        || claims.seeds.len() != parameters.sources.len()
        || claims.seeds.len() != planned.sources.len()
        || claims.counters.iterations > claims.budget.max_iterations
        || claims.counters.candidates > claims.budget.max_candidates
        || claims.counters.mutations > claims.budget.max_total_mutations
        || claims.counters.target_operations > claims.budget.max_total_target_operations
        || claims.counters.iterations != claims.counters.candidates
        || parameters
            .candidates_per_source
            .checked_mul(parameters.sources.len() as u64)
            != Some(claims.counters.candidates)
        || claims.outcomes.values().copied().sum::<u64>() != claims.counters.candidates
    {
        return fail("fuzz qualification evidence differs from its planned budget");
    }
    let expected_outcomes = BTreeSet::from([
        "accepted",
        "clean_rejection",
        "crash",
        "decode_failure",
        "hang",
        "parse_failure",
        "resource_limit",
        "timeout",
        "validation_failure",
    ]);
    if claims
        .outcomes
        .keys()
        .map(String::as_str)
        .collect::<BTreeSet<_>>()
        != expected_outcomes
        || claims.unacceptable_outcomes != ["crash", "hang", "timeout", "resource_limit"]
        || claims
            .unacceptable_outcomes
            .iter()
            .any(|outcome| claims.outcomes.get(outcome).copied().unwrap_or(1) != 0)
    {
        return fail("fuzz qualification outcome evidence is not an acceptable pass");
    }

    for ((claim, recipe), source) in claims
        .seeds
        .iter()
        .zip(&parameters.sources)
        .zip(&planned.sources)
    {
        if claim.id != recipe.seed_description_id
            || claim.id
                != source
                    .parameters
                    .get("seed_description_id")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
            || claim.source_case_id != source.case_binding.case_id
            || claim.source_recipe_id != source.case_binding.recipe_id
            || claim.source_recipe_version != source.case_binding.recipe_version
            || recipe.recipe.recipe_id != source.case_binding.recipe_id
            || recipe.recipe.recipe_version != source.case_binding.recipe_version
            || claim.source_generation_seed != parameters.source_generation_seed
            || claim.source_sha256 != source.expected_sha256
            || claim.source_size_bytes != source.expected_size_bytes
            || claim.surfaces != recipe.mutation_surfaces
            || source.dependency_role != recipe.dependency_role
            || source.artifact_logical_id != recipe.artifact_logical_id
            || source.parameters.get("mutation_surfaces")
                != Some(&serde_json::to_value(&recipe.mutation_surfaces).expect("string vector"))
        {
            return fail(format!(
                "fuzz source evidence drifted for {}",
                source.artifact_id
            ));
        }
    }
    let seed_ids = claims
        .seeds
        .iter()
        .map(|seed| seed.id.as_str())
        .collect::<BTreeSet<_>>();
    if claims.minimizations.iter().any(|item| {
        !seed_ids.contains(item.seed_description_id.as_str())
            || item.attempts > claims.budget.max_minimization_attempts
            || item.target_operations > claims.budget.max_target_operations
            || item.candidate_iteration >= parameters.candidates_per_source
            || item.candidate_seed == 0
            || item.minimized_size > item.original_size
            || item.minimized_fingerprint.is_empty()
            || !matches!(
                item.outcome.as_str(),
                "clean_rejection" | "parse_failure" | "validation_failure" | "decode_failure"
            )
    }) {
        return fail("fuzz minimization evidence violates its planned bounds");
    }
    Ok(())
}

fn validate_internal_eot(
    pair: &ManifestProjectionArtifact,
    planned: &PlannedQualification,
) -> Result<(), CuratedManifestError> {
    if planned.profile.is_some()
        || planned.payload_policy != QualificationPayloadPolicy::EvidenceOnly
        || !planned.sources.is_empty()
    {
        return fail("internal EOT qualification became public or payload-bearing");
    }
    let service = &pair
        .execution
        .materialization
        .as_ref()
        .expect("validated above")
        .service_evidence[0];
    if service.evidence_id != "qualification_record"
        || service.evidence_kind != EOT_KIND
        || service.producer_id != "checked_eot_arithmetic"
        || service.claims.get("status").and_then(Value::as_str) != Some("passed")
        || service.claims.get("payload_policy").and_then(Value::as_str) != Some("evidence_only")
    {
        return fail("internal EOT qualification evidence drifted");
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FuzzClaims {
    case_id: String,
    kind: String,
    contract_version: String,
    profile: String,
    run_seed: u64,
    provider: FuzzProvider,
    target: FuzzTarget,
    budget: FuzzBudget,
    seeds: Vec<FuzzSeed>,
    counters: FuzzCounters,
    outcomes: BTreeMap<String, u64>,
    minimizations: Vec<FuzzMinimization>,
    unacceptable_outcomes: Vec<String>,
    payload_policy: String,
    status: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FuzzProvider {
    kind: String,
    id: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FuzzTarget {
    kind: String,
    independence: String,
    operation_unit: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
struct FuzzBudget {
    max_iterations: u64,
    max_candidates: u64,
    max_mutations_per_candidate: u64,
    max_total_mutations: u64,
    max_bytes_per_mutation: u64,
    max_input_bytes: u64,
    max_output_bytes: u64,
    max_minimization_attempts: u64,
    max_total_target_operations: u64,
    max_target_operations: u64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FuzzSeed {
    id: String,
    source_case_id: String,
    source_recipe_id: String,
    source_recipe_version: String,
    source_generation_seed: u64,
    source_sha256: String,
    source_size_bytes: u64,
    surfaces: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FuzzCounters {
    iterations: u64,
    candidates: u64,
    mutations: u64,
    target_operations: u64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FuzzMinimization {
    seed_description_id: String,
    candidate_iteration: u64,
    candidate_seed: u64,
    outcome: String,
    original_size: u64,
    minimized_size: u64,
    attempts: u64,
    target_operations: u64,
    minimized_fingerprint: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FuzzPlanParameters {
    qualification_kind: String,
    source_generation_seed: u64,
    candidates_per_source: u64,
    sources: Vec<FuzzPlanSource>,
    budget: FuzzBudget,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FuzzPlanSource {
    seed_description_id: String,
    dependency_role: String,
    recipe: FuzzRecipeReference,
    artifact_logical_id: String,
    mutation_surfaces: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FuzzRecipeReference {
    recipe_id: String,
    recipe_version: String,
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::corpus_plan::{
        ArtifactProvenance, ArtifactResourceEstimate, CaseBinding, EvidencePlan,
        PlannedQualificationSource, ValidationPlan,
    };
    use crate::executor::evidence::{
        ArtifactExecutionEvidence, ArtifactResourceEvidence, MaterializationEvidence,
        MaterializationServiceEvidence, PublicationEvidence, PublicationState, RunResourceEvidence,
    };

    const PLAN_SHA256: &str = "1111111111111111111111111111111111111111111111111111111111111111";

    fn input() -> ManifestProjectionInput {
        let claims = json!({
            "case_id":FUZZ_CASE_ID,"kind":"bounded_fuzz_run","contract_version":"0.1.0",
            "profile":"fuzz","run_seed":1,
            "provider":{"kind":"mutation_layer","id":"bounded_deterministic_fuzz"},
            "target":{"kind":"same_project_bounded_part10_probe","independence":"same_project","operation_unit":"input_byte"},
            "budget":{"max_iterations":2,"max_candidates":2,"max_mutations_per_candidate":8,
                "max_total_mutations":16,"max_bytes_per_mutation":64,"max_input_bytes":8388608,
                "max_output_bytes":8388608,"max_minimization_attempts":256,
                "max_total_target_operations":100000000,"max_target_operations":1000000},
            "seeds":[
                {"id":"part10-explicit-vr-le-v1","source_case_id":"classic/sc/mono2_u8_explicit_le",
                 "source_recipe_id":"sc_mono2_u8","source_recipe_version":"0.1.0","source_generation_seed":7,
                 "source_sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                 "source_size_bytes":926,"surfaces":["file_meta","dataset_headers","pixel_data"]},
                {"id":"encapsulated-rle-v1","source_case_id":"classic/sc/mono1_u8_rle_lossless",
                 "source_recipe_id":"sc_mono1_u8_rle_lossless","source_recipe_version":"0.1.0","source_generation_seed":7,
                 "source_sha256":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                 "source_size_bytes":1032,"surfaces":["dataset_headers","encapsulation","pixel_data"]}
            ],
            "counters":{"iterations":2,"candidates":2,"mutations":2,"target_operations":100},
            "outcomes":{"accepted":2,"clean_rejection":0,"crash":0,"decode_failure":0,"hang":0,
                "parse_failure":0,"resource_limit":0,"timeout":0,"validation_failure":0},
            "minimizations":[],"unacceptable_outcomes":["crash","hang","timeout","resource_limit"],
            "payload_policy":"generated_payloads_uncommitted","status":"passed"
        });
        let source = |index: u64,
                      case_id: &str,
                      recipe_id: &str,
                      seed_id: &str,
                      role: &str,
                      hash: &str,
                      size: u64,
                      surfaces: Value| PlannedQualificationSource {
            artifact_id: format!("private_source_{index}"),
            case_binding: CaseBinding {
                case_id: case_id.into(),
                recipe_id: recipe_id.into(),
                recipe_version: "0.1.0".into(),
            },
            artifact_logical_id: "instance".into(),
            dependency_role: role.into(),
            binding_slot: format!("source_{index}"),
            expected_sha256: hash.into(),
            expected_size_bytes: size,
            parameters: BTreeMap::from([
                ("seed_description_id".into(), json!(seed_id)),
                ("mutation_surfaces".into(), surfaces),
            ]),
        };
        let sources = vec![
            source(
                0,
                "classic/sc/mono2_u8_explicit_le",
                "sc_mono2_u8",
                "part10-explicit-vr-le-v1",
                "part10_explicit_vr_le",
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                926,
                json!(["file_meta", "dataset_headers", "pixel_data"]),
            ),
            source(
                1,
                "classic/sc/mono1_u8_rle_lossless",
                "sc_mono1_u8_rle_lossless",
                "encapsulated-rle-v1",
                "encapsulated_rle",
                "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                1032,
                json!(["dataset_headers", "encapsulation", "pixel_data"]),
            ),
        ];
        let parameters = BTreeMap::from_iter(
            json!({"qualification_kind":FUZZ_KIND,"source_generation_seed":7,"candidates_per_source":1,
                "sources":[
                    {"seed_description_id":"part10-explicit-vr-le-v1","dependency_role":"part10_explicit_vr_le",
                     "recipe":{"recipe_id":"sc_mono2_u8","recipe_version":"0.1.0"},"artifact_logical_id":"instance",
                     "mutation_surfaces":["file_meta","dataset_headers","pixel_data"]},
                    {"seed_description_id":"encapsulated-rle-v1","dependency_role":"encapsulated_rle",
                     "recipe":{"recipe_id":"sc_mono1_u8_rle_lossless","recipe_version":"0.1.0"},"artifact_logical_id":"instance",
                     "mutation_surfaces":["dataset_headers","encapsulation","pixel_data"]}],
                "budget":{"max_iterations":2,"max_candidates":2,"max_mutations_per_candidate":8,"max_total_mutations":16,
                    "max_bytes_per_mutation":64,"max_input_bytes":8388608,"max_output_bytes":8388608,
                    "max_minimization_attempts":256,"max_total_target_operations":100000000,"max_target_operations":1000000}})
                .as_object().unwrap().clone(),
        );
        let planned = PlannedArtifact::Qualification(PlannedQualification {
            logical_id: "curated_fuzz_qualification".into(),
            order: 2,
            provenance: ArtifactProvenance::Requested,
            case_binding: Some(CaseBinding {
                case_id: FUZZ_CASE_ID.into(),
                recipe_id: "fuzz_parser_bounded_seed_corpus".into(),
                recipe_version: "0.1.0".into(),
            }),
            profile: Some("fuzz".into()),
            run_seed: Some(1),
            qualification_kind: FUZZ_KIND.into(),
            parameters,
            sources,
            payload_policy: QualificationPayloadPolicy::NoPayload,
            validation: ValidationPlan { rules: vec![] },
            evidence: EvidencePlan {
                obligations: vec![],
            },
            resources: ArtifactResourceEstimate {
                output_bytes: 0,
                peak_working_bytes: 16 * 1024 * 1024,
            },
        });
        let claims = claims.as_object().unwrap().clone().into_iter().collect();
        let execution = ArtifactExecutionEvidence {
            logical_id: "curated_fuzz_qualification".into(),
            order: 2,
            artifact_kind: ArtifactKind::Qualification,
            status: ExecutionStatus::Succeeded,
            corpus_plan_sha256: PLAN_SHA256.into(),
            instance_plan_sha256: None,
            output: None,
            materialization: Some(MaterializationEvidence {
                backend_id: FUZZ_PRODUCER_ID.into(),
                transfer_syntax_uid: None,
                streamed_slots: vec![],
                completed: true,
                materialized_instance_plan_sha256: None,
                materialized_encoding_sha256: None,
                materialized_artifact_sha256: None,
                preamble_policy: None,
                preamble_sha256: None,
                file_meta_policy: None,
                file_meta_sha256: None,
                file_meta_size_bytes: None,
                implementation_class_uid: None,
                implementation_version_name: None,
                content: vec![],
                imported_dicom: None,
                service_evidence: vec![MaterializationServiceEvidence {
                    evidence_id: "qualification_record".into(),
                    evidence_kind: FUZZ_EVIDENCE_KIND.into(),
                    producer_id: FUZZ_PRODUCER_ID.into(),
                    producer_version: "0.1.0".into(),
                    producer_executable_sha256: None,
                    claims,
                }],
            }),
            validation: vec![],
            obligations: vec![],
            providers: vec![],
            codecs: vec![],
            resources: ArtifactResourceEvidence {
                planned_output_bytes: 0,
                planned_peak_working_bytes: 16 * 1024 * 1024,
                actual_output_bytes: 0,
                actual_peak_working_bytes: Some(1024),
                elapsed_milliseconds: 1,
            },
        };
        ManifestProjectionInput {
            corpus_plan_sha256: PLAN_SHA256.into(),
            artifacts: vec![ManifestProjectionArtifact { planned, execution }],
            unavailable: vec![],
            resources: RunResourceEvidence {
                planned_max_artifacts: 3,
                planned_max_total_output_bytes: 1,
                planned_max_peak_working_bytes: 16 * 1024 * 1024,
                requested_parallelism: 1,
                used_parallelism: 1,
                actual_artifact_output_bytes: 0,
                actual_publication_bytes: 0,
                actual_peak_working_bytes: Some(1024),
            },
            publication: PublicationEvidence {
                manifest_relative_path: "manifest.json".into(),
                state: PublicationState::Staging,
                private_staging: true,
                no_overwrite: true,
                validation_complete: true,
                cleanup_complete: false,
                manifest_sha256: None,
            },
        }
    }

    #[test]
    fn returns_the_exact_actual_fuzz_record() {
        let input = input();
        let expected = Value::Object(
            input.artifacts[0]
                .execution
                .materialization
                .as_ref()
                .unwrap()
                .service_evidence[0]
                .claims
                .clone()
                .into_iter()
                .collect(),
        );
        assert_eq!(project_qualifications(&input).unwrap(), vec![expected]);
    }

    #[test]
    fn fails_closed_on_plan_execution_identity_or_payload_drift() {
        let mut identity = input();
        identity.artifacts[0].execution.logical_id = "different".into();
        assert!(
            project_qualifications(&identity)
                .unwrap_err()
                .0
                .contains("identity mismatch")
        );

        let mut payload = input();
        payload.artifacts[0].execution.resources.actual_output_bytes = 1;
        assert!(
            project_qualifications(&payload)
                .unwrap_err()
                .0
                .contains("retained a payload")
        );

        let mut claims = input();
        claims.artifacts[0]
            .execution
            .materialization
            .as_mut()
            .unwrap()
            .service_evidence[0]
            .claims
            .insert("payload".into(), json!([1, 2, 3]));
        assert!(
            project_qualifications(&claims)
                .unwrap_err()
                .0
                .contains("unknown field")
        );
    }

    #[test]
    fn fails_closed_on_unverified_actual_identity() {
        let mut input = input();
        input.artifacts[0]
            .execution
            .materialization
            .as_mut()
            .unwrap()
            .service_evidence[0]
            .claims
            .insert("run_seed".into(), json!(2));
        assert!(
            project_qualifications(&input)
                .unwrap_err()
                .0
                .contains("public identity drifted")
        );
    }
}
