use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::{Value, json};

#[cfg(test)]
use crate::codecs::{FrameDecodeInput, FrameDecoder};
use crate::codecs::{FrameEncodeInput, FrameEncoder, NativeRleLosslessEncoder};
use crate::encapsulation::{BasicOffsetTablePolicy, EncapsulatedPixelData};

use super::{
    AdvancedFamilyProfile, BundleResolver, CompositionManifestAssembler, CompositionManifestInputs,
    CompositionSpec, CompositionUidRole, ContentLimits, ContentSource, CyclePolicy,
    DefaultPixelOutput, IdentityAllocator, IdentityChoice, LocalContentResolver, LogicalReference,
    ManifestEntryInput, Part10Materializer, ReferenceGraph, ReferenceNode, ResolvedInstancePlan,
    TemplateCatalog, TemplateDescriptor, default_family_pixels, resolve_family_attributes,
    resolve_raw_native_pixels, resolved_sc_plan, sc_default_pixels,
};
use crate::{PACKAGE_NAME, PACKAGE_VERSION, RUSTC_VERSION, TARGET_TRIPLE, sha256_hex};

static NEXT_STAGING: AtomicU64 = AtomicU64::new(0);

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

pub fn compose(options: &ComposeOptions) -> Result<(ComposeSummary, Value), ComposeError> {
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
    )
}

pub fn compose_from_bytes(
    spec_bytes: &[u8],
    options: &ComposeBytesOptions,
) -> Result<(ComposeSummary, Value), ComposeError> {
    compose_loaded(
        spec_bytes,
        &options.spec_root,
        &options.out_dir,
        options.seed,
        &options.catalog_path,
        options.dry_run,
    )
}

