use std::collections::BTreeMap;
use std::sync::Arc;

use synth_dicom_gen::corpus_plan::{
    ArtifactDependency, ArtifactProvenance, ArtifactResourceEstimate, EvidencePlan, OutputPlan,
    OutputRelativePath, PlannedArtifact, PlannedAuxiliaryArtifact, PlannedQualification,
    PublicationPlan, PublicationTransaction, QualificationPayloadPolicy, ResourcePlan,
    ValidationPlan, ValidationRequirement, ValidationRule,
};
use synth_dicom_gen::planning::{
    CapabilityAvailability, CapabilityRequirement, CapabilityService, CasePlanner,
    CasePlannerRegistry, ContentFactory, ContentFactoryOutput, ContentFactoryRegistry,
    ContentFactoryRequest, CorpusPlanAssembler, CuratedCaseRequest, DeterministicIdentityRequest,
    IdentityService, ManifestProjectionInput, ManifestProjector, PlanFragment, PlanProvider,
    PlanProviderRegistry, PlanProviderRequest, PlannedCase, PlanningContext, PlanningError,
    PlanningTemplate, ProjectionError, RecipeIdentity, TemplateIdentity, TemplateService,
    ValidationExecutorDescriptor, ValidationRuleDescriptor, ValidationRuleRegistry,
};
use serde_json::{Value, json};

fn recipe(id: &str) -> RecipeIdentity {
    RecipeIdentity {
        recipe_id: id.into(),
        recipe_version: "1.0.0".into(),
    }
}

fn validation(rule_id: &str) -> ValidationPlan {
    ValidationPlan {
        rules: vec![ValidationRule {
            rule_id: rule_id.into(),
            requirement: ValidationRequirement::Required,
            parameters: BTreeMap::new(),
        }],
    }
}

fn qualification(id: &str, order: u64, provenance: ArtifactProvenance) -> PlannedArtifact {
    PlannedArtifact::Qualification(PlannedQualification {
        logical_id: id.into(),
        order,
        provenance,
        case_binding: None,
        profile: None,
        run_seed: None,
        qualification_kind: "test_qualification".into(),
        parameters: BTreeMap::new(),
        sources: vec![],
        payload_policy: QualificationPayloadPolicy::EvidenceOnly,
        validation: validation("part10.identity"),
        evidence: EvidencePlan {
            obligations: vec![],
        },
        resources: ArtifactResourceEstimate {
            output_bytes: 0,
            peak_working_bytes: 1024,
        },
    })
}

fn auxiliary(id: &str, order: u64, requested_by: &str) -> PlannedArtifact {
    PlannedArtifact::Auxiliary(PlannedAuxiliaryArtifact {
        logical_id: id.into(),
        order,
        provenance: ArtifactProvenance::Dependency {
            requested_by: vec![requested_by.into()],
        },
        auxiliary_kind: "evidence".into(),
        output: OutputPlan {
            relative_path: OutputRelativePath::new(format!("evidence/{id}.json")).unwrap(),
            role: "evidence".into(),
            publish: true,
        },
        parameters: BTreeMap::new(),
        validation: validation("part10.identity"),
        evidence: EvidencePlan {
            obligations: vec![],
        },
        resources: ArtifactResourceEstimate {
            output_bytes: 128,
            peak_working_bytes: 1024,
        },
    })
}

fn dependency(artifact_id: &str, depends_on: &str) -> ArtifactDependency {
    ArtifactDependency {
        artifact_id: artifact_id.into(),
        depends_on: depends_on.into(),
        relationship: "evidence_for".into(),
        frame_numbers: vec![],
    }
}

