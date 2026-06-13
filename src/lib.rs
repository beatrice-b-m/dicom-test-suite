pub const PACKAGE_NAME: &str = env!("CARGO_PKG_NAME");
pub const PACKAGE_VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn version_banner() -> String {
    format!("{PACKAGE_NAME} {PACKAGE_VERSION}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use dicom_dictionary_std::uids;
    use dicom_object::InMemDicomObject;
    use dicom_transfer_syntax_registry::{TransferSyntaxIndex, TransferSyntaxRegistry};

    #[test]
    fn version_banner_uses_package_metadata() {
        assert_eq!(version_banner(), "dicom-test-suite 0.1.0");
    }

    #[test]
    fn pinned_dicom_rs_crates_expose_phase_one_primitives() {
        let _obj = InMemDicomObject::new_empty();

        let explicit_vr_le = TransferSyntaxRegistry
            .get(uids::EXPLICIT_VR_LITTLE_ENDIAN)
            .expect("Explicit VR Little Endian must be available for Part 10 smoke cases");

        assert_eq!(explicit_vr_le.uid(), uids::EXPLICIT_VR_LITTLE_ENDIAN);
        assert_eq!(uids::VERIFICATION, "1.2.840.10008.1.1");
    }
}
