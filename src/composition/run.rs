use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use serde_json::{Value, json};

use crate::codecs::{FrameDecodeInput, FrameDecoder, NativeRleLosslessEncoder};
#[cfg(test)]
use crate::codecs::{FrameEncodeInput, FrameEncoder};
use crate::encapsulation::{BasicOffsetTablePolicy, EncapsulatedPixelData};

use super::advanced_defaults::{
    AdvancedDefaultMember, AdvancedSourceMember, group_identity, is_direct_advanced_default,
    is_external_quantitative_default, is_native_quantitative_default, is_reference_default,
    is_typed_bulk_default, plan_external_quantitative_default, plan_image_group,
    plan_native_quantitative_default, plan_reference_default, plan_typed_bulk_default,
};
use super::advanced_semantic_defaults::{
    align_external_sr_source_graph, is_external_sr_default, is_native_rt_default,
    is_native_sr_default, plan_external_sr_default, plan_native_rt_default, plan_native_sr_default,
    qualify_tid1500_segmentation_sources, semantic_identity_roles,
};
use super::executor_adapter::{
    CompositionExecutionBundle, CompositionExecutionServiceFactory,
    CompositionExecutorManifestProjector, CompositionProjectionContext, CompositionSource,
    CompositionSourceAsset, DeferredCompositionProvider,
};
use super::external_quantitative::{
    plan_caller_parametric_map, seed_parametric_reference_sequence,
};
use super::{
    AdvancedFamilyProfile, BundleResolver, CompositionManifestInputs, CompositionSpec,
    CompositionUidRole, ContentLimits, ContentSource, CyclePolicy, DefaultPixelOutput,
    IdentityAllocator, IdentityChoice, LocalContentResolver, LogicalReference, ProviderInvocation,
    ProviderOutputDeclaration, ProviderRequest, ReferenceGraph, ReferenceNode,
    ResolvedInstancePlan, TemplateCatalog, TemplateDescriptor, default_family_pixels,
    resolve_family_attributes, resolve_raw_native_pixels, resolved_sc_plan, sc_default_pixels,
};
use crate::executor::engine::{CorpusExecutor, CorpusExecutorError};
use crate::executor::materialization::{
    AuxiliaryMaterializationHandler, AuxiliaryPayload, MaterializationError,
};
use crate::executor::services::{
    ArtifactExecutionBindings, ByteBinding, CodecRequest, NativeFrameBinding,
    ProviderOutputExpectation, ProviderRequest as ExecutorProviderRequest, SlotExecutionBinding,
    StagedAssetHandle, StagedAssetRegistry, StagingRelativePath,
};
use crate::recipes::{AdvancedProviderLimits, RecipeCatalog};
use crate::{PACKAGE_NAME, PACKAGE_VERSION, RUSTC_VERSION, TARGET_TRIPLE, sha256_hex};

#[derive(Debug, Clone)]
pub struct ComposeOptions {
    pub spec_path: PathBuf,
    pub out_dir: PathBuf,
    pub seed: u64,
    pub catalog_path: PathBuf,
    pub dry_run: bool,
}

#[derive(Debug, Clone)]
pub struct ComposeBytesOptions {
    pub spec_root: PathBuf,
    pub out_dir: PathBuf,
    pub seed: u64,
    pub catalog_path: PathBuf,
    pub dry_run: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComposeSummary {
    pub out_dir: PathBuf,
    pub manifest_path: PathBuf,
    pub instances_written: usize,
    pub output_bytes: u64,
    pub dry_run: bool,
}

#[derive(Debug, Clone, Default)]
pub struct ComposeCancellationToken {
    cancelled: Arc<AtomicBool>,
    executor: crate::executor::cancellation::CancellationToken,
}

impl ComposeCancellationToken {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
        self.executor.cancel();
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }

    fn executor_token(&self) -> &crate::executor::cancellation::CancellationToken {
        &self.executor
    }
}

pub fn compose(options: &ComposeOptions) -> Result<(ComposeSummary, Value), ComposeError> {
    compose_with_cancellation(options, &ComposeCancellationToken::new())
}

pub fn compose_with_cancellation(
    options: &ComposeOptions,
    cancellation: &ComposeCancellationToken,
) -> Result<(ComposeSummary, Value), ComposeError> {
    check_cancelled(cancellation)?;
    let spec_bytes = fs::read(&options.spec_path).map_err(|source| ComposeError::Io {
        path: options.spec_path.clone(),
        source,
    })?;
    let spec_root = options.spec_path.parent().unwrap_or_else(|| Path::new("."));
    compose_loaded(
        &spec_bytes,
        spec_root,
        &options.out_dir,
        options.seed,
        &options.catalog_path,
        options.dry_run,
        cancellation,
    )
}

pub fn compose_from_bytes(
    spec_bytes: &[u8],
    options: &ComposeBytesOptions,
) -> Result<(ComposeSummary, Value), ComposeError> {
    compose_from_bytes_with_cancellation(spec_bytes, options, &ComposeCancellationToken::new())
}

pub fn compose_from_bytes_with_cancellation(
    spec_bytes: &[u8],
    options: &ComposeBytesOptions,
    cancellation: &ComposeCancellationToken,
) -> Result<(ComposeSummary, Value), ComposeError> {
    check_cancelled(cancellation)?;
    compose_loaded(
        spec_bytes,
        &options.spec_root,
        &options.out_dir,
        options.seed,
        &options.catalog_path,
        options.dry_run,
        cancellation,
    )
}

fn compose_loaded(
    spec_bytes: &[u8],
    spec_root: &Path,
    out_dir: &Path,
    seed: u64,
    catalog_path: &Path,
    dry_run: bool,
    cancellation: &ComposeCancellationToken,
) -> Result<(ComposeSummary, Value), ComposeError> {
    if out_dir.exists() {
        return Err(ComposeError::OutputExists(out_dir.to_path_buf()));
    }
    let catalog_bytes = fs::read(catalog_path).map_err(|source| ComposeError::Io {
        path: catalog_path.to_path_buf(),
        source,
    })?;
    let spec = CompositionSpec::from_slice(&spec_bytes)?;
    let catalog = TemplateCatalog::from_slice(&catalog_bytes)?;
    let parent = out_dir
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let options = ComposeOptions {
        spec_path: spec_root.join("<in-memory-spec>"),
        out_dir: out_dir.to_path_buf(),
        seed,
        catalog_path: catalog_path.to_path_buf(),
        dry_run,
    };
    let planned = resolve_execution_bundle(
        &options,
        &spec,
        &catalog,
        &spec_bytes,
        &catalog_bytes,
        &std::env::temp_dir(),
        spec_root,
        cancellation,
    )?;
    if cancellation.is_cancelled() {
        return Err(ComposeError::Cancelled);
    }
    if dry_run {
        return Ok((
            ComposeSummary {
                out_dir: out_dir.to_path_buf(),
                manifest_path: out_dir.join("manifest.json"),
                instances_written: 0,
                output_bytes: 0,
                dry_run: true,
            },
            planned.dry_run_output,
        ));
    }
    if let Err(source) = fs::create_dir_all(parent) {
        return Err(ComposeError::Io {
            path: parent.to_path_buf(),
            source,
        });
    }
    let services = CompositionExecutionServiceFactory::new(
        &planned.bundle,
        Arc::new(RejectCompositionAuxiliary),
    );
    let projector = CompositionExecutorManifestProjector::new(planned.bundle.projection.clone());
    let executor = CorpusExecutor::new(services, projector);
    // macOS exposes its temporary directory through `/var`, a system symlink
    // to `/private/var`. Resolve the existing parent so the shared transaction
    // receives a symlink-free destination while the public path stays intact.
    let canonical_parent = match fs::canonicalize(parent) {
        Ok(parent) => parent,
        Err(source) => {
            return Err(ComposeError::Io {
                path: parent.to_path_buf(),
                source,
            });
        }
    };
    let execution_out = canonical_parent.join(
        out_dir
            .file_name()
            .ok_or_else(|| ComposeError::Executor("output has no final component".into()))?,
    );
    let execution = executor.execute(
        &planned.bundle.plan,
        &execution_out,
        spec.parallelism,
        cancellation.executor_token(),
    );
    let execution = match execution {
        Ok(execution) => Ok(execution),
        Err(error) => Err(map_executor_error(error, &spec.resource_limits)),
    };
    match execution {
        Ok(execution) => {
            let manifest = serde_json::from_slice(&execution.manifest_bytes)
                .map_err(|error| ComposeError::ExecutorManifest(error.to_string()))?;
            Ok((
                ComposeSummary {
                    out_dir: out_dir.to_path_buf(),
                    manifest_path: out_dir.join("manifest.json"),
                    instances_written: planned.bundle.plan.artifacts.len(),
                    output_bytes: execution.evidence.resources.actual_artifact_output_bytes,
                    dry_run: false,
                },
                manifest,
            ))
        }
        Err(error) => Err(error),
    }
}

struct PlannedCompositionExecution {
    bundle: CompositionExecutionBundle,
    dry_run_output: Value,
}

struct RejectCompositionAuxiliary;

impl AuxiliaryMaterializationHandler for RejectCompositionAuxiliary {
    fn render(
        &self,
        artifact: &crate::corpus_plan::PlannedAuxiliaryArtifact,
        _: &ArtifactExecutionBindings,
        _: &StagedAssetRegistry,
    ) -> Result<AuxiliaryPayload, MaterializationError> {
        Err(MaterializationError::Auxiliary(format!(
            "composition does not plan auxiliary artifact {}",
            artifact.logical_id
        )))
    }
}