#[derive(Debug)]
struct DummyPlanner(&'static str);

impl CasePlanner for DummyPlanner {
    fn identity(&self) -> RecipeIdentity {
        recipe(self.0)
    }

    fn plan(
        &self,
        request: &CuratedCaseRequest,
        _context: &PlanningContext<'_>,
    ) -> Result<PlannedCase, PlanningError> {
        Ok(PlannedCase {
            case_id: request.case_id.clone(),
            recipe: request.recipe.clone(),
            fragment: PlanFragment {
                artifacts: vec![],
                dependencies: vec![],
                unavailable: vec![],
                plan_provider_ids: vec![],
                content_factory_ids: vec![],
            },
        })
    }
}

#[derive(Debug)]
struct DummyProvider(&'static str);

impl PlanProvider for DummyProvider {
    fn provider_id(&self) -> &str {
        self.0
    }

    fn plan(
        &self,
        _request: &PlanProviderRequest,
        _context: &PlanningContext<'_>,
    ) -> Result<PlanFragment, PlanningError> {
        Ok(PlanFragment {
            artifacts: vec![],
            dependencies: vec![],
            unavailable: vec![],
            plan_provider_ids: vec![self.0.into()],
            content_factory_ids: vec![],
        })
    }
}

#[derive(Debug)]
struct DummyFactory(&'static str);

impl ContentFactory for DummyFactory {
    fn factory_id(&self) -> &str {
        self.0
    }

    fn plan_content(
        &self,
        _request: &ContentFactoryRequest,
        _identities: &dyn IdentityService,
    ) -> Result<ContentFactoryOutput, PlanningError> {
        Ok(ContentFactoryOutput {
            content: vec![],
            resources: ArtifactResourceEstimate {
                output_bytes: 0,
                peak_working_bytes: 1,
            },
            evidence: vec![],
        })
    }
}

fn planner_registry(ids: &[&'static str]) -> CasePlannerRegistry {
    CasePlannerRegistry::new(
        ids.iter()
            .map(|id| Arc::new(DummyPlanner(id)) as Arc<dyn CasePlanner>),
    )
    .unwrap()
}

fn provider_registry(ids: &[&'static str]) -> PlanProviderRegistry {
    PlanProviderRegistry::new(
        ids.iter()
            .map(|id| Arc::new(DummyProvider(id)) as Arc<dyn PlanProvider>),
    )
    .unwrap()
}

fn factory_registry(ids: &[&'static str]) -> ContentFactoryRegistry {
    ContentFactoryRegistry::new(
        ids.iter()
            .map(|id| Arc::new(DummyFactory(id)) as Arc<dyn ContentFactory>),
    )
    .unwrap()
}

fn rules() -> ValidationRuleRegistry {
    ValidationRuleRegistry::new([ValidationRuleDescriptor {
        rule_id: "part10.identity".into(),
        layer: "part10".into(),
        executor: ValidationExecutorDescriptor::BuiltIn {
            executor_id: "builtin.part10".into(),
        },
    }])
    .unwrap()
}

fn publication() -> PublicationPlan {
    PublicationPlan {
        manifest_path: OutputRelativePath::new("manifest.json").unwrap(),
        transaction: PublicationTransaction::AtomicNoReplace,
        private_staging: true,
        no_overwrite: true,
    }
}

fn resources() -> ResourcePlan {
    ResourcePlan {
        max_artifacts: 16,
        max_total_output_bytes: 1_000_000,
        max_peak_working_bytes: 1_000_000,
        max_parallelism: 4,
    }
}

fn case(
    case_id: &str,
    recipe_id: &str,
    artifacts: Vec<PlannedArtifact>,
    dependencies: Vec<ArtifactDependency>,
) -> PlannedCase {
    PlannedCase {
        case_id: case_id.into(),
        recipe: recipe(recipe_id),
        fragment: PlanFragment {
            artifacts,
            dependencies,
            unavailable: vec![],
            plan_provider_ids: vec!["geometry.series".into()],
            content_factory_ids: vec!["pixels.native".into()],
        },
    }
}

#[test]
fn assembly_is_stable_by_explicit_order_across_fragment_submission_order() {
    let planners = planner_registry(&["root_recipe", "source_recipe"]);
    let providers = provider_registry(&["geometry.series"]);
    let factories = factory_registry(&["pixels.native"]);
    let rules = rules();
    let root = case(
        "root-case",
        "root_recipe",
        vec![
            qualification("root", 20, ArtifactProvenance::Requested),
            auxiliary("report", 30, "root"),
        ],
        vec![dependency("root", "source"), dependency("report", "root")],
    );
    let source = case(
        "source-case",
        "source_recipe",
        vec![qualification(
            "source",
            10,
            ArtifactProvenance::Dependency {
                requested_by: vec!["root".into()],
            },
        )],
        vec![],
    );

    let mut first = CorpusPlanAssembler::new(
        7,
        publication(),
        resources(),
        &planners,
        &providers,
        &factories,
        &rules,
    );
    first.add_case(root.clone()).unwrap();
    first.add_case(source.clone()).unwrap();
    let first = first.assemble().unwrap();

    let mut second = CorpusPlanAssembler::new(
        7,
        publication(),
        resources(),
        &planners,
        &providers,
        &factories,
        &rules,
    );
    second.add_case(source).unwrap();
    second.add_case(root).unwrap();
    let second = second.assemble().unwrap();

    assert_eq!(
        first
            .artifacts
            .iter()
            .map(PlannedArtifact::logical_id)
            .collect::<Vec<_>>(),
        vec!["source", "root", "report"]
    );
    assert_eq!(
        first.canonical_sha256().unwrap(),
        second.canonical_sha256().unwrap()
    );
    assert_eq!(first, second);
}

#[test]
fn assembler_rejects_unregistered_recipe_provider_factory_and_rule() {
    let planners = planner_registry(&["known"]);
    let providers = provider_registry(&["known.provider"]);
    let factories = factory_registry(&["known.factory"]);
    let rules = rules();

    let mut assembler = CorpusPlanAssembler::new(
        1,
        publication(),
        resources(),
        &planners,
        &providers,
        &factories,
        &rules,
    );
    assert!(matches!(
        assembler.add_case(case(
            "unknown-recipe",
            "unknown",
            vec![qualification("one", 1, ArtifactProvenance::Requested)],
            vec![],
        )),
        Err(PlanningError::UnregisteredRecipeIdentity(_))
    ));

    let unknown_provider = PlanFragment {
        artifacts: vec![],
        dependencies: vec![],
        unavailable: vec![],
        plan_provider_ids: vec!["unknown.provider".into()],
        content_factory_ids: vec![],
    };
    assert!(matches!(
        assembler.add_fragment(unknown_provider),
        Err(PlanningError::UnregisteredPlanProvider(_))
    ));

    let unknown_factory = PlanFragment {
        artifacts: vec![],
        dependencies: vec![],
        unavailable: vec![],
        plan_provider_ids: vec![],
        content_factory_ids: vec!["unknown.factory".into()],
    };
    assert!(matches!(
        assembler.add_fragment(unknown_factory),
        Err(PlanningError::UnregisteredContentFactory(_))
    ));

    let mut artifact = qualification("rule", 2, ArtifactProvenance::Requested);
    let PlannedArtifact::Qualification(value) = &mut artifact else {
        unreachable!()
    };
    value.validation.rules[0].rule_id = "unknown.rule".into();
    assert!(matches!(
        assembler.add_fragment(PlanFragment {
            artifacts: vec![artifact],
            dependencies: vec![],
            unavailable: vec![],
            plan_provider_ids: vec![],
            content_factory_ids: vec![],
        }),
        Err(PlanningError::UnregisteredValidationRule(_))
    ));
}

#[test]
fn registries_reject_duplicate_identities_before_planning() {
    assert!(matches!(
        CasePlannerRegistry::new([
            Arc::new(DummyPlanner("same")) as Arc<dyn CasePlanner>,
            Arc::new(DummyPlanner("same")) as Arc<dyn CasePlanner>,
        ]),
        Err(PlanningError::DuplicateRecipeIdentity(_))
    ));
    assert!(matches!(
        PlanProviderRegistry::new([
            Arc::new(DummyProvider("same")) as Arc<dyn PlanProvider>,
            Arc::new(DummyProvider("same")) as Arc<dyn PlanProvider>,
        ]),
        Err(PlanningError::DuplicatePlanProvider(_))
    ));
    assert!(matches!(
        ContentFactoryRegistry::new([
            Arc::new(DummyFactory("same")) as Arc<dyn ContentFactory>,
            Arc::new(DummyFactory("same")) as Arc<dyn ContentFactory>,
        ]),
        Err(PlanningError::DuplicateContentFactory(_))
    ));
    assert!(matches!(
        ValidationRuleRegistry::new([
            ValidationRuleDescriptor {
                rule_id: "same".into(),
                layer: "part10".into(),
                executor: ValidationExecutorDescriptor::BuiltIn {
                    executor_id: "one".into(),
                },
            },
            ValidationRuleDescriptor {
                rule_id: "same".into(),
                layer: "part10".into(),
                executor: ValidationExecutorDescriptor::BuiltIn {
                    executor_id: "two".into(),
                },
            },
        ]),
        Err(PlanningError::DuplicateValidationRule(_))
    ));
}

struct FixedIdentity;

impl IdentityService for FixedIdentity {
    fn allocate(&self, _request: &DeterministicIdentityRequest) -> Result<String, PlanningError> {
        Ok("2.25.1".into())
    }
}

struct FixedCapabilities;

impl CapabilityService for FixedCapabilities {
    fn resolve(
        &self,
        _requirement: &CapabilityRequirement,
    ) -> Result<CapabilityAvailability, PlanningError> {
        Ok(CapabilityAvailability::Available { version: None })
    }
}

struct FixedTemplates;

impl TemplateService for FixedTemplates {
    fn resolve(&self, _identity: &TemplateIdentity) -> Result<PlanningTemplate, PlanningError> {
        Err(PlanningError::Template("unused test service".into()))
    }
}

struct FixedProjector;

impl ManifestProjector for FixedProjector {
    fn projector_id(&self) -> &str {
        "test.projector"
    }

    fn project(
        &self,
        plan: &synth_dicom_gen::corpus_plan::CorpusPlan,
        input: &ManifestProjectionInput,
    ) -> Result<Value, ProjectionError> {
        Ok(json!({
            "plan": input.corpus_plan_sha256,
            "artifacts": plan.artifacts.len()
        }))
    }
}

#[test]
fn planning_context_and_projector_are_data_only_contracts() {
    let providers = provider_registry(&[]);
    let factories = factory_registry(&[]);
    let rules = rules();
    let context = PlanningContext {
        seed: 9,
        standards_lock_sha256: "abc",
        identities: &FixedIdentity,
        capabilities: &FixedCapabilities,
        templates: &FixedTemplates,
        content_factories: &factories,
        validation_rules: &rules,
        plan_providers: &providers,
    };
    assert_eq!(context.seed, 9);
    assert_eq!(context.standards_lock_sha256, "abc");

    let plan = synth_dicom_gen::corpus_plan::CorpusPlan {
        schema_version: synth_dicom_gen::corpus_plan::CORPUS_PLAN_SCHEMA_VERSION.into(),
        seed: 9,
        artifacts: vec![qualification("one", 1, ArtifactProvenance::Requested)],
        dependencies: vec![],
        unavailable: vec![],
        publication: publication(),
        resources: resources(),
    };
    let hash = plan.canonical_sha256().unwrap();
    let projected = FixedProjector
        .project(
            &plan,
            &ManifestProjectionInput {
                corpus_plan_sha256: hash.clone(),
                execution_evidence: BTreeMap::new(),
            },
        )
        .unwrap();
    assert_eq!(projected, json!({"plan": hash, "artifacts": 1}));
}

#[test]
fn neutral_planning_module_has_no_filesystem_or_writer_escape_hatch() {
    let source = include_str!("../src/planning.rs");
    for forbidden in [
        "std::fs",
        "std::path",
        "PathBuf",
        "Part10Materializer",
        "PreparedGenerationRun",
        "pub out_dir",
        "output_directory",
        "write_to_file",
    ] {
        assert!(
            !source.contains(forbidden),
            "forbidden planning dependency {forbidden}"
        );
    }
}