fn compose_loaded(
    spec_bytes: &[u8],
    spec_root: &Path,
    out_dir: &Path,
    seed: u64,
    catalog_path: &Path,
    dry_run: bool,
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
    fs::create_dir_all(parent).map_err(|source| ComposeError::Io {
        path: parent.to_path_buf(),
        source,
    })?;
    let staging = parent.join(format!(
        ".dts-compose-{}-{:08}",
        std::process::id(),
        NEXT_STAGING.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&staging).map_err(|source| ComposeError::Io {
        path: staging.clone(),
        source,
    })?;
    let options = ComposeOptions {
        spec_path: spec_root.join("<in-memory-spec>"),
        out_dir: out_dir.to_path_buf(),
        seed,
        catalog_path: catalog_path.to_path_buf(),
        dry_run,
    };
    let result = resolve_and_stage(
        &options,
        &spec,
        &catalog,
        &spec_bytes,
        &catalog_bytes,
        &staging,
        spec_root,
    );
    match result {
        Ok((summary, output)) if dry_run => {
            remove_private_staging(&staging)?;
            Ok((summary, output))
        }
        Ok((mut summary, output)) => {
            fs::rename(&staging, out_dir).map_err(|source| ComposeError::Io {
                path: out_dir.to_path_buf(),
                source,
            })?;
            summary.out_dir = out_dir.to_path_buf();
            summary.manifest_path = out_dir.join("manifest.json");
            Ok((summary, output))
        }
        Err(error) => {
            let _ = remove_private_staging(&staging);
            Err(error)
        }
    }
}

fn resolve_and_stage(
    options: &ComposeOptions,
    spec: &CompositionSpec,
    catalog: &TemplateCatalog,
    spec_bytes: &[u8],
    catalog_bytes: &[u8],
    staging: &Path,
    spec_root: &Path,
) -> Result<(ComposeSummary, Value), ComposeError> {
    let bundle_resolution = BundleResolver.resolve(spec.clone(), catalog)?;
    let spec = &bundle_resolution.spec;
    let output_root = staging.join("instances");
    let asset_root = staging.join(".assets");
    fs::create_dir(&output_root).map_err(|source| ComposeError::Io {
        path: output_root.clone(),
        source,
    })?;
    fs::create_dir(&asset_root).map_err(|source| ComposeError::Io {
        path: asset_root.clone(),
        source,
    })?;
    let mut content_resolver = LocalContentResolver::new(
        spec_root,
        &asset_root,
        ContentLimits {
            max_files: usize::try_from(spec.resource_limits.max_input_files)
                .map_err(|_| ComposeError::ResourceRange)?,
            max_file_bytes: spec.resource_limits.max_file_bytes,
            max_total_bytes: spec.resource_limits.max_total_input_bytes,
        },
    )?;
    let run_defaults = spec.defaults.typed_attributes()?;
    let mut plans = Vec::with_capacity(spec.instances.len());
    let mut templates = Vec::with_capacity(spec.instances.len());
    let mut identity_plans = BTreeMap::new();

    for instance in &spec.instances {
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
        let mut identities = allocator.allocate_plan(instance.instance_id.clone(), roles)?;
        apply_explicit_identities(instance, &mut identities.identities)?;
        identity_plans.insert(instance.instance_id.clone(), identities);
        templates.push(template);
    }
    apply_shared_identities(spec, &mut identity_plans)?;

    for (instance, template) in spec.instances.iter().zip(templates) {
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
        reject_content_element_override(&instance.instance_id, &overrides)?;
        let base_plan = ResolvedInstancePlan {
            plan_schema_version: "0.1.0".into(),
            instance_id: instance.instance_id.clone(),
            template_id: template.template_id.clone(),
            template_version: template.template_version,
            sop_class_uid: template.sop_class_uid.clone(),
            transfer_syntax_uid: transfer_syntax_uid.into(),
            identities: identity_plans
                .remove(&instance.instance_id)
                .expect("identity pass covered every instance"),
            attributes: vec![],
            content: vec![],
            references: vec![],
        };
        if let Some(profile) = AdvancedFamilyProfile::for_template(&template.template_id.0) {
            let private_root = staging.join(".defaults").join(&instance.instance_id);
            let plan = profile.resolve_plan(
                instance,
                template,
                base_plan.identities,
                options.seed,
                &private_root,
                &mut content_resolver,
            )?;
            plans.push(plan);
        } else if let Some(profile) =
            super::ClassicFamilyProfile::for_template(&template.template_id)
        {
            let mut pixel = resolve_family_pixels(instance, &profile, &mut content_resolver)?;
            validate_family_pixel_contract(&profile, &pixel)?;
            if transfer_syntax_uid == crate::codecs::RLE_LOSSLESS_TRANSFER_SYNTAX_UID {
                pixel = encode_rle_pixels(pixel)?;
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
            plans.push(plan);
        } else {
            let pixel = resolve_sc_pixels(instance, template, &mut content_resolver)?;
            validate_sc_pixel_contract(template, &pixel)?;
            plans.push(resolved_sc_plan(
                base_plan,
                template,
                &run_defaults,
                &overrides,
                pixel,
            )?);
        }
    }

    let private_defaults = staging.join(".defaults");
    if private_defaults.exists() {
        remove_private_staging(&private_defaults)?;
    }

    super::advanced_family::validate_concatenation_closure(&plans, &bundle_resolution.members)?;
    materialize_reference_graph(&mut plans, spec, &bundle_resolution.members)?;
    super::advanced_family::rewrite_materialized_dicom_references(&mut plans)?;

    let dry_run_output = json!({
        "composition_spec_schema_version": spec.composition_spec_schema_version,
        "seed": options.seed,
        "plans": plans
    });
    if options.dry_run {
        return Ok((
            ComposeSummary {
                out_dir: options.out_dir.clone(),
                manifest_path: options.out_dir.join("manifest.json"),
                instances_written: 0,
                output_bytes: 0,
                dry_run: true,
            },
            dry_run_output,
        ));
    }

    let used_parallelism = usize::try_from(spec.parallelism)
        .map_err(|_| ComposeError::ResourceRange)?
        .min(plans.len())
        .max(1);
    let entry_paths = materialize_plans(&mut plans, staging, used_parallelism)?;
    let output_bytes = entry_paths.iter().try_fold(0_u64, |total, (path, _)| {
        let size = fs::metadata(path)
            .map_err(|source| ComposeError::Io {
                path: path.clone(),
                source,
            })?
            .len();
        total.checked_add(size).ok_or(ComposeError::OutputSizeOverflow)
    })?;
    if output_bytes > spec.resource_limits.max_total_output_bytes {
        return Err(ComposeError::OutputLimit {
            size: output_bytes,
            limit: spec.resource_limits.max_total_output_bytes,
        });
    }
    let entries = plans
        .iter()
        .zip(&entry_paths)
        .map(|(plan, (path, relative_path))| {
            let member = bundle_resolution.member(&plan.instance_id);
            ManifestEntryInput {
                plan,
                output_path: path,
                relative_path: relative_path.clone(),
                requested: member.requested,
                bundle_root_instance_id: member.bundle_root_instance_id.clone(),
                bundle_role: member.bundle_role.clone(),
                source_provenance: member.source.clone(),
                determinism: "byte_stable".into(),
            }
        })
        .collect::<Vec<_>>();
    let manifest = CompositionManifestAssembler.assemble(
        CompositionManifestInputs {
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
        &entries,
    )?;
    let manifest_path = staging.join("manifest.json");
    fs::write(
        &manifest_path,
        format!(
            "{}\n",
            serde_json::to_string_pretty(&manifest).expect("manifest serializes")
        ),
    )
    .map_err(|source| ComposeError::Io {
        path: manifest_path.clone(),
        source,
    })?;
    fs::remove_dir_all(&asset_root).map_err(|source| ComposeError::Io {
        path: asset_root,
        source,
    })?;
    Ok((
        ComposeSummary {
            out_dir: staging.to_path_buf(),
            manifest_path,
            instances_written: plans.len(),
            output_bytes,
            dry_run: false,
        },
        manifest,
    ))
}

fn materialize_plans(
    plans: &mut [ResolvedInstancePlan],
    staging: &Path,
    workers: usize,
) -> Result<Vec<(PathBuf, String)>, ComposeError> {
    let chunk_size = plans.len().div_ceil(workers);
    let chunks = std::thread::scope(|scope| {
        let mut handles = Vec::new();
        for chunk in plans.chunks_mut(chunk_size) {
            handles.push(scope.spawn(move || {
                let mut paths = Vec::with_capacity(chunk.len());
                for plan in chunk {
                    let relative_path = format!("instances/{}.dcm", plan.instance_id);
                    let path = staging.join(&relative_path);
                    let outcome = Part10Materializer.materialize_with_outcome(plan, &path)?;
                    for content in &mut plan.content {
                        if outcome.streamed_slots.contains(&content.slot) {
                            content
                                .properties
                                .insert("writer_materialization".into(), "stream_copy".into());
                        }
                    }
                    paths.push((path, relative_path));
                }
                Ok::<_, ComposeError>(paths)
            }));
        }
        handles
            .into_iter()
            .map(|handle| handle.join().map_err(|_| ComposeError::ParallelWorkerPanic)?)
            .collect::<Result<Vec<_>, ComposeError>>()
    })?;
    Ok(chunks.into_iter().flatten().collect())
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
        _ => Err(ComposeError::UnsupportedP2Content(
            instance.instance_id.clone(),
        )),
    }
}

fn resolve_family_pixels(
    instance: &super::SpecInstance,
    profile: &super::ClassicFamilyProfile,
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
        _ => Err(ComposeError::UnsupportedFamilyContent(
            instance.instance_id.clone(),
        )),
    }
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

fn encode_rle_pixels(mut pixel: DefaultPixelOutput) -> Result<DefaultPixelOutput, ComposeError> {
    let native = match pixel.content.materialization.as_ref() {
        Some(super::ContentMaterialization::Inline(bytes)) => bytes.clone(),
        Some(super::ContentMaterialization::StagedFile(path)) => {
            fs::read(path).map_err(|source| ComposeError::Io {
                path: path.clone(),
                source,
            })?
        }
        Some(super::ContentMaterialization::Encapsulated { .. }) => {
            return Err(ComposeError::AlreadyEncapsulated);
        }
        None => return Err(ComposeError::MissingPixelMaterialization),
    };
    let shape = &pixel.plan.shape;
    let photometric = match shape.photometric_interpretation {
        super::PhotometricInterpretation::Monochrome1 => "MONOCHROME1",
        super::PhotometricInterpretation::Monochrome2 => "MONOCHROME2",
        super::PhotometricInterpretation::PaletteColor => "PALETTE COLOR",
        super::PhotometricInterpretation::Rgb => "RGB",
        super::PhotometricInterpretation::YbrFull => "YBR_FULL",
        super::PhotometricInterpretation::YbrFull422 => "YBR_FULL_422",
    };
    let mut encoded_frames = Vec::with_capacity(pixel.plan.frame_spans.len());
    for frame in &pixel.plan.frame_spans {
        if frame.bit_offset % 8 != 0 || frame.bit_length % 8 != 0 {
            return Err(ComposeError::CodecPixelAlignment);
        }
        let start =
            usize::try_from(frame.bit_offset / 8).map_err(|_| ComposeError::ResourceRange)?;
        let length =
            usize::try_from(frame.bit_length / 8).map_err(|_| ComposeError::ResourceRange)?;
        let end = start
            .checked_add(length)
            .ok_or(ComposeError::ResourceRange)?;
        let encoded = NativeRleLosslessEncoder::new().encode_frame(FrameEncodeInput {
            native_frame: native.get(start..end).ok_or(ComposeError::ResourceRange)?,
            rows: u16::try_from(shape.rows).map_err(|_| ComposeError::ResourceRange)?,
            columns: u16::try_from(shape.columns).map_err(|_| ComposeError::ResourceRange)?,
            samples_per_pixel: u16::from(shape.samples_per_pixel),
            bits_allocated: u16::from(shape.bits_allocated),
            bits_stored: u16::from(shape.bits_stored),
            photometric_interpretation: photometric,
        })?;
        encoded_frames.push(encoded.bytes);
    }
    let encapsulated = EncapsulatedPixelData::one_fragment_per_frame(
        &encoded_frames,
        BasicOffsetTablePolicy::Populated,
    )?;
    let mut fragments = encapsulated.fragment_payloads;
    for fragment in &mut fragments {
        if fragment.len() % 2 != 0 {
            fragment.push(0);
        }
    }
    let bytes = fragments.concat();
    pixel.content.kind = "encapsulated_pixels".into();
    pixel.content.vr = super::DicomVr::OB;
    pixel.content.size_bytes = bytes.len() as u64;
    pixel.content.sha256 = sha256_hex(&bytes);
    pixel
        .content
        .properties
        .insert("native_sha256".into(), sha256_hex(&native));
    pixel.content.properties.insert(
        "codec_backend".into(),
        NativeRleLosslessEncoder::BACKEND_ID.into(),
    );
    pixel.content.properties.insert(
        "compressed_frame_sha256".into(),
        serde_json::to_string(&encapsulated.compressed_frame_hashes)
            .expect("frame hashes serialize"),
    );
    pixel.content.materialization = Some(super::ContentMaterialization::Encapsulated {
        basic_offset_table: encapsulated.basic_offset_table.offsets,
        fragments,
    });
    Ok(pixel)
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

fn reject_content_element_override(
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
    Ok(())
}

fn resource_map(limits: &super::ResourceLimits) -> BTreeMap<String, u64> {
    BTreeMap::from([
        ("max_instances".into(), limits.max_instances),
        ("max_input_files".into(), limits.max_input_files),
        ("max_file_bytes".into(), limits.max_file_bytes),
        ("max_total_input_bytes".into(), limits.max_total_input_bytes),
        (
            "max_total_output_bytes".into(),
            limits.max_total_output_bytes,
        ),
    ])
}

fn remove_private_staging(path: &Path) -> Result<(), ComposeError> {
    fs::remove_dir_all(path).map_err(|source| ComposeError::Io {
        path: path.to_path_buf(),
        source,
    })
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
    Defaults(super::DefaultError),
    Materialize(super::MaterializeError),
    Manifest(super::ManifestError),
    Family(super::FamilyError),
    AdvancedFamily(super::AdvancedFamilyError),
    Bundle(super::BundleError),
    Reference(super::ReferenceError),
    Codec(crate::codecs::CodecError),
    Encapsulation(crate::encapsulation::EncapsulationError),
    ResourceRange,
    ParallelWorkerPanic,
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
    ParameterSchema {
        instance_id: String,
        message: String,
    },
    OutputSizeOverflow,
    MissingPixelMaterialization,
    AlreadyEncapsulated,
    CodecPixelAlignment,
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
from_error!(super::DefaultError, Defaults);
from_error!(super::MaterializeError, Materialize);
from_error!(super::ManifestError, Manifest);
from_error!(super::FamilyError, Family);
from_error!(super::AdvancedFamilyError, AdvancedFamily);
from_error!(super::BundleError, Bundle);
from_error!(super::ReferenceError, Reference);
from_error!(crate::codecs::CodecError, Codec);
from_error!(crate::encapsulation::EncapsulationError, Encapsulation);

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
}
