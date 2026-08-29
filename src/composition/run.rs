use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::{Value, json};

use super::{
    CompositionManifestAssembler, CompositionManifestInputs, CompositionSpec, CompositionUidRole,
    ContentLimits, ContentSource, DefaultPixelOutput, IdentityAllocator, IdentityChoice,
    LocalContentResolver, ManifestEntryInput, Part10Materializer, ResolvedInstancePlan,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComposeSummary {
    pub out_dir: PathBuf,
    pub manifest_path: PathBuf,
    pub instances_written: usize,
    pub output_bytes: u64,
    pub dry_run: bool,
}

pub fn compose(options: &ComposeOptions) -> Result<(ComposeSummary, Value), ComposeError> {
    if options.out_dir.exists() {
        return Err(ComposeError::OutputExists(options.out_dir.clone()));
    }
    let spec_bytes = fs::read(&options.spec_path).map_err(|source| ComposeError::Io {
        path: options.spec_path.clone(),
        source,
    })?;
    let catalog_bytes = fs::read(&options.catalog_path).map_err(|source| ComposeError::Io {
        path: options.catalog_path.clone(),
        source,
    })?;
    let spec = CompositionSpec::from_slice(&spec_bytes)?;
    let catalog = TemplateCatalog::from_slice(&catalog_bytes)?;
    let parent = options
        .out_dir
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
    let result = resolve_and_stage(
        options,
        &spec,
        &catalog,
        &spec_bytes,
        &catalog_bytes,
        &staging,
    );
    match result {
        Ok((summary, output)) if options.dry_run => {
            remove_private_staging(&staging)?;
            Ok((summary, output))
        }
        Ok((mut summary, output)) => {
            fs::rename(&staging, &options.out_dir).map_err(|source| ComposeError::Io {
                path: options.out_dir.clone(),
                source,
            })?;
            summary.out_dir = options.out_dir.clone();
            summary.manifest_path = options.out_dir.join("manifest.json");
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
) -> Result<(ComposeSummary, Value), ComposeError> {
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
    let spec_root = options.spec_path.parent().unwrap_or_else(|| Path::new("."));
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
        let p2_sc = matches!(
            template.template_id.0.as_str(),
            "classic/secondary-capture/monochrome" | "classic/secondary-capture/rgb"
        );
        let family = super::ClassicFamilyProfile::for_template(&template.template_id);
        if !p2_sc && family.is_none() {
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
        if family.is_some_and(|profile| profile.include_geometry) {
            roles.push((CompositionUidRole::FrameOfReference, 0));
        }
        let mut identities = allocator.allocate_plan(instance.instance_id.clone(), roles)?;
        apply_explicit_identities(instance, &mut identities.identities)?;
        identity_plans.insert(instance.instance_id.clone(), identities);
        templates.push(template);
    }
    apply_shared_identities(spec, &mut identity_plans)?;

    for (instance, template) in spec.instances.iter().zip(templates) {
        if !instance.references.is_empty() {
            return Err(ComposeError::UnsupportedScReference(
                instance.instance_id.clone(),
            ));
        }
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
        if let Some(profile) = super::ClassicFamilyProfile::for_template(&template.template_id) {
            let pixel = resolve_family_pixels(instance, &profile, &mut content_resolver)?;
            validate_family_pixel_contract(&profile, &pixel)?;
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

    let mut entry_paths = Vec::with_capacity(plans.len());
    let mut output_bytes = 0_u64;
    for plan in &plans {
        let relative_path = format!("instances/{}.dcm", plan.instance_id);
        let path = staging.join(&relative_path);
        Part10Materializer.materialize(plan, &path)?;
        output_bytes = output_bytes
            .checked_add(
                fs::metadata(&path)
                    .map_err(|source| ComposeError::Io {
                        path: path.clone(),
                        source,
                    })?
                    .len(),
            )
            .ok_or(ComposeError::OutputSizeOverflow)?;
        if output_bytes > spec.resource_limits.max_total_output_bytes {
            return Err(ComposeError::OutputLimit {
                size: output_bytes,
                limit: spec.resource_limits.max_total_output_bytes,
            });
        }
        entry_paths.push((path, relative_path));
    }
    let entries = plans
        .iter()
        .zip(&entry_paths)
        .map(|(plan, (path, relative_path))| ManifestEntryInput {
            plan,
            output_path: path,
            relative_path: relative_path.clone(),
            requested: true,
            bundle_root_instance_id: plan.instance_id.clone(),
            bundle_role: "root".into(),
            determinism: "byte_stable".into(),
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
            requested_parallelism: 1,
            used_parallelism: 1,
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
    ResourceRange,
    UnsupportedTemplate(String),
    UnsupportedP2Content(String),
    UnsupportedFamilyContent(String),
    UnsupportedScReference(String),
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
    OutputSizeOverflow,
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
}