fn resolve_execution_bundle(
    options: &ComposeOptions,
    spec: &CompositionSpec,
    catalog: &TemplateCatalog,
    spec_bytes: &[u8],
    catalog_bytes: &[u8],
    _scratch_parent: &Path,
    spec_root: &Path,
    cancellation: &ComposeCancellationToken,
) -> Result<PlannedCompositionExecution, ComposeError> {
    let bundle_resolution = BundleResolver.resolve(spec.clone(), catalog)?;
    let mut spec = bundle_resolution.spec.clone();
    align_external_sr_source_graph(&mut spec).map_err(ComposeError::AdvancedDefaults)?;
    if spec.instances.len() as u64 > spec.resource_limits.max_instances {
        return Err(ComposeError::Spec(super::SpecError::InstanceLimit {
            count: spec.instances.len(),
            limit: spec.resource_limits.max_instances,
        }));
    }
    let mut content_resolver = LocalContentResolver::new_read_only(
        spec_root,
        ContentLimits {
            max_files: usize::try_from(spec.resource_limits.max_input_files)
                .map_err(|_| ComposeError::ResourceRange)?,
            max_file_bytes: spec.resource_limits.max_file_bytes,
            max_total_bytes: spec.resource_limits.max_total_input_bytes,
        },
    )?;
    let run_defaults = spec.defaults.typed_attributes()?;
    reject_structural_overrides("composition defaults", &run_defaults)?;
    let recipes = RecipeCatalog::load(
        "cases/recipes",
        "cases/registry.json",
        &options.catalog_path,
    )
    .map_err(|error| ComposeError::AdvancedDefaults(error.to_string()))?;
    let repository_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let advanced_limits = advanced_provider_limits(&spec)?;
    let mut plans_by_id = BTreeMap::new();
    let mut advanced_artifacts: BTreeMap<
        String,
        super::corpus_adapter::AdvancedCompositionArtifact,
    > = BTreeMap::new();
    let mut advanced_dependencies = Vec::new();
    let mut external_dicom_providers = BTreeMap::new();
    let mut processed_advanced_groups = std::collections::BTreeSet::new();
    let mut native_codec_plans = BTreeMap::new();
    let mut caller_parametric_ids = std::collections::BTreeSet::new();
    let mut templates = Vec::with_capacity(spec.instances.len());
    let mut identity_plans = BTreeMap::new();

    for instance in &spec.instances {
        check_cancelled(cancellation)?;
        let template =
            catalog.resolve_qualified(&instance.template.id, instance.template.version)?;
        validate_parameters(instance, template)?;
        let p2_sc = matches!(
            template.template_id.0.as_str(),
            "classic/secondary-capture/monochrome" | "classic/secondary-capture/rgb"
        );
        let family = super::ClassicFamilyProfile::for_template(&template.template_id);
        let advanced = AdvancedFamilyProfile::for_template(&template.template_id.0);
        if !p2_sc && family.is_none() && advanced.is_none() {
            return Err(ComposeError::UnsupportedTemplate(
                template.template_id.0.clone(),
            ));
        }
        let allocator = IdentityAllocator::new(
            catalog.standards_lock_sha256.clone(),
            template.template_id.clone(),
            template.template_version,
            options.seed,
        )?;
        let mut roles = vec![
            (CompositionUidRole::StudyInstance, 0),
            (CompositionUidRole::SeriesInstance, 0),
            (CompositionUidRole::SopInstance, 0),
            (CompositionUidRole::ImplementationClass, 0),
        ];
        if family.is_some_and(|profile| profile.include_geometry)
            || advanced.is_some_and(|profile| profile.include_frame_of_reference)
        {
            roles.push((CompositionUidRole::FrameOfReference, 0));
        }
        roles.extend(
            semantic_identity_roles(&recipes, template)
                .map_err(ComposeError::AdvancedDefaults)?
                .into_iter()
                .map(|role| (role, 0)),
        );
        let mut identities = allocator.allocate_plan(instance.instance_id.clone(), roles)?;
        apply_explicit_identities(instance, &mut identities.identities)?;
        identity_plans.insert(instance.instance_id.clone(), identities);
        templates.push(template);
    }
    apply_shared_identities(&spec, &mut identity_plans)?;
    let (mut execution_bindings, deferred_providers) =
        plan_spec_providers(&spec, &templates, &identity_plans)?;

    for (instance_index, (instance, template)) in spec
        .instances
        .iter()
        .zip(templates.iter().copied())
        .enumerate()
    {
        check_cancelled(cancellation)?;
        validate_reference_roles(instance, template)?;
        let transfer_syntax_uid = instance
            .transfer_syntax_uid
            .as_deref()
            .or(spec.defaults.transfer_syntax_uid.as_deref())
            .unwrap_or_else(|| catalog.default_transfer_syntax(template).uid.as_str());
        if !template
            .transfer_syntaxes
            .iter()
            .any(|syntax| syntax.uid == transfer_syntax_uid)
        {
            return Err(ComposeError::UnsupportedTransferSyntax {
                instance_id: instance.instance_id.clone(),
                uid: transfer_syntax_uid.into(),
            });
        }
        let overrides = instance.typed_attributes()?;
        reject_structural_overrides(&instance.instance_id, &overrides)?;
        let base_plan = ResolvedInstancePlan {
            plan_schema_version: "0.1.0".into(),
            instance_id: instance.instance_id.clone(),
            template_id: template.template_id.clone(),
            template_version: template.template_version,
            sop_class_uid: template.sop_class_uid.clone(),
            transfer_syntax_uid: transfer_syntax_uid.into(),
            identities: identity_plans
                .get(&instance.instance_id)
                .cloned()
                .expect("identity pass covered every instance"),
            attributes: vec![],
            content: vec![],
            references: vec![],
        };
        if is_direct_advanced_default(template) && is_reference_default(template) {
            continue;
        } else if is_direct_advanced_default(template) {
            let member = AdvancedDefaultMember {
                instance,
                template,
                identities: base_plan.identities.clone(),
                order: u64::try_from(instance_index).map_err(|_| ComposeError::ResourceRange)?,
            };
            let bundle_root = &bundle_resolution
                .member(&instance.instance_id)
                .bundle_root_instance_id;
            let group = group_identity(&recipes, &member, bundle_root)
                .map_err(ComposeError::AdvancedDefaults)?;
            if !processed_advanced_groups.insert(group.clone()) {
                continue;
            }
            let mut group_members = Vec::new();
            for (order, (candidate, candidate_template)) in spec
                .instances
                .iter()
                .zip(templates.iter().copied())
                .enumerate()
            {
                if !is_direct_advanced_default(candidate_template)
                    || is_reference_default(candidate_template)
                {
                    continue;
                }
                let candidate_member = AdvancedDefaultMember {
                    instance: candidate,
                    template: candidate_template,
                    identities: identity_plans
                        .get(&candidate.instance_id)
                        .cloned()
                        .expect("identity pass covered every instance"),
                    order: u64::try_from(order).map_err(|_| ComposeError::ResourceRange)?,
                };
                let candidate_root = &bundle_resolution
                    .member(&candidate.instance_id)
                    .bundle_root_instance_id;
                if group_identity(&recipes, &candidate_member, candidate_root)
                    .map_err(ComposeError::AdvancedDefaults)?
                    == group
                {
                    group_members.push(candidate_member);
                }
            }
            let output = plan_image_group(
                &recipes,
                &catalog.standards_lock_sha256,
                options.seed,
                advanced_limits.clone(),
                &group_members,
            )
            .map_err(ComposeError::AdvancedDefaults)?;
            advanced_dependencies.extend(output.dependencies);
            execution_bindings.extend(output.bindings);
            for mut planned in output.artifacts.into_values() {
                let planned_instance = spec
                    .instances
                    .iter()
                    .find(|candidate| candidate.instance_id == planned.logical_id)
                    .expect("provider output was selected from composition members");
                let planned_template = templates
                    .iter()
                    .copied()
                    .zip(&spec.instances)
                    .find_map(|(template, candidate)| {
                        (candidate.instance_id == planned.logical_id).then_some(template)
                    })
                    .expect("provider output was selected from composition templates");
                let profile = AdvancedFamilyProfile::for_template(&planned_template.template_id.0)
                    .expect("direct advanced template has a profile");
                let mut resolved = planned.instance.clone();
                profile.customize_direct_plan(
                    planned_instance,
                    &mut resolved,
                    &mut content_resolver,
                )?;
                resolved.identities = identity_plans
                    .get(&resolved.instance_id)
                    .cloned()
                    .expect("identity pass covered every advanced instance");
                planned.encoding.implementation.class_uid = resolved
                    .identities
                    .get(&CompositionUidRole::ImplementationClass, 0)
                    .expect("advanced identity plan includes implementation class")
                    .to_owned();
                planned.instance = resolved.clone();
                advanced_artifacts.insert(planned.logical_id.clone(), planned.into());
                plans_by_id.insert(resolved.instance_id.clone(), resolved);
            }
        } else if is_typed_bulk_default(template) {
            let member = AdvancedDefaultMember {
                instance,
                template,
                identities: base_plan.identities.clone(),
                order: u64::try_from(instance_index).map_err(|_| ComposeError::ResourceRange)?,
            };
            let mut output = plan_typed_bulk_default(&recipes, &member)
                .map_err(ComposeError::AdvancedDefaults)?;
            let mut planned = output
                .artifacts
                .remove(&instance.instance_id)
                .ok_or_else(|| {
                    ComposeError::AdvancedDefaults("typed-bulk provider omitted target".into())
                })?;
            let profile = AdvancedFamilyProfile::for_template(&template.template_id.0)
                .expect("typed-bulk default template has a profile");
            profile.customize_direct_plan(
                instance,
                &mut planned.instance,
                &mut content_resolver,
            )?;
            if instance
                .content
                .iter()
                .any(|assignment| !matches!(assignment.source, ContentSource::Default))
            {
                output.bindings.remove(&instance.instance_id);
            }
            execution_bindings.extend(output.bindings);
            advanced_artifacts.insert(planned.logical_id.clone(), planned.clone().into());
            plans_by_id.insert(instance.instance_id.clone(), planned.instance);
        } else if is_external_quantitative_default(template)
            && instance
                .content
                .iter()
                .any(|assignment| !matches!(assignment.source, ContentSource::Default))
        {
            let plan =
                plan_caller_parametric_map(base_plan, instance, template, &mut content_resolver)?;
            caller_parametric_ids.insert(instance.instance_id.clone());
            plans_by_id.insert(instance.instance_id.clone(), plan);
        } else if is_native_quantitative_default(template)
            || (is_external_quantitative_default(template)
                && instance
                    .content
                    .iter()
                    .all(|assignment| matches!(assignment.source, ContentSource::Default)))
        {
            continue;
        } else if is_native_sr_default(template) || is_external_sr_default(template) {
            continue;
        } else if is_native_rt_default(template) {
            continue;
        } else if AdvancedFamilyProfile::for_template(&template.template_id.0).is_some() {
            return Err(ComposeError::AdvancedDefaults(format!(
                "{} has not been routed through its neutral default provider",
                template.template_id
            )));
        } else if let Some(profile) =
            super::ClassicFamilyProfile::for_template(&template.template_id)
        {
            let pixel = resolve_family_pixels(
                instance,
                &profile,
                transfer_syntax_uid,
                &mut content_resolver,
            )?;
            validate_family_pixel_contract(&profile, &pixel)?;
            if transfer_syntax_uid == crate::codecs::RLE_LOSSLESS_TRANSFER_SYNTAX_UID
                && !matches!(
                    pixel.content.materialization,
                    Some(super::ContentMaterialization::Encapsulated { .. })
                )
            {
                native_codec_plans.insert(instance.instance_id.clone(), pixel.plan.clone());
            }
            let mut plan = base_plan;
            plan.attributes = resolve_family_attributes(
                &profile,
                &plan.identities,
                &pixel.plan,
                &run_defaults,
                &overrides,
            )?;
            plan.content.push(pixel.content);
            plans_by_id.insert(plan.instance_id.clone(), plan);
        } else {
            let pixel = resolve_sc_pixels(instance, template, &mut content_resolver)?;
            validate_sc_pixel_contract(template, &pixel)?;
            let plan = resolved_sc_plan(base_plan, template, &run_defaults, &overrides, pixel)?;
            plans_by_id.insert(plan.instance_id.clone(), plan);
        }
    }

    for (instance_index, (instance, template)) in spec
        .instances
        .iter()
        .zip(templates.iter().copied())
        .enumerate()
    {
        if is_external_quantitative_default(template)
            && instance
                .content
                .iter()
                .all(|assignment| matches!(assignment.source, ContentSource::Default))
        {
            check_cancelled(cancellation)?;
            if template
                .template_id
                .0
                .starts_with("derived/parametric-map/")
            {
                for (source_index, reference) in instance.references.iter().enumerate() {
                    let source_plan = plans_by_id
                        .get_mut(&reference.target_instance_id)
                        .ok_or_else(|| {
                            ComposeError::AdvancedDefaults(format!(
                                "quantitative source {} is not planned",
                                reference.target_instance_id
                            ))
                        })?;
                    qualify_parametric_source_geometry(source_plan, source_index)?;
                }
            }
            let sources = instance
                .references
                .iter()
                .map(|reference| {
                    let source_index = spec
                        .instances
                        .iter()
                        .position(|candidate| candidate.instance_id == reference.target_instance_id)
                        .ok_or_else(|| {
                            ComposeError::AdvancedDefaults(format!(
                                "quantitative source {} is absent",
                                reference.target_instance_id
                            ))
                        })?;
                    advanced_source_member(
                        &reference.target_instance_id,
                        source_index,
                        &plans_by_id,
                        &advanced_artifacts,
                        &execution_bindings,
                        &bundle_resolution.members,
                        &spec.resource_limits,
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;
            let member = AdvancedDefaultMember {
                instance,
                template,
                identities: identity_plans
                    .get(&instance.instance_id)
                    .cloned()
                    .expect("identity pass covered every instance"),
                order: u64::try_from(instance_index).map_err(|_| ComposeError::ResourceRange)?,
            };
            let output = plan_external_quantitative_default(
                &recipes,
                &repository_root,
                &catalog.standards_lock_sha256,
                options.seed,
                &member,
                &sources,
            )
            .map_err(ComposeError::AdvancedDefaults)?;
            advanced_dependencies.extend(output.dependencies);
            let request_id = output.artifact.provider.request_id.clone();
            execution_bindings.insert(instance.instance_id.clone(), output.bindings);
            plans_by_id.insert(
                instance.instance_id.clone(),
                output.artifact.declared_instance.clone(),
            );
            advanced_artifacts.insert(
                instance.instance_id.clone(),
                super::corpus_adapter::AdvancedCompositionArtifact::Imported(output.artifact),
            );
            external_dicom_providers.insert(request_id, output.provider);
            continue;
        }
        if is_native_quantitative_default(template) {
            check_cancelled(cancellation)?;
            let source_reference = instance.references.first().ok_or_else(|| {
                ComposeError::AdvancedDefaults(format!(
                    "quantitative target {} has no bundle source",
                    instance.instance_id
                ))
            })?;
            let source_index = spec
                .instances
                .iter()
                .position(|candidate| candidate.instance_id == source_reference.target_instance_id)
                .ok_or_else(|| {
                    ComposeError::AdvancedDefaults(format!(
                        "quantitative source {} is absent",
                        source_reference.target_instance_id
                    ))
                })?;
            let source = advanced_source_member(
                &source_reference.target_instance_id,
                source_index,
                &plans_by_id,
                &advanced_artifacts,
                &execution_bindings,
                &bundle_resolution.members,
                &spec.resource_limits,
            )?;
            let member = AdvancedDefaultMember {
                instance,
                template,
                identities: identity_plans
                    .get(&instance.instance_id)
                    .cloned()
                    .expect("identity pass covered every instance"),
                order: u64::try_from(instance_index).map_err(|_| ComposeError::ResourceRange)?,
            };
            let mut output = plan_native_quantitative_default(&recipes, &member, &source)
                .map_err(ComposeError::AdvancedDefaults)?;
            advanced_dependencies.extend(output.dependencies);
            let mut planned = output
                .artifacts
                .remove(&instance.instance_id)
                .ok_or_else(|| {
                    ComposeError::AdvancedDefaults(
                        "quantitative provider omitted composition target".into(),
                    )
                })?;
            let profile = AdvancedFamilyProfile::for_template(&template.template_id.0)
                .expect("quantitative template has an advanced profile");
            profile.customize_direct_plan(
                instance,
                &mut planned.instance,
                &mut content_resolver,
            )?;
            if instance
                .content
                .iter()
                .any(|assignment| !matches!(assignment.source, ContentSource::Default))
            {
                output.bindings.remove(&instance.instance_id);
            }
            execution_bindings.extend(output.bindings);
            advanced_artifacts.insert(planned.logical_id.clone(), planned.clone().into());
            plans_by_id.insert(instance.instance_id.clone(), planned.instance);
            continue;
        }
        if !is_direct_advanced_default(template) || !is_reference_default(template) {
            continue;
        }
        check_cancelled(cancellation)?;
        validate_reference_roles(instance, template)?;
        let mut target = AdvancedDefaultMember {
            instance,
            template,
            identities: identity_plans
                .get(&instance.instance_id)
                .cloned()
                .expect("identity pass covered every instance"),
            order: u64::try_from(instance_index).map_err(|_| ComposeError::ResourceRange)?,
        };
        let sources = instance
            .references
            .iter()
            .map(|reference| {
                advanced_source_member(
                    reference.target_instance_id.as_str(),
                    spec.instances
                        .iter()
                        .position(|candidate| candidate.instance_id == reference.target_instance_id)
                        .ok_or_else(|| {
                            ComposeError::AdvancedDefaults(format!(
                                "reference source {} is absent",
                                reference.target_instance_id
                            ))
                        })?,
                    &plans_by_id,
                    &advanced_artifacts,
                    &execution_bindings,
                    &bundle_resolution.members,
                    &spec.resource_limits,
                )
            })
            .collect::<Result<Vec<_>, ComposeError>>()?;
        if let Some(source) = sources.first() {
            for role in [CompositionUidRole::StudyInstance] {
                if let Some(value) = source.artifact.instance.identities.get(&role, 0) {
                    let key = match role {
                        CompositionUidRole::StudyInstance => "study_instance_uid#0",
                        CompositionUidRole::FrameOfReference => "frame_of_reference_uid#0",
                        _ => unreachable!(),
                    };
                    target
                        .identities
                        .identities
                        .insert(key.into(), value.to_owned());
                }
            }
        }
        let output = plan_reference_default(
            &recipes,
            &catalog.standards_lock_sha256,
            options.seed,
            advanced_limits.clone(),
            &target,
            &sources,
        )
        .map_err(ComposeError::AdvancedDefaults)?;
        advanced_dependencies.extend(output.dependencies);
        execution_bindings.extend(output.bindings);
        let mut planned = output.artifacts.into_values().next().ok_or_else(|| {
            ComposeError::AdvancedDefaults("reference provider omitted target".into())
        })?;
        let profile = AdvancedFamilyProfile::for_template(&template.template_id.0)
            .expect("direct advanced template has a profile");
        let mut resolved = planned.instance.clone();
        profile.customize_direct_plan(instance, &mut resolved, &mut content_resolver)?;
        resolved.identities = target.identities.clone();
        planned.encoding.implementation.class_uid = resolved
            .identities
            .get(&CompositionUidRole::ImplementationClass, 0)
            .expect("advanced identity plan includes implementation class")
            .to_owned();
        planned.instance = resolved.clone();
        advanced_artifacts.insert(planned.logical_id.clone(), planned.into());
        plans_by_id.insert(resolved.instance_id.clone(), resolved);
    }

    for (instance_index, (instance, template)) in spec
        .instances
        .iter()
        .zip(templates.iter().copied())
        .enumerate()
    {
        if !is_external_sr_default(template) {
            continue;
        }
        check_cancelled(cancellation)?;
        let sources = instance
            .references
            .iter()
            .map(|reference| {
                advanced_source_member(
                    &reference.target_instance_id,
                    spec.instances
                        .iter()
                        .position(|candidate| candidate.instance_id == reference.target_instance_id)
                        .ok_or_else(|| {
                            ComposeError::AdvancedDefaults(format!(
                                "external SR source {} is absent",
                                reference.target_instance_id
                            ))
                        })?,
                    &plans_by_id,
                    &advanced_artifacts,
                    &execution_bindings,
                    &bundle_resolution.members,
                    &spec.resource_limits,
                )
            })
            .collect::<Result<Vec<_>, ComposeError>>()?;
        let member = AdvancedDefaultMember {
            instance,
            template,
            identities: identity_plans
                .get(&instance.instance_id)
                .cloned()
                .expect("identity pass covered every instance"),
            order: u64::try_from(instance_index).map_err(|_| ComposeError::ResourceRange)?,
        };
        let output = plan_external_sr_default(
            &recipes,
            &repository_root,
            &catalog.standards_lock_sha256,
            options.seed,
            &member,
            &sources,
        )
        .map_err(ComposeError::AdvancedDefaults)?;
        advanced_dependencies.extend(output.dependencies);
        let request_id = output.artifact.provider.request_id.clone();
        execution_bindings.insert(instance.instance_id.clone(), output.bindings);
        plans_by_id.insert(
            instance.instance_id.clone(),
            output.artifact.declared_instance.clone(),
        );
        advanced_artifacts.insert(
            instance.instance_id.clone(),
            super::corpus_adapter::AdvancedCompositionArtifact::Imported(output.artifact),
        );
        external_dicom_providers.insert(request_id, output.provider);
    }

    for (instance_index, (instance, template)) in spec
        .instances
        .iter()
        .zip(templates.iter().copied())
        .enumerate()
    {
        if !is_native_sr_default(template) {
            continue;
        }
        check_cancelled(cancellation)?;
        let sources = instance
            .references
            .iter()
            .map(|reference| {
                advanced_source_member(
                    &reference.target_instance_id,
                    spec.instances
                        .iter()
                        .position(|candidate| candidate.instance_id == reference.target_instance_id)
                        .ok_or_else(|| {
                            ComposeError::AdvancedDefaults(format!(
                                "SR source {} is absent",
                                reference.target_instance_id
                            ))
                        })?,
                    &plans_by_id,
                    &advanced_artifacts,
                    &execution_bindings,
                    &bundle_resolution.members,
                    &spec.resource_limits,
                )
            })
            .collect::<Result<Vec<_>, ComposeError>>()?;
        let member = AdvancedDefaultMember {
            instance,
            template,
            identities: identity_plans
                .get(&instance.instance_id)
                .cloned()
                .expect("identity pass covered every instance"),
            order: u64::try_from(instance_index).map_err(|_| ComposeError::ResourceRange)?,
        };
        let mut output = plan_native_sr_default(&recipes, &member, &sources)
            .map_err(ComposeError::AdvancedDefaults)?;
        advanced_dependencies.extend(output.dependencies);
        let mut planned = output
            .artifacts
            .remove(&instance.instance_id)
            .ok_or_else(|| {
                ComposeError::AdvancedDefaults("SR provider omitted composition target".into())
            })?;
        let profile = AdvancedFamilyProfile::for_template(&template.template_id.0)
            .expect("SR template has an advanced profile");
        profile.customize_direct_plan(instance, &mut planned.instance, &mut content_resolver)?;
        execution_bindings.extend(output.bindings);
        advanced_artifacts.insert(planned.logical_id.clone(), planned.clone().into());
        plans_by_id.insert(instance.instance_id.clone(), planned.instance);
    }

    let mut pending_rt = spec
        .instances
        .iter()
        .zip(templates.iter().copied())
        .enumerate()
        .filter_map(|(index, (instance, template))| {
            is_native_rt_default(template).then_some((index, instance, template))
        })
        .collect::<Vec<_>>();
    while !pending_rt.is_empty() {
        let before = pending_rt.len();
        let mut deferred = Vec::new();
        for (instance_index, instance, template) in pending_rt {
            check_cancelled(cancellation)?;
            if instance
                .references
                .iter()
                .any(|reference| !plans_by_id.contains_key(&reference.target_instance_id))
            {
                deferred.push((instance_index, instance, template));
                continue;
            }
            let sources = instance
                .references
                .iter()
                .map(|reference| {
                    advanced_source_member(
                        &reference.target_instance_id,
                        spec.instances
                            .iter()
                            .position(|candidate| {
                                candidate.instance_id == reference.target_instance_id
                            })
                            .ok_or_else(|| {
                                ComposeError::AdvancedDefaults(format!(
                                    "RT source {} is absent",
                                    reference.target_instance_id
                                ))
                            })?,
                        &plans_by_id,
                        &advanced_artifacts,
                        &execution_bindings,
                        &bundle_resolution.members,
                        &spec.resource_limits,
                    )
                })
                .collect::<Result<Vec<_>, ComposeError>>()?;
            let member = AdvancedDefaultMember {
                instance,
                template,
                identities: identity_plans
                    .get(&instance.instance_id)
                    .cloned()
                    .expect("identity pass covered every instance"),
                order: u64::try_from(instance_index).map_err(|_| ComposeError::ResourceRange)?,
            };
            let mut output = plan_native_rt_default(&recipes, &member, &sources)
                .map_err(ComposeError::AdvancedDefaults)?;
            advanced_dependencies.extend(output.dependencies);
            let mut planned = output
                .artifacts
                .remove(&instance.instance_id)
                .ok_or_else(|| {
                    ComposeError::AdvancedDefaults("RT provider omitted composition target".into())
                })?;
            let profile = AdvancedFamilyProfile::for_template(&template.template_id.0)
                .expect("RT template has an advanced profile");
            profile.customize_direct_plan(
                instance,
                &mut planned.instance,
                &mut content_resolver,
            )?;
            execution_bindings.extend(output.bindings);
            advanced_artifacts.insert(planned.logical_id.clone(), planned.clone().into());
            plans_by_id.insert(instance.instance_id.clone(), planned.instance);
        }
        if deferred.len() == before {
            return Err(ComposeError::AdvancedDefaults(
                "RT recipe dependency graph could not be resolved".into(),
            ));
        }
        pending_rt = deferred;
    }

    let mut plans = spec
        .instances
        .iter()
        .map(|instance| {
            plans_by_id.remove(&instance.instance_id).ok_or_else(|| {
                ComposeError::AdvancedDefaults(format!(
                    "advanced planning omitted {}",
                    instance.instance_id
                ))
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    enforce_synthetic_data(&mut plans)?;

    super::advanced_family::validate_concatenation_closure(&plans, &bundle_resolution.members)?;
    validate_explicit_reference_frames(&plans, &spec)?;
    let imported_references = advanced_artifacts
        .iter()
        .filter_map(|(id, artifact)| match artifact {
            super::corpus_adapter::AdvancedCompositionArtifact::Imported(_) => plans
                .iter_mut()
                .find(|plan| plan.instance_id == *id)
                .map(|plan| (id.clone(), std::mem::take(&mut plan.references))),
            super::corpus_adapter::AdvancedCompositionArtifact::Native(_) => None,
        })
        .collect::<BTreeMap<_, _>>();
    materialize_reference_graph(&mut plans, &spec, &bundle_resolution.members)?;
    for plan in &mut plans {
        if caller_parametric_ids.contains(&plan.instance_id) {
            seed_parametric_reference_sequence(plan)?;
        }
    }
    for plan in &mut plans {
        if imported_references.contains_key(&plan.instance_id) {
            plan.references.clear();
        }
    }
    super::advanced_family::rewrite_materialized_dicom_references(&mut plans)?;
    qualify_tid1500_segmentation_sources(&spec, &mut plans)
        .map_err(ComposeError::AdvancedDefaults)?;
    for plan in &mut plans {
        if let Some(references) = imported_references.get(&plan.instance_id) {
            plan.references = references.clone();
        }
    }
    for plan in &plans {
        if let Some(artifact) = advanced_artifacts.get_mut(&plan.instance_id) {
            *artifact.resolved_instance_mut() = plan.clone();
        }
    }

    let source_assets =
        bind_execution_content(&mut plans, &native_codec_plans, &mut execution_bindings)?;
    let corpus_plan = super::corpus_adapter::resolved_composition_corpus_plan_with_advanced(
        options.seed,
        &plans,
        &bundle_resolution.members,
        &spec.resource_limits,
        spec.parallelism,
        &advanced_artifacts,
        &advanced_dependencies,
    )?;

    let dry_run_output = json!({
        "composition_spec_schema_version": spec.composition_spec_schema_version,
        "seed": options.seed,
        "plans": plans
    });
    let used_parallelism = usize::try_from(spec.parallelism)
        .map_err(|_| ComposeError::ResourceRange)?
        .min(plans.len())
        .max(1);
    let projection = Arc::new(CompositionProjectionContext {
        inputs: CompositionManifestInputs {
            generated_at: "2000-01-01T00:00:00Z".into(),
            generator: json!({
                "name": PACKAGE_NAME,
                "version": PACKAGE_VERSION,
                "rustc": RUSTC_VERSION,
                "target": TARGET_TRIPLE
            }),
            standards: json!({
                "dicom_base_edition": "2026b",
                "standards_lock_sha256": catalog.standards_lock_sha256
            }),
            dependencies: json!({ "template_catalog": "templates/catalog.json" }),
            seed: options.seed,
            composition_spec_schema_version: spec.composition_spec_schema_version.clone(),
            input_spec_sha256: sha256_hex(spec_bytes),
            template_catalog_schema_version: catalog.template_catalog_schema_version.clone(),
            template_catalog_sha256: sha256_hex(catalog_bytes),
            resource_limits: resource_map(&spec.resource_limits),
            requested_parallelism: spec.parallelism,
            used_parallelism: u32::try_from(used_parallelism)
                .map_err(|_| ComposeError::ResourceRange)?,
        },
        members: bundle_resolution.members.clone(),
    });
    Ok(PlannedCompositionExecution {
        bundle: CompositionExecutionBundle {
            plan: corpus_plan,
            bindings: execution_bindings,
            projection,
            source_assets,
            providers: deferred_providers,
            external_dicom_providers,
        },
        dry_run_output,
    })
}

fn qualify_parametric_source_geometry(
    plan: &mut ResolvedInstancePlan,
    index: usize,
) -> Result<(), ComposeError> {
    let positions = ["0", "5", "10"];
    let instance_numbers = ["30", "10", "20"];
    let position = positions.get(index).ok_or(ComposeError::ResourceRange)?;
    for (tag, vr, value) in [
        (
            "0020,0032",
            super::DicomVr::DS,
            super::AttributeValue::Multi(vec![
                super::PrimitiveValue::String("0".into()),
                super::PrimitiveValue::String("0".into()),
                super::PrimitiveValue::String((*position).into()),
            ]),
        ),
        (
            "0020,0013",
            super::DicomVr::IS,
            super::AttributeValue::Primitive(super::PrimitiveValue::String(
                instance_numbers[index].into(),
            )),
        ),
    ] {
        let address = super::AttributeAddress::from_normalized_tag(tag)
            .map_err(|error| ComposeError::AdvancedDefaults(error.to_string()))?;
        let replacement = super::ResolvedAttribute {
            address: address.clone(),
            vr,
            value: Some(value),
            origin: super::ValueOrigin::DerivedStructural,
        };
        if let Some(existing) = plan
            .attributes
            .iter_mut()
            .find(|attribute| attribute.address == address)
        {
            *existing = replacement;
        } else {
            plan.attributes.push(replacement);
        }
    }
    plan.attributes
        .sort_by(|left, right| left.address.cmp(&right.address));
    Ok(())
}

fn advanced_provider_limits(
    spec: &CompositionSpec,
) -> Result<AdvancedProviderLimits, ComposeError> {
    let max_references = spec
        .resource_limits
        .max_instances
        .checked_mul(spec.resource_limits.max_instances)
        .ok_or(ComposeError::ResourceRange)?;
    Ok(AdvancedProviderLimits {
        max_artifacts: spec.resource_limits.max_instances,
        max_references,
        max_binding_slots: spec
            .resource_limits
            .max_input_files
            .max(spec.resource_limits.max_instances),
        max_total_output_bytes: spec.resource_limits.max_total_output_bytes,
        max_peak_working_bytes: spec.resource_limits.max_total_output_bytes,
        max_parallelism: spec.parallelism.max(1),
    })
}

fn advanced_source_member(
    instance_id: &str,
    order: usize,
    plans: &BTreeMap<String, ResolvedInstancePlan>,
    advanced: &BTreeMap<String, super::corpus_adapter::AdvancedCompositionArtifact>,
    bindings: &BTreeMap<String, ArtifactExecutionBindings>,
    members: &BTreeMap<String, super::BundleMemberProvenance>,
    limits: &super::ResourceLimits,
) -> Result<AdvancedSourceMember, ComposeError> {
    let plan = plans.get(instance_id).ok_or_else(|| {
        ComposeError::AdvancedDefaults(format!("reference source {instance_id} is not planned"))
    })?;
    let artifact = if let Some(artifact) = advanced.get(instance_id) {
        artifact.native().cloned().ok_or_else(|| {
            ComposeError::AdvancedDefaults(format!(
                "imported artifact {instance_id} cannot be used as a native planning source"
            ))
        })?
    } else {
        let per_artifact_limit = limits
            .max_total_output_bytes
            .checked_div(limits.max_instances.max(1))
            .unwrap_or(limits.max_total_output_bytes)
            .max(1);
        match super::corpus_adapter::planned_artifact(order, plan, members, per_artifact_limit)? {
            crate::corpus_plan::PlannedArtifact::Dicom(artifact) => artifact,
            _ => unreachable!("composition source plans are DICOM artifacts"),
        }
    };
    let binding = bindings
        .get(instance_id)
        .cloned()
        .unwrap_or_else(|| ArtifactExecutionBindings {
            artifact_id: instance_id.to_owned(),
            slots: plan
                .content
                .iter()
                .map(|content| {
                    (
                        content.slot.clone(),
                        SlotExecutionBinding::NativeFrames { frames: Vec::new() },
                    )
                })
                .collect(),
        });
    Ok(AdvancedSourceMember { artifact, binding })
}

fn enforce_synthetic_data(plans: &mut [ResolvedInstancePlan]) -> Result<(), ComposeError> {
    let address = super::AttributeAddress::from_normalized_tag("0008,001C")
        .expect("Synthetic Data is a standard DICOM tag");
    for plan in plans {
        if let Some(attribute) = plan
            .attributes
            .iter_mut()
            .find(|attribute| attribute.address == address)
        {
            attribute.vr = super::DicomVr::CS;
            attribute.value = Some(super::AttributeValue::Primitive(
                super::PrimitiveValue::String("YES".into()),
            ));
            attribute.origin = super::ValueOrigin::DerivedStructural;
            continue;
        }
        plan.attributes.push(super::ResolvedAttribute {
            address: address.clone(),
            vr: super::DicomVr::CS,
            value: Some(super::AttributeValue::Primitive(
                super::PrimitiveValue::String("YES".into()),
            )),
            origin: super::ValueOrigin::DerivedStructural,
        });
        plan.attributes
            .sort_by(|left, right| left.address.cmp(&right.address));
    }
    Ok(())
}

fn validate_explicit_reference_frames(
    plans: &[ResolvedInstancePlan],
    spec: &CompositionSpec,
) -> Result<(), ComposeError> {
    let frames = plans
        .iter()
        .map(|plan| Ok((plan.instance_id.as_str(), resolved_frame_count(plan)?)))
        .collect::<Result<BTreeMap<_, _>, ComposeError>>()?;
    for instance in &spec.instances {
        for reference in &instance.references {
            let available = frames
                .get(reference.target_instance_id.as_str())
                .copied()
                .ok_or_else(|| {
                    ComposeError::AdvancedDefaults(format!(
                        "reference target {} is not planned",
                        reference.target_instance_id
                    ))
                })?;
            if let Some(frame) = reference
                .frames
                .iter()
                .copied()
                .find(|frame| *frame == 0 || *frame > available)
            {
                return Err(ComposeError::AdvancedDefaults(format!(
                    "reference frame {frame} exceeds {} frames on {}",
                    available, reference.target_instance_id
                )));
            }
        }
    }
    Ok(())
}

fn bind_execution_content(
    plans: &mut [ResolvedInstancePlan],
    native_codec_plans: &BTreeMap<String, super::NativePixelPlan>,
    bindings: &mut BTreeMap<String, ArtifactExecutionBindings>,
) -> Result<Vec<CompositionSourceAsset>, ComposeError> {
    let mut sources = Vec::new();
    for (artifact_index, plan) in plans.iter_mut().enumerate() {
        let artifact_bindings =
            bindings
                .entry(plan.instance_id.clone())
                .or_insert_with(|| ArtifactExecutionBindings {
                    artifact_id: plan.instance_id.clone(),
                    slots: BTreeMap::new(),
                });
        for content in &mut plan.content {
            let planning_materialization = content.materialization.clone();
            let caller_inline = content.properties.get("content_origin").map(String::as_str)
                == Some("inline_fixture");
            let source = match &content.materialization {
                Some(super::ContentMaterialization::StagedFile(path)) => {
                    Some(CompositionSource::File(path.clone()))
                }
                Some(super::ContentMaterialization::Inline(bytes)) if caller_inline => {
                    Some(CompositionSource::Inline(bytes.clone()))
                }
                _ => None,
            };
            let source_handle = if let Some(source) = source {
                let handle = StagedAssetHandle::new(format!(
                    "composition-source-{}-{}",
                    artifact_index, content.slot
                ))
                .map_err(|error| ComposeError::ExecutionBinding(error.to_string()))?;
                let relative_path = StagingRelativePath::new(format!(
                    ".composition-inputs/{artifact_index}/{}.bin",
                    content.slot
                ))
                .map_err(|error| ComposeError::ExecutionBinding(error.to_string()))?;
                sources.push(CompositionSourceAsset {
                    handle: handle.clone(),
                    source,
                    staging_relative_path: relative_path,
                    media_type: "application/octet-stream".into(),
                    expected_size_bytes: content.size_bytes,
                    expected_sha256: content.sha256.clone(),
                });
                content.materialization = None;
                Some(handle)
            } else {
                None
            };

            if let Some(native) = native_codec_plans.get(&plan.instance_id) {
                let provider_request =
                    artifact_bindings
                        .slots
                        .get(&content.slot)
                        .and_then(|binding| match binding {
                            SlotExecutionBinding::ProviderRequest { request } => {
                                Some(request.clone())
                            }
                            _ => None,
                        });
                let provider_asset = provider_request
                    .as_ref()
                    .map(|request| {
                        StagedAssetHandle::new(format!(
                            "provider:{}:{}",
                            request.request_id, content.slot
                        ))
                    })
                    .transpose()
                    .map_err(|error| ComposeError::ExecutionBinding(error.to_string()))?;
                let frames = native
                    .frame_spans
                    .iter()
                    .map(|span| {
                        if span.bit_offset % 8 != 0 || span.bit_length % 8 != 0 {
                            return Err(ComposeError::CodecPixelAlignment);
                        }
                        let offset = span.bit_offset / 8;
                        let length = span.bit_length / 8;
                        let bytes = match (&planning_materialization, &source_handle) {
                            (Some(super::ContentMaterialization::Inline(all)), _) => {
                                let start = usize::try_from(offset)
                                    .map_err(|_| ComposeError::ResourceRange)?;
                                let end = usize::try_from(
                                    offset
                                        .checked_add(length)
                                        .ok_or(ComposeError::ResourceRange)?,
                                )
                                .map_err(|_| ComposeError::ResourceRange)?;
                                let frame = all
                                    .get(start..end)
                                    .ok_or(ComposeError::ResourceRange)?
                                    .to_vec();
                                ByteBinding::Inline {
                                    sha256: sha256_hex(&frame),
                                    bytes: frame,
                                }
                            }
                            (
                                Some(super::ContentMaterialization::StagedFile(path)),
                                Some(handle),
                            ) => {
                                let mut file =
                                    fs::File::open(path).map_err(|source| ComposeError::Io {
                                        path: path.clone(),
                                        source,
                                    })?;
                                file.seek(SeekFrom::Start(offset)).map_err(|source| {
                                    ComposeError::CodecRead {
                                        frame: span.frame_number as usize,
                                        source,
                                    }
                                })?;
                                let mut frame = vec![
                                    0;
                                    usize::try_from(length)
                                        .map_err(|_| ComposeError::ResourceRange)?
                                ];
                                file.read_exact(&mut frame).map_err(|source| {
                                    ComposeError::CodecRead {
                                        frame: span.frame_number as usize,
                                        source,
                                    }
                                })?;
                                ByteBinding::StagedRange {
                                    asset: handle.clone(),
                                    offset,
                                    length,
                                    sha256: sha256_hex(&frame),
                                }
                            }
                            (None, _) if provider_asset.is_some() => {
                                ByteBinding::VerifiedAssetRange {
                                    asset: provider_asset
                                        .as_ref()
                                        .expect("provider asset was checked")
                                        .clone(),
                                    offset,
                                    length,
                                }
                            }
                            _ => return Err(ComposeError::MissingPixelMaterialization),
                        };
                        Ok(NativeFrameBinding {
                            frame_number: span.frame_number,
                            bytes,
                            rows: native.shape.rows,
                            columns: native.shape.columns,
                            samples_per_pixel: u16::from(native.shape.samples_per_pixel),
                            bits_allocated: u16::from(native.shape.bits_allocated),
                            photometric_interpretation: photometric_name(
                                native.shape.photometric_interpretation,
                            )
                            .into(),
                        })
                    })
                    .collect::<Result<Vec<_>, ComposeError>>()?;
                let codec = CodecRequest {
                    request_id: format!("composition-rle-{}", artifact_index),
                    artifact_id: plan.instance_id.clone(),
                    slot: content.slot.clone(),
                    backend_id: NativeRleLosslessEncoder::BACKEND_ID.into(),
                    source_transfer_syntax_uid: "1.2.840.10008.1.2.1".into(),
                    target_transfer_syntax_uid: crate::codecs::RLE_LOSSLESS_TRANSFER_SYNTAX_UID
                        .into(),
                    frames,
                    parameters: BTreeMap::from([(
                        "bits_stored".into(),
                        json!(native.shape.bits_stored),
                    )]),
                };
                let binding = if let Some(provider) = provider_request {
                    SlotExecutionBinding::ProviderCodecPipeline { provider, codec }
                } else {
                    SlotExecutionBinding::CodecRequest { request: codec }
                };
                artifact_bindings
                    .slots
                    .insert(content.slot.clone(), binding);
            } else if let Some(handle) = source_handle {
                artifact_bindings.slots.insert(
                    content.slot.clone(),
                    SlotExecutionBinding::StagedAsset { asset: handle },
                );
            }
        }
    }
    Ok(sources)
}

fn photometric_name(value: super::PhotometricInterpretation) -> &'static str {
    match value {
        super::PhotometricInterpretation::Monochrome1 => "MONOCHROME1",
        super::PhotometricInterpretation::Monochrome2 => "MONOCHROME2",
        super::PhotometricInterpretation::PaletteColor => "PALETTE COLOR",
        super::PhotometricInterpretation::Rgb => "RGB",
        super::PhotometricInterpretation::YbrFull => "YBR_FULL",
        super::PhotometricInterpretation::YbrFull422 => "YBR_FULL_422",
    }
}

fn plan_spec_providers(
    spec: &CompositionSpec,
    templates: &[&TemplateDescriptor],
    identity_plans: &BTreeMap<String, super::IdentityPlan>,
) -> Result<
    (
        BTreeMap<String, ArtifactExecutionBindings>,
        BTreeMap<String, DeferredCompositionProvider>,
    ),
    ComposeError,
> {
    let mut bindings = BTreeMap::new();
    let mut providers = BTreeMap::new();
    for (instance, template) in spec.instances.iter().zip(templates) {
        for assignment in &instance.content {
            let ContentSource::Provider {
                provider_id,
                provider_version,
                executable,
                executable_sha256,
                arguments,
                timeout_ms,
                size_bytes,
                sha256,
                media_type,
                pixel,
                parameters,
            } = assignment.source.clone()
            else {
                continue;
            };
            let identities = identity_plans
                .get(&instance.instance_id)
                .expect("identity pass covers provider instance")
                .identities
                .clone();
            let mut request = ProviderRequest {
                protocol_version: super::CONTENT_PROVIDER_PROTOCOL_VERSION.into(),
                request_id: String::new(),
                provider_id: provider_id.clone(),
                expected_provider_version: provider_version,
                argument_sha256: super::provider_arguments_sha256(&arguments),
                instance_id: instance.instance_id.clone(),
                template_id: template.template_id.0.clone(),
                template_version: template.template_version.to_string(),
                identities,
                output: ProviderOutputDeclaration {
                    slot: assignment.slot.clone(),
                    size_bytes,
                    sha256,
                    max_size_bytes: spec.resource_limits.max_file_bytes,
                    media_type: media_type.clone(),
                    pixel: pixel.as_ref().map(|declaration| {
                        serde_json::to_value(declaration).expect("pixel serializes")
                    }),
                },
                parameters,
                network_policy: "disabled".into(),
            };
            request.request_id = request.canonical_request_id();
            let invocation = ProviderInvocation {
                executable: PathBuf::from(executable),
                executable_sha256,
                arguments,
                timeout: Duration::from_millis(timeout_ms),
            };
            let executor_request = ExecutorProviderRequest {
                request_id: request.request_id.clone(),
                artifact_id: instance.instance_id.clone(),
                provider_id,
                required_version: request.expected_provider_version.clone(),
                parameters: request.parameters.clone(),
                input_assets: BTreeMap::new(),
                expected_outputs: vec![ProviderOutputExpectation {
                    slot: assignment.slot.clone(),
                    media_type: media_type.unwrap_or_else(|| "application/octet-stream".into()),
                    maximum_size_bytes: spec.resource_limits.max_file_bytes,
                    expected_sha256: Some(request.output.sha256.clone()),
                }],
            };
            let artifact_bindings =
                bindings
                    .entry(instance.instance_id.clone())
                    .or_insert_with(|| ArtifactExecutionBindings {
                        artifact_id: instance.instance_id.clone(),
                        slots: BTreeMap::new(),
                    });
            artifact_bindings.slots.insert(
                assignment.slot.clone(),
                SlotExecutionBinding::ProviderRequest {
                    request: executor_request,
                },
            );
            providers.insert(
                request.request_id.clone(),
                DeferredCompositionProvider {
                    request,
                    invocation,
                },
            );
        }
    }
    Ok((bindings, providers))
}

fn check_cancelled(cancellation: &ComposeCancellationToken) -> Result<(), ComposeError> {
    if cancellation.is_cancelled() {
        Err(ComposeError::Cancelled)
    } else {
        Ok(())
    }
}

fn map_executor_error(error: CorpusExecutorError, limits: &super::ResourceLimits) -> ComposeError {
    use crate::executor::engine::ArtifactExecutionError;
    use crate::executor::scheduler::SchedulerError;

    match error {
        CorpusExecutorError::Cancelled(_)
        | CorpusExecutorError::Scheduler(SchedulerError::Cancelled)
        | CorpusExecutorError::Scheduler(SchedulerError::Worker {
            source: ArtifactExecutionError::Cancelled(_),
            ..
        }) => ComposeError::Cancelled,
        CorpusExecutorError::Scheduler(SchedulerError::Worker {
            source: ArtifactExecutionError::Service(service),
            ..
        }) if service.stage == "provider" => {
            ComposeError::Provider(super::ProviderError::Invalid {
                message: service.message,
            })
        }
        CorpusExecutorError::Scheduler(SchedulerError::ResourceLimitExceeded {
            observed, ..
        }) => ComposeError::OutputLimit {
            size: observed.total_output_bytes,
            limit: limits.max_total_output_bytes,
        },
        CorpusExecutorError::PrimaryAndCleanup { primary, .. } => {
            map_executor_error(*primary, limits)
        }
        other => {
            let message = other.to_string();
            if message.contains("ResourceLimitExceeded")
                || message.contains("ArtifactOutputLimitExceeded")
                || message.contains("RunResourceLimitExceeded")
            {
                ComposeError::OutputLimit {
                    size: limits.max_total_output_bytes.saturating_add(1),
                    limit: limits.max_total_output_bytes,
                }
            } else {
                ComposeError::Executor(message)
            }
        }
    }
}

fn validate_parameters(
    instance: &super::SpecInstance,
    template: &TemplateDescriptor,
) -> Result<(), ComposeError> {
    let validator = jsonschema::validator_for(&template.parameter_schema).map_err(|error| {
        ComposeError::ParameterSchema {
            instance_id: instance.instance_id.clone(),
            message: format!("template parameter schema is invalid: {error}"),
        }
    })?;
    let parameters = serde_json::to_value(&instance.parameters).expect("parameters serialize");
    let errors = validator
        .iter_errors(&parameters)
        .map(|error| error.to_string())
        .collect::<Vec<_>>();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(ComposeError::ParameterSchema {
            instance_id: instance.instance_id.clone(),
            message: errors.join("; "),
        })
    }
}

fn apply_explicit_identities(
    instance: &super::SpecInstance,
    identities: &mut BTreeMap<String, String>,
) -> Result<(), ComposeError> {
    for (name, choice) in &instance.identities {
        let role = identity_key(name)?;
        match choice {
            IdentityChoice::Explicit { uid } => {
                identities.insert(role.into(), uid.clone());
            }
            IdentityChoice::Auto { auto } if *auto => {}
            IdentityChoice::Shared { .. } => {}
            IdentityChoice::Auto { .. } => {
                return Err(ComposeError::InvalidAutoIdentity(name.clone()));
            }
        }
    }
    Ok(())
}

fn apply_shared_identities(
    spec: &CompositionSpec,
    identities: &mut BTreeMap<String, super::IdentityPlan>,
) -> Result<(), ComposeError> {
    for instance in &spec.instances {
        for (name, choice) in &instance.identities {
            let IdentityChoice::Shared { share_with } = choice else {
                continue;
            };
            let key = identity_key(name)?;
            let shared = identities
                .get(share_with)
                .and_then(|plan| plan.identities.get(key))
                .cloned()
                .ok_or_else(|| ComposeError::UnknownSharedIdentity {
                    instance_id: instance.instance_id.clone(),
                    share_with: share_with.clone(),
                    identity: name.clone(),
                })?;
            identities
                .get_mut(&instance.instance_id)
                .expect("identity pass covered instance")
                .identities
                .insert(key.into(), shared);
        }
    }
    Ok(())
}

fn identity_key(name: &str) -> Result<&'static str, ComposeError> {
    match name {
        "study" => Ok("study_instance_uid#0"),
        "series" => Ok("series_instance_uid#0"),
        "sop_instance" => Ok("sop_instance_uid#0"),
        "frame_of_reference" => Ok("frame_of_reference_uid#0"),
        "patient" => Err(ComposeError::UnsupportedPatientIdentity),
        other => Err(ComposeError::UnknownIdentity(other.into())),
    }
}

fn validate_reference_roles(
    instance: &super::SpecInstance,
    template: &TemplateDescriptor,
) -> Result<(), ComposeError> {
    let declared = template
        .reference_slots
        .iter()
        .filter_map(|slot| slot["role"].as_str())
        .collect::<std::collections::BTreeSet<_>>();
    for reference in &instance.references {
        if !declared.contains(reference.role.as_str()) {
            return Err(ComposeError::UnknownReferenceRole {
                instance_id: instance.instance_id.clone(),
                role: reference.role.clone(),
            });
        }
    }
    Ok(())
}

fn materialize_reference_graph(
    plans: &mut [ResolvedInstancePlan],
    spec: &CompositionSpec,
    members: &BTreeMap<String, super::BundleMemberProvenance>,
) -> Result<(), ComposeError> {
    let nodes = plans
        .iter()
        .map(|plan| {
            Ok(ReferenceNode {
                instance_id: plan.instance_id.clone(),
                bundle_id: members
                    .get(&plan.instance_id)
                    .expect("bundle member covers plan")
                    .bundle_root_instance_id
                    .clone(),
                sop_class_uid: plan.sop_class_uid.clone(),
                frames: resolved_frame_count(plan)?,
            })
        })
        .collect::<Result<Vec<_>, ComposeError>>()?;
    let edges = spec
        .instances
        .iter()
        .flat_map(|instance| {
            instance
                .references
                .iter()
                .map(|reference| LogicalReference {
                    source_instance_id: instance.instance_id.clone(),
                    target_instance_id: reference.target_instance_id.clone(),
                    role: reference.role.clone(),
                    frame_role: (!reference.frames.is_empty()).then(|| "referenced_frame".into()),
                    frames: reference.frames.clone(),
                    cycle_policy: CyclePolicy::Forbidden,
                })
        })
        .collect::<Vec<_>>();
    let graph = ReferenceGraph::new(nodes, edges)?;
    let identities = plans
        .iter()
        .map(|plan| (plan.instance_id.clone(), plan.identities.clone()))
        .collect::<BTreeMap<_, _>>();
    let references = graph.materialize(&identities)?;
    for plan in plans {
        plan.references = references
            .iter()
            .filter(|reference| reference.source_instance_id == plan.instance_id)
            .cloned()
            .collect();
    }
    Ok(())
}

fn resolved_frame_count(plan: &ResolvedInstancePlan) -> Result<u32, ComposeError> {
    let Some(attribute) = plan
        .attributes
        .iter()
        .find(|attribute| attribute.address.normalized_tag() == "0028,0008")
    else {
        return Ok(1);
    };
    let frame_count = match attribute.value.as_ref() {
        Some(super::AttributeValue::Primitive(super::PrimitiveValue::String(value))) => {
            value.trim().parse::<u32>().ok()
        }
        Some(super::AttributeValue::Primitive(super::PrimitiveValue::Unsigned(value))) => {
            u32::try_from(*value).ok()
        }
        Some(super::AttributeValue::Primitive(super::PrimitiveValue::Signed(value))) => {
            u32::try_from(*value).ok()
        }
        _ => None,
    };
    frame_count
        .filter(|count| *count > 0)
        .ok_or_else(|| ComposeError::InvalidResolvedFrameCount {
            instance_id: plan.instance_id.clone(),
        })
}

fn resolve_sc_pixels(
    instance: &super::SpecInstance,
    template: &TemplateDescriptor,
    resolver: &mut LocalContentResolver,
) -> Result<DefaultPixelOutput, ComposeError> {
    if instance.content.len() > 1 {
        return Err(ComposeError::ContentCardinality(
            instance.instance_id.clone(),
        ));
    }
    let Some(assignment) = instance.content.first() else {
        return Ok(sc_default_pixels(&template.template_id)?);
    };
    if assignment.slot != "pixels" {
        return Err(ComposeError::UnknownContentSlot(assignment.slot.clone()));
    }
    match &assignment.source {
        ContentSource::Default => Ok(sc_default_pixels(&template.template_id)?),
        ContentSource::LocalFile {
            path,
            sha256,
            pixel: Some(pixel),
            ..
        } => {
            let output =
                resolve_raw_native_pixels(resolver, path, sha256.as_deref(), pixel.shape()?)?;
            Ok(DefaultPixelOutput {
                plan: output.plan,
                content: output.content,
            })
        }
        ContentSource::LocalFile { pixel: None, .. } => Err(ComposeError::MissingPixelDeclaration(
            instance.instance_id.clone(),
        )),
        ContentSource::InlineSmallFixture {
            base64,
            sha256,
            pixel: Some(pixel),
            ..
        } => {
            let bytes = super::spec::decode_base64(base64)?;
            let asset =
                resolver.resolve_inline("pixels", "native_pixels", &bytes, sha256.as_deref())?;
            let output =
                super::native_content::resolve_staged_native_pixels(asset, pixel.shape()?)?;
            Ok(DefaultPixelOutput {
                plan: output.plan,
                content: output.content,
            })
        }
        ContentSource::InlineSmallFixture { pixel: None, .. } => Err(
            ComposeError::MissingPixelDeclaration(instance.instance_id.clone()),
        ),
        ContentSource::ResolvedProvider {
            output,
            pixel: Some(pixel),
            ..
        } => {
            let asset = resolver.resolve_provider("pixels", "native_pixels", output)?;
            let output =
                super::native_content::resolve_staged_native_pixels(asset, pixel.shape()?)?;
            Ok(DefaultPixelOutput {
                plan: output.plan,
                content: output.content,
            })
        }
        ContentSource::ResolvedProvider { pixel: None, .. } => Err(
            ComposeError::MissingPixelDeclaration(instance.instance_id.clone()),
        ),
        ContentSource::Provider {
            provider_id,
            provider_version,
            executable_sha256,
            arguments,
            size_bytes,
            sha256,
            pixel: Some(pixel),
            ..
        } => resolve_declared_provider_pixels(
            provider_id,
            provider_version,
            executable_sha256,
            arguments,
            *size_bytes,
            sha256,
            pixel,
        ),
        ContentSource::Provider { pixel: None, .. } => Err(ComposeError::MissingPixelDeclaration(
            instance.instance_id.clone(),
        )),
        _ => Err(ComposeError::UnsupportedP2Content(
            instance.instance_id.clone(),
        )),
    }
}

fn resolve_family_pixels(
    instance: &super::SpecInstance,
    profile: &super::ClassicFamilyProfile,
    transfer_syntax_uid: &str,
    resolver: &mut LocalContentResolver,
) -> Result<DefaultPixelOutput, ComposeError> {
    if instance.content.len() > 1 {
        return Err(ComposeError::ContentCardinality(
            instance.instance_id.clone(),
        ));
    }
    let Some(assignment) = instance.content.first() else {
        let (plan, content) = default_family_pixels(profile)?;
        return Ok(DefaultPixelOutput { plan, content });
    };
    if assignment.slot != "pixels" {
        return Err(ComposeError::UnknownContentSlot(assignment.slot.clone()));
    }
    match &assignment.source {
        ContentSource::Default => {
            let (plan, content) = default_family_pixels(profile)?;
            Ok(DefaultPixelOutput { plan, content })
        }
        ContentSource::LocalFile {
            path,
            sha256,
            pixel: Some(pixel),
            ..
        } => {
            let output =
                resolve_raw_native_pixels(resolver, path, sha256.as_deref(), pixel.shape()?)?;
            Ok(DefaultPixelOutput {
                plan: output.plan,
                content: output.content,
            })
        }
        ContentSource::LocalFile { pixel: None, .. } => Err(ComposeError::MissingPixelDeclaration(
            instance.instance_id.clone(),
        )),
        ContentSource::InlineSmallFixture {
            base64,
            sha256,
            pixel: Some(pixel),
            ..
        } => {
            let bytes = super::spec::decode_base64(base64)?;
            let asset =
                resolver.resolve_inline("pixels", "native_pixels", &bytes, sha256.as_deref())?;
            let output =
                super::native_content::resolve_staged_native_pixels(asset, pixel.shape()?)?;
            Ok(DefaultPixelOutput {
                plan: output.plan,
                content: output.content,
            })
        }
        ContentSource::InlineSmallFixture { pixel: None, .. } => Err(
            ComposeError::MissingPixelDeclaration(instance.instance_id.clone()),
        ),
        ContentSource::EncodedFrames {
            transfer_syntax_uid: source_transfer_syntax_uid,
            frames,
            pixel: Some(pixel),
        } => resolve_encoded_rle_pixels(
            resolver,
            source_transfer_syntax_uid,
            transfer_syntax_uid,
            frames,
            pixel,
        ),
        ContentSource::EncodedFrames { pixel: None, .. } => Err(
            ComposeError::MissingPixelDeclaration(instance.instance_id.clone()),
        ),
        ContentSource::ResolvedProvider {
            output,
            pixel: Some(pixel),
            ..
        } => {
            let asset = resolver.resolve_provider("pixels", "native_pixels", output)?;
            let output =
                super::native_content::resolve_staged_native_pixels(asset, pixel.shape()?)?;
            Ok(DefaultPixelOutput {
                plan: output.plan,
                content: output.content,
            })
        }
        ContentSource::ResolvedProvider { pixel: None, .. } => Err(
            ComposeError::MissingPixelDeclaration(instance.instance_id.clone()),
        ),
        ContentSource::Provider {
            provider_id,
            provider_version,
            executable_sha256,
            arguments,
            size_bytes,
            sha256,
            pixel: Some(pixel),
            ..
        } => resolve_declared_provider_pixels(
            provider_id,
            provider_version,
            executable_sha256,
            arguments,
            *size_bytes,
            sha256,
            pixel,
        ),
        ContentSource::Provider { pixel: None, .. } => Err(ComposeError::MissingPixelDeclaration(
            instance.instance_id.clone(),
        )),
    }
}

fn resolve_declared_provider_pixels(
    provider_id: &str,
    provider_version: &str,
    executable_sha256: &str,
    arguments: &[String],
    size_bytes: u64,
    sha256: &str,
    pixel: &super::PixelDeclaration,
) -> Result<DefaultPixelOutput, ComposeError> {
    let shape = pixel.shape()?;
    let plan = super::NativePixelPlan::plan(shape)?;
    if size_bytes != plan.unpadded_value_bytes {
        return Err(ComposeError::RawContent(super::RawContentError::Length {
            path: format!("providers/{provider_id}/pixels"),
            expected: plan.unpadded_value_bytes,
            actual: size_bytes,
        }));
    }
    let vr = if plan.shape.bits_allocated <= 8 {
        super::DicomVr::OB
    } else {
        super::DicomVr::OW
    };
    Ok(DefaultPixelOutput {
        plan,
        content: super::CanonicalContent {
            slot: "pixels".into(),
            kind: "native_pixels".into(),
            address: super::AttributeAddress::from_normalized_tag("7FE0,0010")
                .expect("Pixel Data is a known tag"),
            vr,
            size_bytes,
            sha256: sha256.into(),
            properties: BTreeMap::from([
                ("content_origin".into(), "provider".into()),
                ("provider_id".into(), provider_id.into()),
                ("provider_version".into(), provider_version.into()),
                (
                    "provider_executable_sha256".into(),
                    executable_sha256.into(),
                ),
                (
                    "provider_argument_sha256".into(),
                    super::provider_arguments_sha256(arguments),
                ),
                (
                    "provider_protocol_version".into(),
                    super::CONTENT_PROVIDER_PROTOCOL_VERSION.into(),
                ),
                ("provider_network_policy".into(), "disabled".into()),
                (
                    "spec_relative_path".into(),
                    format!("providers/{provider_id}/pixels"),
                ),
                ("staging_method".into(), "stream_copy".into()),
                (
                    "pixel_shape".into(),
                    serde_json::to_string(&pixel.shape()?).expect("pixel shape serializes"),
                ),
            ]),
            placement: super::ContentPlacement::TopLevel,
            materialization: None,
        },
    })
}

fn validate_family_pixel_contract(
    profile: &super::ClassicFamilyProfile,
    pixel: &DefaultPixelOutput,
) -> Result<(), ComposeError> {
    let actual = &pixel.plan.shape;
    let expected = &profile.default_shape;
    let multiframe = matches!(
        profile.kind,
        super::ClassicFamilyKind::ScSingleBit
            | super::ClassicFamilyKind::ScGrayscaleByte
            | super::ClassicFamilyKind::UltrasoundMultiFrame
            | super::ClassicFamilyKind::NuclearMedicine
    );
    let allowed = actual.rows > 0
        && actual.columns > 0
        && if multiframe {
            actual.frames > 0
        } else {
            actual.frames == 1
        }
        && actual.samples_per_pixel == expected.samples_per_pixel
        && actual.photometric_interpretation == expected.photometric_interpretation
        && actual.sample_type == expected.sample_type
        && actual.bits_allocated == expected.bits_allocated
        && actual.bits_stored == expected.bits_stored
        && actual.high_bit == expected.high_bit
        && actual.byte_order == super::ByteOrder::Little
        && actual.planar_configuration == expected.planar_configuration;
    if allowed {
        Ok(())
    } else {
        Err(ComposeError::PixelContract(profile.template_id.0.clone()))
    }
}

fn resolve_encoded_rle_pixels(
    resolver: &mut LocalContentResolver,
    source_transfer_syntax_uid: &str,
    output_transfer_syntax_uid: &str,
    frames: &[super::EncodedFrame],
    declaration: &super::PixelDeclaration,
) -> Result<DefaultPixelOutput, ComposeError> {
    if source_transfer_syntax_uid != crate::codecs::RLE_LOSSLESS_TRANSFER_SYNTAX_UID
        || output_transfer_syntax_uid != source_transfer_syntax_uid
    {
        return Err(ComposeError::EncodedTransferSyntaxMismatch {
            source: source_transfer_syntax_uid.into(),
            output: output_transfer_syntax_uid.into(),
        });
    }
    let shape = declaration.shape()?;
    if frames.len() != usize::try_from(shape.frames).map_err(|_| ComposeError::ResourceRange)? {
        return Err(ComposeError::EncodedFrameCount {
            expected: shape.frames,
            actual: frames.len(),
        });
    }
    let plan = super::NativePixelPlan::plan(shape.clone())?;
    let photometric = match shape.photometric_interpretation {
        super::PhotometricInterpretation::Monochrome1 => "MONOCHROME1",
        super::PhotometricInterpretation::Monochrome2 => "MONOCHROME2",
        super::PhotometricInterpretation::PaletteColor => "PALETTE COLOR",
        super::PhotometricInterpretation::Rgb => "RGB",
        super::PhotometricInterpretation::YbrFull => "YBR_FULL",
        super::PhotometricInterpretation::YbrFull422 => "YBR_FULL_422",
    };
    let decoder = NativeRleLosslessEncoder::new();
    let mut encoded = Vec::with_capacity(frames.len());
    let mut encoded_frame_assets = Vec::with_capacity(frames.len());
    let mut decoded_frame_sha256 = Vec::with_capacity(frames.len());
    for (index, frame) in frames.iter().enumerate() {
        let asset = resolver.resolve(
            "pixels",
            "encoded_frame",
            Path::new(&frame.path),
            frame.sha256.as_deref(),
        )?;
        encoded_frame_assets.push(json!({
            "spec_relative_path": asset.spec_relative_path,
            "size_bytes": asset.size_bytes,
            "sha256": asset.sha256
        }));
        let bytes = fs::read(&asset.staged_path).map_err(|source| ComposeError::Io {
            path: asset.staged_path,
            source,
        })?;
        let decoded = decoder.decode_frame(FrameDecodeInput {
            encoded_frame: &bytes,
            rows: u16::try_from(shape.rows).map_err(|_| ComposeError::ResourceRange)?,
            columns: u16::try_from(shape.columns).map_err(|_| ComposeError::ResourceRange)?,
            samples_per_pixel: u16::from(shape.samples_per_pixel),
            bits_allocated: u16::from(shape.bits_allocated),
            bits_stored: u16::from(shape.bits_stored),
            photometric_interpretation: photometric,
        })?;
        let expected_frame_bytes = plan.frame_spans[index].bit_length.div_ceil(8);
        if decoded.native_bytes.len() as u64 != expected_frame_bytes {
            return Err(ComposeError::EncodedDecodedLength {
                frame: index,
                expected: expected_frame_bytes,
                actual: decoded.native_bytes.len() as u64,
            });
        }
        decoded_frame_sha256.push(sha256_hex(&decoded.native_bytes));
        encoded.push(bytes);
    }
    let encapsulated =
        EncapsulatedPixelData::one_fragment_per_frame(&encoded, BasicOffsetTablePolicy::Populated)?;
    let compressed_frame_sha256 = encapsulated.compressed_frame_hashes.clone();
    let basic_offset_table = encapsulated.basic_offset_table.offsets;
    let mut fragments = encapsulated.fragment_payloads;
    for fragment in &mut fragments {
        if fragment.len() % 2 != 0 {
            fragment.push(0);
        }
    }
    let content_bytes = fragments.concat();
    let content = super::CanonicalContent {
        slot: "pixels".into(),
        kind: "encapsulated_pixels".into(),
        address: super::AttributeAddress::from_normalized_tag("7FE0,0010")
            .expect("Pixel Data is a known tag"),
        vr: super::DicomVr::OB,
        size_bytes: content_bytes.len() as u64,
        sha256: sha256_hex(&content_bytes),
        properties: BTreeMap::from([
            ("content_origin".into(), "encoded_frames".into()),
            (
                "codec_backend".into(),
                NativeRleLosslessEncoder::BACKEND_ID.into(),
            ),
            (
                "codec_semantic_validation".into(),
                "independent_decode_passed".into(),
            ),
            (
                "compressed_frame_sha256".into(),
                serde_json::to_string(&compressed_frame_sha256).expect("hashes serialize"),
            ),
            (
                "decoded_frame_sha256".into(),
                serde_json::to_string(&decoded_frame_sha256).expect("hashes serialize"),
            ),
            (
                "source_transfer_syntax_uid".into(),
                source_transfer_syntax_uid.into(),
            ),
            (
                "encoded_frame_assets".into(),
                serde_json::to_string(&encoded_frame_assets).expect("assets serialize"),
            ),
        ]),
        placement: super::ContentPlacement::TopLevel,
        materialization: Some(super::ContentMaterialization::Encapsulated {
            basic_offset_table,
            fragments,
        }),
    };
    Ok(DefaultPixelOutput { plan, content })
}

fn validate_sc_pixel_contract(
    template: &TemplateDescriptor,
    pixel: &DefaultPixelOutput,
) -> Result<(), ComposeError> {
    let shape = &pixel.plan.shape;
    let mono = template.template_id.0.ends_with("/monochrome");
    let allowed = if mono {
        shape.samples_per_pixel == 1
            && matches!(
                shape.photometric_interpretation,
                super::PhotometricInterpretation::Monochrome1
                    | super::PhotometricInterpretation::Monochrome2
            )
            && matches!(shape.bits_allocated, 8 | 16)
            && shape.sample_type == super::SampleType::UnsignedInteger
            && shape.planar_configuration.is_none()
    } else {
        shape.samples_per_pixel == 3
            && shape.photometric_interpretation == super::PhotometricInterpretation::Rgb
            && shape.bits_allocated == 8
            && shape.sample_type == super::SampleType::UnsignedInteger
            && shape.planar_configuration.is_some()
    };
    if allowed && shape.byte_order == super::ByteOrder::Little {
        Ok(())
    } else {
        Err(ComposeError::PixelContract(template.template_id.0.clone()))
    }
}

fn reject_structural_overrides(
    instance_id: &str,
    operations: &[super::AttributeOperation],
) -> Result<(), ComposeError> {
    if let Some(operation) = operations.iter().find(|operation| {
        matches!(
            operation.address().normalized_tag().as_str(),
            "7FE0,0010" | "7FE0,0008" | "7FE0,0009"
        )
    }) {
        return Err(ComposeError::ProtectedContentOverride {
            instance_id: instance_id.into(),
            tag: operation.address().normalized_tag(),
        });
    }
    if operations
        .iter()
        .any(|operation| operation.address().normalized_tag() == "0008,001C")
    {
        return Err(ComposeError::ProtectedSyntheticDataOverride {
            instance_id: instance_id.into(),
        });
    }
    Ok(())
}

fn resource_map(limits: &super::ResourceLimits) -> BTreeMap<String, u64> {
    BTreeMap::from([
        ("max_spec_bytes".into(), limits.max_spec_bytes),
        ("max_instances".into(), limits.max_instances),
        ("max_input_files".into(), limits.max_input_files),
        ("max_file_bytes".into(), limits.max_file_bytes),
        ("max_total_input_bytes".into(), limits.max_total_input_bytes),
        (
            "max_total_output_bytes".into(),
            limits.max_total_output_bytes,
        ),
        (
            "max_attributes_per_instance".into(),
            limits.max_attributes_per_instance,
        ),
        ("max_sequence_items".into(), limits.max_sequence_items),
        (
            "max_value_multiplicity".into(),
            limits.max_value_multiplicity,
        ),
        (
            "max_content_assignments_per_instance".into(),
            limits.max_content_assignments_per_instance,
        ),
        (
            "max_references_per_instance".into(),
            limits.max_references_per_instance,
        ),
        ("max_parameter_nodes".into(), limits.max_parameter_nodes),
        ("max_parameter_depth".into(), limits.max_parameter_depth),
    ])
}

#[derive(Debug)]
pub enum ComposeError {
    OutputExists(PathBuf),
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    Spec(super::SpecError),
    Template(super::TemplateError),
    Identity(super::IdentityError),
    Content(super::ContentError),
    RawContent(super::RawContentError),
    Pixel(super::PixelError),
    Defaults(super::DefaultError),
    Materialize(super::MaterializeError),
    Manifest(super::ManifestError),
    Family(super::FamilyError),
    AdvancedFamily(super::AdvancedFamilyError),
    Bundle(super::BundleError),
    Reference(super::ReferenceError),
    Codec(crate::codecs::CodecError),
    Encapsulation(crate::encapsulation::EncapsulationError),
    Provider(super::ProviderError),
    CorpusPlan(crate::corpus_plan::CorpusPlanError),
    AdvancedDefaults(String),
    ExecutionBinding(String),
    Executor(String),
    ExecutorManifest(String),
    Cancelled,
    ResourceRange,
    UnsupportedTemplate(String),
    UnsupportedP2Content(String),
    UnsupportedFamilyContent(String),
    UnsupportedScReference(String),
    UnknownReferenceRole {
        instance_id: String,
        role: String,
    },
    InvalidResolvedFrameCount {
        instance_id: String,
    },
    UnsupportedTransferSyntax {
        instance_id: String,
        uid: String,
    },
    UnsupportedPatientIdentity,
    UnknownIdentity(String),
    InvalidAutoIdentity(String),
    UnknownSharedIdentity {
        instance_id: String,
        share_with: String,
        identity: String,
    },
    ContentCardinality(String),
    UnknownContentSlot(String),
    MissingPixelDeclaration(String),
    PixelContract(String),
    ProtectedContentOverride {
        instance_id: String,
        tag: String,
    },
    ProtectedSyntheticDataOverride {
        instance_id: String,
    },
    ParameterSchema {
        instance_id: String,
        message: String,
    },
    OutputSizeOverflow,
    MissingPixelMaterialization,
    CodecPixelAlignment,
    CodecRead {
        frame: usize,
        source: std::io::Error,
    },
    EncodedTransferSyntaxMismatch {
        source: String,
        output: String,
    },
    EncodedFrameCount {
        expected: u32,
        actual: usize,
    },
    EncodedDecodedLength {
        frame: usize,
        expected: u64,
        actual: u64,
    },
    OutputLimit {
        size: u64,
        limit: u64,
    },
}

macro_rules! from_error {
    ($source:ty, $variant:ident) => {
        impl From<$source> for ComposeError {
            fn from(error: $source) -> Self {
                Self::$variant(error)
            }
        }
    };
}
from_error!(super::SpecError, Spec);
from_error!(super::TemplateError, Template);
from_error!(super::IdentityError, Identity);
from_error!(super::ContentError, Content);
from_error!(super::RawContentError, RawContent);
from_error!(super::PixelError, Pixel);
from_error!(super::DefaultError, Defaults);
from_error!(super::MaterializeError, Materialize);
from_error!(super::ManifestError, Manifest);
from_error!(super::FamilyError, Family);
from_error!(super::AdvancedFamilyError, AdvancedFamily);
from_error!(super::BundleError, Bundle);
from_error!(super::ReferenceError, Reference);
from_error!(crate::codecs::CodecError, Codec);
from_error!(crate::encapsulation::EncapsulationError, Encapsulation);
from_error!(super::ProviderError, Provider);
from_error!(crate::corpus_plan::CorpusPlanError, CorpusPlan);

impl fmt::Display for ComposeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for ComposeError {}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;

    static NEXT: AtomicU64 = AtomicU64::new(0);

    fn output(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "dts-compose-run-{label}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ))
    }

    #[test]
    fn caller_content_planning_writes_nothing_and_plan_contains_no_paths() {
        let root = output("planning-read-only");
        fs::create_dir(&root).unwrap();
        let pixels = [1_u8, 2, 3, 4];
        fs::write(root.join("pixels.raw"), pixels).unwrap();
        let spec_value = json!({
            "composition_spec_schema_version": "0.1.0",
            "instances": [
                {
                    "instance_id": "local",
                    "template": {"id": "classic/secondary-capture/monochrome"},
                    "content": [{"slot": "pixels", "source": {
                        "kind": "local_file", "path": "pixels.raw",
                        "sha256": sha256_hex(&pixels),
                        "pixel": {"rows":2,"columns":2,"frames":1,"samples_per_pixel":1,
                            "photometric_interpretation":"MONOCHROME2","sample_type":"uint",
                            "bits_allocated":8,"bits_stored":8,"high_bit":7,"byte_order":"little"}
                    }}]
                },
                {
                    "instance_id": "inline",
                    "template": {"id": "classic/secondary-capture/monochrome"},
                    "content": [{"slot": "pixels", "source": {
                        "kind": "inline_small_fixture", "base64": "AQIDBA==",
                        "sha256": sha256_hex(&pixels),
                        "pixel": {"rows":2,"columns":2,"frames":1,"samples_per_pixel":1,
                            "photometric_interpretation":"MONOCHROME2","sample_type":"uint",
                            "bits_allocated":8,"bits_stored":8,"high_bit":7,"byte_order":"little"}
                    }}]
                }
            ]
        });
        let spec_bytes = serde_json::to_vec(&spec_value).unwrap();
        let catalog_bytes = fs::read("templates/catalog.json").unwrap();
        let spec = CompositionSpec::from_slice(&spec_bytes).unwrap();
        let catalog = TemplateCatalog::from_slice(&catalog_bytes).unwrap();
        let before = fs::read_dir(&root).unwrap().count();
        let planned = resolve_execution_bundle(
            &ComposeOptions {
                spec_path: root.join("spec.json"),
                out_dir: root.join("out"),
                seed: 1,
                catalog_path: "templates/catalog.json".into(),
                dry_run: false,
            },
            &spec,
            &catalog,
            &spec_bytes,
            &catalog_bytes,
            &root,
            &root,
            &ComposeCancellationToken::new(),
        )
        .unwrap();

        assert_eq!(fs::read_dir(&root).unwrap().count(), before);
        assert_eq!(planned.bundle.source_assets.len(), 2);
        for artifact in &planned.bundle.plan.artifacts {
            let crate::corpus_plan::PlannedArtifact::Dicom(artifact) = artifact else {
                panic!("composition caller content should plan DICOM artifacts")
            };
            assert!(
                artifact
                    .instance
                    .content
                    .iter()
                    .all(|content| content.materialization.is_none())
            );
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn external_quantitative_composition_artifacts_have_no_curated_case_binding() {
        let spec_bytes =
            fs::read("tests/fixtures/composition/valid/p6-quantitative-defaults.json").unwrap();
        let catalog_bytes = fs::read("templates/catalog.json").unwrap();
        let spec = CompositionSpec::from_slice(&spec_bytes).unwrap();
        let catalog = TemplateCatalog::from_slice(&catalog_bytes).unwrap();
        let root = output("external-quantitative-plan");
        let planned = resolve_execution_bundle(
            &ComposeOptions {
                spec_path: "tests/fixtures/composition/valid/p6-quantitative-defaults.json".into(),
                out_dir: root.clone(),
                seed: 69,
                catalog_path: "templates/catalog.json".into(),
                dry_run: false,
            },
            &spec,
            &catalog,
            &spec_bytes,
            &catalog_bytes,
            Path::new("tests/fixtures/composition/valid"),
            Path::new("tests/fixtures/composition/valid"),
            &ComposeCancellationToken::new(),
        )
        .unwrap();
        let imported = planned
            .bundle
            .plan
            .artifacts
            .iter()
            .filter_map(|artifact| match artifact {
                crate::corpus_plan::PlannedArtifact::ImportedDicom(imported) => Some(imported),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(imported.len(), 3);
        assert!(
            imported
                .iter()
                .all(|artifact| artifact.case_binding.is_none())
        );
        assert!(!root.exists());
    }

    #[test]
    fn default_spec_resolves_writes_validates_and_promotes_once() {
        let out = output("default");
        let (summary, manifest) = compose(&ComposeOptions {
            spec_path: "tests/fixtures/composition/valid/template-only.json".into(),
            out_dir: out.clone(),
            seed: 5,
            catalog_path: "templates/catalog.json".into(),
            dry_run: false,
        })
        .unwrap();
        assert_eq!(summary.instances_written, 1);
        assert!(out.join("instances/primary.dcm").is_file());
        assert_eq!(manifest["run"]["kind"], "composition");
        assert_eq!(
            manifest["composition"]["publication"]["atomic_promotion"],
            "complete"
        );
        assert_eq!(
            manifest["composition"]["publication"]["cleanup_complete"],
            true
        );
        let published: Value =
            serde_json::from_slice(&fs::read(out.join("manifest.json")).unwrap()).unwrap();
        assert_eq!(published, manifest);
        fs::remove_dir_all(out).unwrap();
    }

    #[test]
    fn default_bundle_dependency_is_generated_and_reference_closed() {
        let root = output("default-bundle");
        fs::create_dir(&root).unwrap();
        let catalog = TemplateCatalog::load("templates/catalog.json").unwrap();
        let mut catalog_value = serde_json::to_value(catalog).unwrap();
        let templates = catalog_value["templates"].as_array_mut().unwrap();
        let ct = templates
            .iter_mut()
            .find(|template| template["template_id"] == "classic/ct")
            .unwrap();
        ct["reference_slots"] = json!([{
            "role":"source_image", "required":true, "cardinality":"one",
            "target_sop_class_uids":["1.2.840.10008.5.1.4.1.1.1"],
            "frame_roles":[], "cycle_policy":"forbidden",
            "default_dependency":"classic/cr",
            "description":"Synthetic source image for bundle qualification."
        }]);
        ct["default_bundle"] = json!({"dependencies":[{
            "logical_role":"image_source", "reference_role":"source_image",
            "instance_suffix":"source", "template_id":"classic/cr",
            "template_version":"1.0.0", "share_study":true
        }]});
        let catalog_path = root.join("catalog.json");
        fs::write(
            &catalog_path,
            serde_json::to_vec_pretty(&catalog_value).unwrap(),
        )
        .unwrap();
        let spec_path = root.join("spec.json");
        fs::write(
            &spec_path,
            serde_json::to_vec_pretty(&json!({
                "composition_spec_schema_version":"0.1.0",
                "instances":[{"instance_id":"root","template":{"id":"classic/ct"}}]
            }))
            .unwrap(),
        )
        .unwrap();
        let out = root.join("out");
        let (summary, manifest) = compose(&ComposeOptions {
            spec_path,
            out_dir: out.clone(),
            seed: 6,
            catalog_path,
            dry_run: false,
        })
        .unwrap();
        assert_eq!(summary.instances_written, 2);
        assert!(out.join("instances/root__source.dcm").is_file());
        let entries = manifest["composition"]["entries"].as_array().unwrap();
        let source = entries
            .iter()
            .find(|entry| entry["instance_id"] == "root__source")
            .unwrap();
        assert_eq!(source["requested"], false);
        assert_eq!(source["source_provenance"], "default_template_dependency");
        assert_eq!(source["bundle_root_instance_id"], "root");
        assert_eq!(source["bundle_role"], "image_source");
        let root_entry = entries
            .iter()
            .find(|entry| entry["instance_id"] == "root")
            .unwrap();
        assert_eq!(
            root_entry["references"][0]["target_instance_id"],
            "root__source"
        );
        assert_eq!(
            root_entry["references"][0]["referenced_sop_instance_uid"],
            source["uids"]["sop_instance_uid#0"]
        );
        let bundle = &manifest["composition"]["bundles"][0];
        assert_eq!(bundle["bundle_root_instance_id"], "root");
        assert_eq!(bundle["members"].as_array().unwrap().len(), 2);
        assert_eq!(
            bundle["dependency_closure"],
            json!(["root", "root__source"])
        );
        assert_eq!(bundle["references"][0]["referenced_frames"], json!([]));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn dry_run_returns_canonical_plans_without_promoting_output() {
        let out = output("dry-run");
        let (summary, resolved) = compose(&ComposeOptions {
            spec_path: "tests/fixtures/composition/valid/template-only.json".into(),
            out_dir: out.clone(),
            seed: 5,
            catalog_path: "templates/catalog.json".into(),
            dry_run: true,
        })
        .unwrap();
        assert!(summary.dry_run);
        assert!(!out.exists());
        assert_eq!(resolved["plans"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn p3_3_defaults_materialize_all_qualified_family_profiles() {
        let out = output("p3-3");
        let (summary, manifest) = compose(&ComposeOptions {
            spec_path: "tests/fixtures/composition/valid/classic-p3-3-defaults.json".into(),
            out_dir: out.clone(),
            seed: 33,
            catalog_path: "templates/catalog.json".into(),
            dry_run: false,
        })
        .unwrap();
        assert_eq!(summary.instances_written, 3);
        assert_eq!(
            manifest["composition"]["entries"].as_array().unwrap().len(),
            3
        );
        for instance in ["cr", "ct", "mr"] {
            assert!(out.join(format!("instances/{instance}.dcm")).is_file());
        }
        fs::remove_dir_all(out).unwrap();
    }

    #[test]
    fn p3_3_caller_native_pixels_round_trip_and_wrong_model_is_rejected() {
        let root = output("p3-3-caller");
        fs::create_dir(&root).unwrap();
        let raw = (0_u16..256).flat_map(u16::to_le_bytes).collect::<Vec<_>>();
        for name in ["cr", "ct", "mr"] {
            fs::write(root.join(format!("{name}.raw")), &raw).unwrap();
        }
        let pixel = |sample_type: &str| {
            json!({
                "rows": 16, "columns": 16, "frames": 1,
                "samples_per_pixel": 1,
                "photometric_interpretation": "MONOCHROME2",
                "sample_type": sample_type,
                "bits_allocated": 16, "bits_stored": 12, "high_bit": 11,
                "byte_order": "little"
            })
        };
        let assignment = |name: &str, sample_type: &str| {
            json!([{
                "slot": "pixels",
                "source": {
                    "kind": "local_file",
                    "path": format!("{name}.raw"),
                    "sha256": sha256_hex(&raw),
                    "pixel": pixel(sample_type)
                }
            }])
        };
        let spec = json!({
            "composition_spec_schema_version": "0.1.0",
            "instances": [
                { "instance_id": "cr", "template": { "id": "classic/cr" }, "content": assignment("cr", "uint") },
                { "instance_id": "ct", "template": { "id": "classic/ct" }, "content": assignment("ct", "int") },
                { "instance_id": "mr", "template": { "id": "classic/mr" }, "content": assignment("mr", "uint") }
            ]
        });
        let spec_path = root.join("spec.json");
        fs::write(&spec_path, serde_json::to_vec_pretty(&spec).unwrap()).unwrap();
        let out = root.join("out");
        compose(&ComposeOptions {
            spec_path: spec_path.clone(),
            out_dir: out.clone(),
            seed: 34,
            catalog_path: "templates/catalog.json".into(),
            dry_run: false,
        })
        .unwrap();
        for name in ["cr", "ct", "mr"] {
            let object =
                dicom_object::open_file(out.join(format!("instances/{name}.dcm"))).unwrap();
            assert_eq!(
                object
                    .element_by_name("PixelData")
                    .unwrap()
                    .to_bytes()
                    .unwrap()
                    .as_ref(),
                raw.as_slice()
            );
        }

        let mut wrong = spec;
        wrong["instances"][1]["content"] = assignment("ct", "uint");
        let wrong_spec = root.join("wrong.json");
        fs::write(&wrong_spec, serde_json::to_vec_pretty(&wrong).unwrap()).unwrap();
        let wrong_out = root.join("wrong-out");
        assert!(matches!(
            compose(&ComposeOptions {
                spec_path: wrong_spec,
                out_dir: wrong_out.clone(),
                seed: 34,
                catalog_path: "templates/catalog.json".into(),
                dry_run: false,
            }),
            Err(ComposeError::PixelContract(template)) if template == "classic/ct"
        ));
        assert!(!wrong_out.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn p3_3_defaults_are_byte_stable_across_two_runs() {
        let first = output("p3-3-repro-a");
        let second = output("p3-3-repro-b");
        for out in [&first, &second] {
            compose(&ComposeOptions {
                spec_path: "tests/fixtures/composition/valid/classic-p3-3-defaults.json".into(),
                out_dir: out.clone(),
                seed: 35,
                catalog_path: "templates/catalog.json".into(),
                dry_run: false,
            })
            .unwrap();
        }
        for name in ["cr", "ct", "mr"] {
            assert_eq!(
                fs::read(first.join(format!("instances/{name}.dcm"))).unwrap(),
                fs::read(second.join(format!("instances/{name}.dcm"))).unwrap()
            );
        }
        fs::remove_dir_all(first).unwrap();
        fs::remove_dir_all(second).unwrap();
    }

    #[test]
    fn p3_4_defaults_materialize_and_are_byte_stable() {
        let first = output("p3-4-repro-a");
        let second = output("p3-4-repro-b");
        for out in [&first, &second] {
            let (summary, _) = compose(&ComposeOptions {
                spec_path: "tests/fixtures/composition/valid/classic-p3-4-defaults.json".into(),
                out_dir: out.clone(),
                seed: 36,
                catalog_path: "templates/catalog.json".into(),
                dry_run: false,
            })
            .unwrap();
            assert_eq!(summary.instances_written, 3);
        }
        for name in ["dx", "mg_presentation", "mg_processing"] {
            assert_eq!(
                fs::read(first.join(format!("instances/{name}.dcm"))).unwrap(),
                fs::read(second.join(format!("instances/{name}.dcm"))).unwrap()
            );
        }
        fs::remove_dir_all(first).unwrap();
        fs::remove_dir_all(second).unwrap();
    }

    #[test]
    fn p3_4_caller_pixels_round_trip_with_family_photometric_contracts() {
        let root = output("p3-4-caller");
        fs::create_dir(&root).unwrap();
        let raw = (0_u16..256).flat_map(u16::to_le_bytes).collect::<Vec<_>>();
        for name in ["dx", "mg-presentation", "mg-processing"] {
            fs::write(root.join(format!("{name}.raw")), &raw).unwrap();
        }
        let assignment = |name: &str, photometric: &str| {
            json!([{
                "slot": "pixels",
                "source": {
                    "kind": "local_file", "path": format!("{name}.raw"),
                    "sha256": sha256_hex(&raw),
                    "pixel": {
                        "rows": 16, "columns": 16, "frames": 1,
                        "samples_per_pixel": 1,
                        "photometric_interpretation": photometric,
                        "sample_type": "uint", "bits_allocated": 16,
                        "bits_stored": 12, "high_bit": 11, "byte_order": "little"
                    }
                }
            }])
        };
        let spec = json!({
            "composition_spec_schema_version": "0.1.0",
            "instances": [
                { "instance_id": "dx", "template": { "id": "classic/dx/for-presentation" }, "content": assignment("dx", "MONOCHROME2") },
                { "instance_id": "mg_presentation", "template": { "id": "classic/mammography/for-presentation" }, "content": assignment("mg-presentation", "MONOCHROME1") },
                { "instance_id": "mg_processing", "template": { "id": "classic/mammography/for-processing" }, "content": assignment("mg-processing", "MONOCHROME2") }
            ]
        });
        let spec_path = root.join("spec.json");
        fs::write(&spec_path, serde_json::to_vec_pretty(&spec).unwrap()).unwrap();
        let out = root.join("out");
        compose(&ComposeOptions {
            spec_path: spec_path.clone(),
            out_dir: out.clone(),
            seed: 37,
            catalog_path: "templates/catalog.json".into(),
            dry_run: false,
        })
        .unwrap();
        for name in ["dx", "mg_presentation", "mg_processing"] {
            let object =
                dicom_object::open_file(out.join(format!("instances/{name}.dcm"))).unwrap();
            assert_eq!(
                object
                    .element_by_name("PixelData")
                    .unwrap()
                    .to_bytes()
                    .unwrap()
                    .as_ref(),
                raw.as_slice()
            );
        }

        let mut wrong = spec;
        wrong["instances"][1]["content"] = assignment("mg-presentation", "MONOCHROME2");
        let wrong_spec = root.join("wrong.json");
        fs::write(&wrong_spec, serde_json::to_vec_pretty(&wrong).unwrap()).unwrap();
        let wrong_out = root.join("wrong-out");
        assert!(matches!(
            compose(&ComposeOptions {
                spec_path: wrong_spec, out_dir: wrong_out.clone(), seed: 37,
                catalog_path: "templates/catalog.json".into(), dry_run: false,
            }),
            Err(ComposeError::PixelContract(template))
                if template == "classic/mammography/for-presentation"
        ));
        assert!(!wrong_out.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn p3_5_defaults_materialize_and_are_byte_stable() {
        let first = output("p3-5-repro-a");
        let second = output("p3-5-repro-b");
        for out in [&first, &second] {
            let (summary, _) = compose(&ComposeOptions {
                spec_path: "tests/fixtures/composition/valid/classic-p3-5-defaults.json".into(),
                out_dir: out.clone(),
                seed: 38,
                catalog_path: "templates/catalog.json".into(),
                dry_run: false,
            })
            .unwrap();
            assert_eq!(summary.instances_written, 4);
        }
        for name in ["us_single", "us_multi", "nm", "pet"] {
            assert_eq!(
                fs::read(first.join(format!("instances/{name}.dcm"))).unwrap(),
                fs::read(second.join(format!("instances/{name}.dcm"))).unwrap()
            );
        }
        fs::remove_dir_all(first).unwrap();
        fs::remove_dir_all(second).unwrap();
    }

    #[test]
    fn p3_5_caller_pixels_round_trip_with_derived_multiframe_vectors() {
        let root = output("p3-5-caller");
        fs::create_dir(&root).unwrap();
        let us_single = vec![17_u8; 16 * 16];
        let us_multi = vec![23_u8; 16 * 16 * 3];
        let nm = vec![31_u8; 16 * 16 * 3 * 2];
        let pet = vec![47_u8; 16 * 16 * 2];
        for (name, bytes) in [
            ("us-single", &us_single),
            ("us-multi", &us_multi),
            ("nm", &nm),
            ("pet", &pet),
        ] {
            fs::write(root.join(format!("{name}.raw")), bytes).unwrap();
        }
        let assignment = |name: &str, bytes: &[u8], frames: u32, bits: u8| {
            json!([{
                "slot": "pixels",
                "source": {
                    "kind": "local_file", "path": format!("{name}.raw"),
                    "sha256": sha256_hex(bytes),
                    "pixel": {
                        "rows": 16, "columns": 16, "frames": frames,
                        "samples_per_pixel": 1, "photometric_interpretation": "MONOCHROME2",
                        "sample_type": "uint", "bits_allocated": bits,
                        "bits_stored": bits, "high_bit": bits - 1, "byte_order": "little"
                    }
                }
            }])
        };
        let spec = json!({
            "composition_spec_schema_version": "0.1.0",
            "instances": [
                { "instance_id": "us_single", "template": { "id": "classic/ultrasound/single-frame" }, "content": assignment("us-single", &us_single, 1, 8) },
                { "instance_id": "us_multi", "template": { "id": "classic/ultrasound/multiframe" }, "content": assignment("us-multi", &us_multi, 3, 8) },
                { "instance_id": "nm", "template": { "id": "classic/nuclear-medicine" }, "content": assignment("nm", &nm, 3, 16) },
                { "instance_id": "pet", "template": { "id": "classic/pet" }, "content": assignment("pet", &pet, 1, 16) }
            ]
        });
        let spec_path = root.join("spec.json");
        fs::write(&spec_path, serde_json::to_vec_pretty(&spec).unwrap()).unwrap();
        let out = root.join("out");
        compose(&ComposeOptions {
            spec_path: spec_path.clone(),
            out_dir: out.clone(),
            seed: 39,
            catalog_path: "templates/catalog.json".into(),
            dry_run: false,
        })
        .unwrap();
        for (name, expected) in [
            ("us_single", us_single.as_slice()),
            ("us_multi", us_multi.as_slice()),
            ("nm", nm.as_slice()),
            ("pet", pet.as_slice()),
        ] {
            let object =
                dicom_object::open_file(out.join(format!("instances/{name}.dcm"))).unwrap();
            assert_eq!(
                object
                    .element_by_name("PixelData")
                    .unwrap()
                    .to_bytes()
                    .unwrap()
                    .as_ref(),
                expected
            );
        }
        let nm_object = dicom_object::open_file(out.join("instances/nm.dcm")).unwrap();
        assert_eq!(
            nm_object
                .element_by_name("EnergyWindowVector")
                .unwrap()
                .to_multi_int::<u16>()
                .unwrap()
                .len(),
            3
        );

        let mut wrong = spec;
        wrong["instances"][0]["content"] = assignment("us-single", &us_single, 2, 8);
        let wrong_spec = root.join("wrong.json");
        fs::write(&wrong_spec, serde_json::to_vec_pretty(&wrong).unwrap()).unwrap();
        let wrong_out = root.join("wrong-out");
        assert!(matches!(
            compose(&ComposeOptions {
                spec_path: wrong_spec,
                out_dir: wrong_out.clone(),
                seed: 39,
                catalog_path: "templates/catalog.json".into(),
                dry_run: false,
            }),
            Err(ComposeError::RawContent(_)) | Err(ComposeError::PixelContract(_))
        ));
        assert!(!wrong_out.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn p3_6_defaults_materialize_and_are_byte_stable() {
        let first = output("p3-6-repro-a");
        let second = output("p3-6-repro-b");
        for out in [&first, &second] {
            let (summary, _) = compose(&ComposeOptions {
                spec_path: "tests/fixtures/composition/valid/classic-p3-6-defaults.json".into(),
                out_dir: out.clone(),
                seed: 40,
                catalog_path: "templates/catalog.json".into(),
                dry_run: false,
            })
            .unwrap();
            assert_eq!(summary.instances_written, 3);
        }
        for name in ["endoscopic", "microscopic", "photographic"] {
            assert_eq!(
                fs::read(first.join(format!("instances/{name}.dcm"))).unwrap(),
                fs::read(second.join(format!("instances/{name}.dcm"))).unwrap()
            );
        }
        fs::remove_dir_all(first).unwrap();
        fs::remove_dir_all(second).unwrap();
    }

    #[test]
    fn p3_6_caller_rgb_pixels_round_trip_and_planar_mismatch_is_rejected() {
        let root = output("p3-6-caller");
        fs::create_dir(&root).unwrap();
        let raw = (0..16 * 16 * 3)
            .map(|index| (index % 251) as u8)
            .collect::<Vec<_>>();
        fs::write(root.join("rgb.raw"), &raw).unwrap();
        let assignment = |planar: u8| {
            json!([{
                "slot": "pixels",
                "source": {
                    "kind": "local_file", "path": "rgb.raw", "sha256": sha256_hex(&raw),
                    "pixel": {
                        "rows": 16, "columns": 16, "frames": 1,
                        "samples_per_pixel": 3, "photometric_interpretation": "RGB",
                        "sample_type": "uint", "bits_allocated": 8, "bits_stored": 8,
                        "high_bit": 7, "byte_order": "little", "planar_configuration": planar
                    }
                }
            }])
        };
        let spec = json!({
            "composition_spec_schema_version": "0.1.0",
            "instances": [
                { "instance_id": "endoscopic", "template": { "id": "vl/endoscopic" }, "content": assignment(0) },
                { "instance_id": "microscopic", "template": { "id": "vl/microscopic" }, "content": assignment(0) },
                { "instance_id": "photographic", "template": { "id": "vl/photographic" }, "content": assignment(0) }
            ]
        });
        let spec_path = root.join("spec.json");
        fs::write(&spec_path, serde_json::to_vec_pretty(&spec).unwrap()).unwrap();
        let out = root.join("out");
        compose(&ComposeOptions {
            spec_path: spec_path.clone(),
            out_dir: out.clone(),
            seed: 41,
            catalog_path: "templates/catalog.json".into(),
            dry_run: false,
        })
        .unwrap();
        for name in ["endoscopic", "microscopic", "photographic"] {
            let object =
                dicom_object::open_file(out.join(format!("instances/{name}.dcm"))).unwrap();
            assert_eq!(
                object
                    .element_by_name("PixelData")
                    .unwrap()
                    .to_bytes()
                    .unwrap()
                    .as_ref(),
                raw.as_slice()
            );
        }
        let mut wrong = spec;
        wrong["instances"][0]["content"] = assignment(1);
        let wrong_spec = root.join("wrong.json");
        fs::write(&wrong_spec, serde_json::to_vec_pretty(&wrong).unwrap()).unwrap();
        let wrong_out = root.join("wrong-out");
        assert!(matches!(compose(&ComposeOptions {
            spec_path: wrong_spec, out_dir: wrong_out.clone(), seed: 41,
            catalog_path: "templates/catalog.json".into(), dry_run: false,
        }), Err(ComposeError::PixelContract(template)) if template == "vl/endoscopic"));
        assert!(!wrong_out.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn p3_7_native_defaults_materialize_and_are_byte_stable() {
        let first = output("p3-7-repro-a");
        let second = output("p3-7-repro-b");
        for out in [&first, &second] {
            let (summary, _) = compose(&ComposeOptions {
                spec_path: "tests/fixtures/composition/valid/classic-p3-7-defaults.json".into(),
                out_dir: out.clone(),
                seed: 42,
                catalog_path: "templates/catalog.json".into(),
                dry_run: false,
            })
            .unwrap();
            assert_eq!(summary.instances_written, 2);
        }
        for name in ["xa", "xrf"] {
            assert_eq!(
                fs::read(first.join(format!("instances/{name}.dcm"))).unwrap(),
                fs::read(second.join(format!("instances/{name}.dcm"))).unwrap()
            );
        }
        fs::remove_dir_all(first).unwrap();
        fs::remove_dir_all(second).unwrap();
    }

    #[test]
    fn p3_7_caller_native_pixels_round_trip_and_signed_input_is_rejected() {
        let root = output("p3-7-caller");
        fs::create_dir(&root).unwrap();
        let raw = (0_u16..256).flat_map(u16::to_le_bytes).collect::<Vec<_>>();
        fs::write(root.join("xray.raw"), &raw).unwrap();
        let assignment = |sample_type: &str| {
            json!([{
                "slot": "pixels",
                "source": {
                    "kind": "local_file", "path": "xray.raw", "sha256": sha256_hex(&raw),
                    "pixel": {
                        "rows": 16, "columns": 16, "frames": 1,
                        "samples_per_pixel": 1, "photometric_interpretation": "MONOCHROME2",
                        "sample_type": sample_type, "bits_allocated": 16,
                        "bits_stored": 12, "high_bit": 11, "byte_order": "little"
                    }
                }
            }])
        };
        let spec = json!({
            "composition_spec_schema_version": "0.1.0",
            "instances": [
                { "instance_id": "xa", "template": { "id": "classic/xa" }, "content": assignment("uint") },
                { "instance_id": "xrf", "template": { "id": "classic/xrf" }, "content": assignment("uint") }
            ]
        });
        let spec_path = root.join("spec.json");
        fs::write(&spec_path, serde_json::to_vec_pretty(&spec).unwrap()).unwrap();
        let out = root.join("out");
        compose(&ComposeOptions {
            spec_path: spec_path.clone(),
            out_dir: out.clone(),
            seed: 43,
            catalog_path: "templates/catalog.json".into(),
            dry_run: false,
        })
        .unwrap();
        for name in ["xa", "xrf"] {
            let object =
                dicom_object::open_file(out.join(format!("instances/{name}.dcm"))).unwrap();
            assert_eq!(
                object
                    .element_by_name("PixelData")
                    .unwrap()
                    .to_bytes()
                    .unwrap()
                    .as_ref(),
                raw.as_slice()
            );
        }
        let mut wrong = spec;
        wrong["instances"][0]["content"] = assignment("int");
        let wrong_spec = root.join("wrong.json");
        fs::write(&wrong_spec, serde_json::to_vec_pretty(&wrong).unwrap()).unwrap();
        let wrong_out = root.join("wrong-out");
        assert!(matches!(compose(&ComposeOptions {
            spec_path: wrong_spec, out_dir: wrong_out.clone(), seed: 43,
            catalog_path: "templates/catalog.json".into(), dry_run: false,
        }), Err(ComposeError::PixelContract(template)) if template == "classic/xa"));
        assert!(!wrong_out.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn multiframe_sc_defaults_materialize_and_are_byte_stable() {
        let first = output("mf-sc-repro-a");
        let second = output("mf-sc-repro-b");
        for out in [&first, &second] {
            let (summary, _) = compose(&ComposeOptions {
                spec_path: "tests/fixtures/composition/valid/classic-multiframe-sc-defaults.json"
                    .into(),
                out_dir: out.clone(),
                seed: 44,
                catalog_path: "templates/catalog.json".into(),
                dry_run: false,
            })
            .unwrap();
            assert_eq!(summary.instances_written, 2);
        }
        for name in ["single_bit", "grayscale_byte"] {
            assert_eq!(
                fs::read(first.join(format!("instances/{name}.dcm"))).unwrap(),
                fs::read(second.join(format!("instances/{name}.dcm"))).unwrap()
            );
        }
        fs::remove_dir_all(first).unwrap();
        fs::remove_dir_all(second).unwrap();
    }

    #[test]
    fn multiframe_sc_caller_pixels_round_trip_contiguous_bit_packing() {
        let root = output("mf-sc-caller");
        fs::create_dir(&root).unwrap();
        let bit1 = vec![0xA5_u8; 16 * 16 * 3 / 8];
        let grayscale = vec![71_u8; 16 * 16 * 3];
        fs::write(root.join("bit1.raw"), &bit1).unwrap();
        fs::write(root.join("grayscale.raw"), &grayscale).unwrap();
        let assignment = |path: &str, bytes: &[u8], sample: &str, bits: u8| {
            json!([{
                "slot": "pixels",
                "source": {
                    "kind": "local_file", "path": path, "sha256": sha256_hex(bytes),
                    "pixel": {
                        "rows": 16, "columns": 16, "frames": 3,
                        "samples_per_pixel": 1, "photometric_interpretation": "MONOCHROME2",
                        "sample_type": sample, "bits_allocated": bits,
                        "bits_stored": bits, "high_bit": bits - 1, "byte_order": "little"
                    }
                }
            }])
        };
        let spec = json!({
            "composition_spec_schema_version": "0.1.0",
            "instances": [
                { "instance_id": "single_bit", "template": { "id": "classic/secondary-capture/multiframe-single-bit" }, "content": assignment("bit1.raw", &bit1, "bit1", 1) },
                { "instance_id": "grayscale_byte", "template": { "id": "classic/secondary-capture/multiframe-grayscale-byte" }, "content": assignment("grayscale.raw", &grayscale, "uint", 8) }
            ]
        });
        let spec_path = root.join("spec.json");
        fs::write(&spec_path, serde_json::to_vec_pretty(&spec).unwrap()).unwrap();
        let out = root.join("out");
        compose(&ComposeOptions {
            spec_path: spec_path.clone(),
            out_dir: out.clone(),
            seed: 45,
            catalog_path: "templates/catalog.json".into(),
            dry_run: false,
        })
        .unwrap();
        for (name, expected) in [
            ("single_bit", bit1.as_slice()),
            ("grayscale_byte", grayscale.as_slice()),
        ] {
            let object =
                dicom_object::open_file(out.join(format!("instances/{name}.dcm"))).unwrap();
            assert_eq!(
                object
                    .element_by_name("PixelData")
                    .unwrap()
                    .to_bytes()
                    .unwrap()
                    .as_ref(),
                expected
            );
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn p3_7_rle_encodes_caller_frames_losslessly_and_byte_stably() {
        let root = output("p3-7-rle-caller");
        fs::create_dir(&root).unwrap();
        let raw = (0_u16..256).flat_map(u16::to_le_bytes).collect::<Vec<_>>();
        fs::write(root.join("xray.raw"), &raw).unwrap();
        let spec = json!({
            "composition_spec_schema_version": "0.1.0",
            "instances": [{
                "instance_id": "xa_rle",
                "template": { "id": "classic/xa" },
                "transfer_syntax_uid": "1.2.840.10008.1.2.5",
                "content": [{
                    "slot": "pixels",
                    "source": {
                        "kind": "local_file", "path": "xray.raw", "sha256": sha256_hex(&raw),
                        "pixel": {
                            "rows": 16, "columns": 16, "frames": 1,
                            "samples_per_pixel": 1, "photometric_interpretation": "MONOCHROME2",
                            "sample_type": "uint", "bits_allocated": 16,
                            "bits_stored": 12, "high_bit": 11, "byte_order": "little"
                        }
                    }
                }]
            }]
        });
        let spec_path = root.join("spec.json");
        fs::write(&spec_path, serde_json::to_vec_pretty(&spec).unwrap()).unwrap();
        let first = root.join("first");
        let second = root.join("second");
        for out in [&first, &second] {
            compose(&ComposeOptions {
                spec_path: spec_path.clone(),
                out_dir: out.clone(),
                seed: 46,
                catalog_path: "templates/catalog.json".into(),
                dry_run: false,
            })
            .unwrap();
        }
        let first_bytes = fs::read(first.join("instances/xa_rle.dcm")).unwrap();
        assert_eq!(
            first_bytes,
            fs::read(second.join("instances/xa_rle.dcm")).unwrap()
        );
        let manifest: serde_json::Value =
            serde_json::from_slice(&fs::read(first.join("manifest.json")).unwrap()).unwrap();
        assert_eq!(
            manifest["composition"]["entries"][0]["content"][0]["kind"],
            "encapsulated_pixels"
        );
        assert_eq!(
            manifest["composition"]["entries"][0]["content"][0]["properties"]["codec_backend"],
            "native_project_rle_encoder"
        );
        let object = dicom_object::open_file(first.join("instances/xa_rle.dcm")).unwrap();
        let fragments = match object.element_by_name("PixelData").unwrap().value() {
            dicom_core::value::Value::PixelSequence(sequence) => sequence.fragments(),
            _ => panic!("RLE Pixel Data must reopen as fragments"),
        };
        assert_eq!(fragments.len(), 1);
        let decoded = NativeRleLosslessEncoder::new()
            .decode_frame(FrameDecodeInput {
                encoded_frame: &fragments[0],
                rows: 16,
                columns: 16,
                samples_per_pixel: 1,
                bits_allocated: 16,
                bits_stored: 12,
                photometric_interpretation: "MONOCHROME2",
            })
            .unwrap();
        assert_eq!(decoded.native_bytes, raw);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn encoded_rle_frames_are_hash_checked_decoded_and_preserved() {
        let root = output("encoded-rle-caller");
        fs::create_dir(&root).unwrap();
        let raw = (0_u16..256).flat_map(u16::to_le_bytes).collect::<Vec<_>>();
        let encoded = NativeRleLosslessEncoder::new()
            .encode_frame(FrameEncodeInput {
                native_frame: &raw,
                rows: 16,
                columns: 16,
                samples_per_pixel: 1,
                bits_allocated: 16,
                bits_stored: 12,
                photometric_interpretation: "MONOCHROME2",
            })
            .unwrap()
            .bytes;
        fs::write(root.join("frame-1.rle"), &encoded).unwrap();
        let spec = json!({
            "composition_spec_schema_version": "0.1.0",
            "instances": [{
                "instance_id": "xa_encoded",
                "template": { "id": "classic/xa" },
                "transfer_syntax_uid": "1.2.840.10008.1.2.5",
                "content": [{
                    "slot": "pixels",
                    "source": {
                        "kind": "encoded_frames",
                        "transfer_syntax_uid": "1.2.840.10008.1.2.5",
                        "frames": [{"path": "frame-1.rle", "sha256": sha256_hex(&encoded)}],
                        "pixel": {
                            "rows": 16, "columns": 16, "frames": 1,
                            "samples_per_pixel": 1, "photometric_interpretation": "MONOCHROME2",
                            "sample_type": "uint", "bits_allocated": 16,
                            "bits_stored": 12, "high_bit": 11, "byte_order": "little"
                        }
                    }
                }]
            }]
        });
        let spec_path = root.join("spec.json");
        fs::write(&spec_path, serde_json::to_vec_pretty(&spec).unwrap()).unwrap();
        let out = root.join("out");
        let (_, manifest) = compose(&ComposeOptions {
            spec_path,
            out_dir: out.clone(),
            seed: 81,
            catalog_path: "templates/catalog.json".into(),
            dry_run: false,
        })
        .unwrap();
        let properties = &manifest["composition"]["entries"][0]["content"][0]["properties"];
        assert_eq!(properties["content_origin"], "encoded_frames");
        assert_eq!(
            properties["codec_semantic_validation"],
            "independent_decode_passed"
        );
        assert_eq!(
            manifest["composition"]["assets"][0]["spec_relative_path"],
            "frame-1.rle"
        );
        assert_eq!(
            manifest["composition"]["assets"][0]["sha256"],
            sha256_hex(&encoded)
        );
        let object = dicom_object::open_file(out.join("instances/xa_encoded.dcm")).unwrap();
        let fragments = match object.element_by_name("PixelData").unwrap().value() {
            dicom_core::value::Value::PixelSequence(sequence) => sequence.fragments(),
            _ => panic!("encoded caller input must remain encapsulated"),
        };
        assert_eq!(fragments.len(), 1);
        assert_eq!(
            NativeRleLosslessEncoder::new()
                .decode_frame(FrameDecodeInput {
                    encoded_frame: &fragments[0],
                    rows: 16,
                    columns: 16,
                    samples_per_pixel: 1,
                    bits_allocated: 16,
                    bits_stored: 12,
                    photometric_interpretation: "MONOCHROME2",
                })
                .unwrap()
                .native_bytes,
            raw
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn inline_small_fixture_is_hash_checked_and_resource_accounted() {
        let root = output("inline-small-fixture");
        fs::create_dir(&root).unwrap();
        let raw = [0_u8; 8];
        let spec = json!({
            "composition_spec_schema_version": "0.1.0",
            "instances": [{
                "instance_id": "xa_inline",
                "template": { "id": "classic/xa" },
                "content": [{
                    "slot": "pixels",
                    "source": {
                        "kind": "inline_small_fixture",
                        "base64": "AAAAAAAAAAA=",
                        "sha256": sha256_hex(&raw),
                        "pixel": {
                            "rows": 2, "columns": 2, "frames": 1,
                            "samples_per_pixel": 1, "photometric_interpretation": "MONOCHROME2",
                            "sample_type": "uint", "bits_allocated": 16,
                            "bits_stored": 12, "high_bit": 11, "byte_order": "little"
                        }
                    }
                }]
            }]
        });
        let spec_path = root.join("spec.json");
        fs::write(&spec_path, serde_json::to_vec_pretty(&spec).unwrap()).unwrap();
        let (_, manifest) = compose(&ComposeOptions {
            spec_path,
            out_dir: root.join("out"),
            seed: 82,
            catalog_path: "templates/catalog.json".into(),
            dry_run: false,
        })
        .unwrap();
        assert_eq!(
            manifest["composition"]["entries"][0]["content"][0]["properties"]["content_origin"],
            "inline_fixture"
        );
        assert_eq!(
            manifest["composition"]["assets"][0]["staging_method"],
            "inline"
        );
        fs::remove_dir_all(root).unwrap();
    }
}
