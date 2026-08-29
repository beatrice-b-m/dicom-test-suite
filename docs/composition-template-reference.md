# Composition template reference

This file is rendered from `templates/catalog.json`. Use `templates describe` for the complete attribute policies, content constraints, requirements, evidence, and limitations of one template.

| Template | IOD | SOP Class UID | Transfer syntaxes | Determinism | Independent routes |
|---|---|---|---|---|---|
| `classic/cr`@1.0.0 | Computed Radiography Image | `1.2.840.10008.5.1.4.1.1.1` | `1.2.840.10008.1.2.1` (default) | byte_stable | dicom_validator |
| `classic/ct`@1.0.0 | CT Image | `1.2.840.10008.5.1.4.1.1.2` | `1.2.840.10008.1.2.1` (default) | byte_stable | dicom_validator |
| `classic/dx/for-presentation`@1.0.0 | Digital X-Ray Image | `1.2.840.10008.5.1.4.1.1.1.1` | `1.2.840.10008.1.2.1` (default) | byte_stable | dicom_validator |
| `classic/mammography/for-presentation`@1.0.0 | Digital Mammography X-Ray Image | `1.2.840.10008.5.1.4.1.1.1.2` | `1.2.840.10008.1.2.1` (default) | byte_stable | dicom_validator |
| `classic/mammography/for-processing`@1.0.0 | Digital Mammography X-Ray Image | `1.2.840.10008.5.1.4.1.1.1.2.1` | `1.2.840.10008.1.2` (default) | byte_stable | dicom_validator |
| `classic/mr`@1.0.0 | MR Image | `1.2.840.10008.5.1.4.1.1.4` | `1.2.840.10008.1.2.1` (default) | byte_stable | dicom_validator |
| `classic/nuclear-medicine`@1.0.0 | Nuclear Medicine Image | `1.2.840.10008.5.1.4.1.1.20` | `1.2.840.10008.1.2.1` (default) | byte_stable | dicom_validator |
| `classic/pet`@1.0.0 | PET Image | `1.2.840.10008.5.1.4.1.1.128` | `1.2.840.10008.1.2.1` (default) | byte_stable | dicom_validator |
| `classic/secondary-capture/monochrome`@1.0.0 | Secondary Capture Image | `1.2.840.10008.5.1.4.1.1.7` | `1.2.840.10008.1.2.1` (default) | byte_stable | dicom_validator |
| `classic/secondary-capture/multiframe-grayscale-byte`@1.0.0 | Multi-frame Grayscale Byte Secondary Capture Image | `1.2.840.10008.5.1.4.1.1.7.2` | `1.2.840.10008.1.2.1` (default) | byte_stable | dicom_validator |
| `classic/secondary-capture/multiframe-single-bit`@1.0.0 | Multi-frame Single Bit Secondary Capture Image | `1.2.840.10008.5.1.4.1.1.7.1` | `1.2.840.10008.1.2.1` (default) | byte_stable | dicom_validator |
| `classic/secondary-capture/rgb`@1.0.0 | Secondary Capture Image | `1.2.840.10008.5.1.4.1.1.7` | `1.2.840.10008.1.2.1` (default) | byte_stable | dicom_validator |
| `classic/ultrasound/multiframe`@1.0.0 | Ultrasound Multi-frame Image | `1.2.840.10008.5.1.4.1.1.3.1` | `1.2.840.10008.1.2.1` (default) | byte_stable | dicom_validator |
| `classic/ultrasound/single-frame`@1.0.0 | Ultrasound Image | `1.2.840.10008.5.1.4.1.1.6.1` | `1.2.840.10008.1.2.1` (default) | byte_stable | dicom_validator |
| `classic/xa`@1.0.0 | X-Ray Angiographic Image | `1.2.840.10008.5.1.4.1.1.12.1` | `1.2.840.10008.1.2.1` (default)<br>`1.2.840.10008.1.2.5` | byte_stable | dicom_validator |
| `classic/xrf`@1.0.0 | X-Ray Radiofluoroscopic Image | `1.2.840.10008.5.1.4.1.1.12.2` | `1.2.840.10008.1.2.1` (default)<br>`1.2.840.10008.1.2.5` | byte_stable | dicom_validator |
| `derived/presentation-state/advanced-blending`@1.0.0 | Advanced Blending Presentation State | `1.2.840.10008.5.1.4.1.1.11.8` | `1.2.840.10008.1.2.1` (default) | byte_stable | dicom_validator |
| `derived/presentation-state/blending`@1.0.0 | Blending Softcopy Presentation State | `1.2.840.10008.5.1.4.1.1.11.4` | `1.2.840.10008.1.2.1` (default) | byte_stable | dicom_validator |
| `derived/presentation-state/color`@1.0.0 | Color Softcopy Presentation State | `1.2.840.10008.5.1.4.1.1.11.2` | `1.2.840.10008.1.2.1` (default) | byte_stable | dicom_validator |
| `derived/presentation-state/grayscale`@1.0.0 | Grayscale Softcopy Presentation State | `1.2.840.10008.5.1.4.1.1.11.1` | `1.2.840.10008.1.2.1` (default) | byte_stable | dicom_validator |
| `derived/registration/deformable`@1.0.0 | Deformable Spatial Registration | `1.2.840.10008.5.1.4.1.1.66.3` | `1.2.840.10008.1.2.1` (default) | byte_stable | dicom_validator |
| `derived/registration/spatial`@1.0.0 | Spatial Registration | `1.2.840.10008.5.1.4.1.1.66.1` | `1.2.840.10008.1.2.1` (default) | byte_stable | dicom_validator |
| `enhanced/ct`@1.0.0 | Enhanced CT Image | `1.2.840.10008.5.1.4.1.1.2.1` | `1.2.840.10008.1.2.1` (default) | byte_stable | dicom_validator |
| `enhanced/ct/concatenation-part-1`@1.0.0 | Enhanced CT Image | `1.2.840.10008.5.1.4.1.1.2.1` | `1.2.840.10008.1.2.1` (default) | byte_stable | dicom_validator |
| `enhanced/ct/concatenation-part-2`@1.0.0 | Enhanced CT Image | `1.2.840.10008.5.1.4.1.1.2.1` | `1.2.840.10008.1.2.1` (default) | byte_stable | dicom_validator |
| `enhanced/mr`@1.0.0 | Enhanced MR Image | `1.2.840.10008.5.1.4.1.1.4.1` | `1.2.840.10008.1.2.1` (default) | byte_stable | dicom_validator |
| `enhanced/pet`@1.0.0 | Enhanced PET Image | `1.2.840.10008.5.1.4.1.1.130` | `1.2.840.10008.1.2.1` (default) | byte_stable | dicom_validator |
| `non-image/encapsulated-document/pdf`@1.0.0 | Encapsulated PDF | `1.2.840.10008.5.1.4.1.1.104.1` | `1.2.840.10008.1.2.1` (default) | byte_stable | dicom_validator, pdfinfo |
| `non-image/mesh/stl`@1.0.0 | Encapsulated STL | `1.2.840.10008.5.1.4.1.1.104.3` | `1.2.840.10008.1.2.1` (default) | byte_stable | dicom_validator, pydicom-encapsulated-stl-payload |
| `non-image/waveform/general-ecg`@1.0.0 | General ECG Waveform | `1.2.840.10008.5.1.4.1.1.9.1.2` | `1.2.840.10008.1.2.1` (default) | byte_stable | dicom_validator, pydicom-dicom-validator-waveform |
| `non-image/waveform/twelve-lead-ecg`@1.0.0 | 12-lead ECG Waveform | `1.2.840.10008.5.1.4.1.1.9.1.1` | `1.2.840.10008.1.2.1` (default) | byte_stable | dicom_validator, pydicom-dicom-validator-waveform |
| `vl/endoscopic`@1.0.0 | VL Endoscopic Image | `1.2.840.10008.5.1.4.1.1.77.1.1` | `1.2.840.10008.1.2.1` (default) | byte_stable | dicom_validator |
| `vl/microscopic`@1.0.0 | VL Microscopic Image | `1.2.840.10008.5.1.4.1.1.77.1.2` | `1.2.840.10008.1.2.1` (default) | byte_stable | dicom_validator |
| `vl/photographic`@1.0.0 | VL Photographic Image | `1.2.840.10008.5.1.4.1.1.77.1.4` | `1.2.840.10008.1.2.1` (default) | byte_stable | dicom_validator |
| `vl/wsi/multiple-optical-paths`@1.0.0 | VL Whole Slide Microscopy Image | `1.2.840.10008.5.1.4.1.1.77.1.6` | `1.2.840.10008.1.2.1` (default) | byte_stable | dicom_validator |
| `vl/wsi/pyramid-label`@1.0.0 | VL Whole Slide Microscopy Image | `1.2.840.10008.5.1.4.1.1.77.1.6` | `1.2.840.10008.1.2.1` (default) | byte_stable | dicom_validator |
| `vl/wsi/pyramid-thumbnail`@1.0.0 | VL Whole Slide Microscopy Image | `1.2.840.10008.5.1.4.1.1.77.1.6` | `1.2.840.10008.1.2.1` (default) | byte_stable | dicom_validator |
| `vl/wsi/pyramid-volume`@1.0.0 | VL Whole Slide Microscopy Image | `1.2.840.10008.5.1.4.1.1.77.1.6` | `1.2.840.10008.1.2.1` (default) | byte_stable | dicom_validator |
| `vl/wsi/tiled-full`@1.0.0 | VL Whole Slide Microscopy Image | `1.2.840.10008.5.1.4.1.1.77.1.6` | `1.2.840.10008.1.2.1` (default) | byte_stable | dicom_validator |
| `vl/wsi/tiled-sparse`@1.0.0 | VL Whole Slide Microscopy Image | `1.2.840.10008.5.1.4.1.1.77.1.6` | `1.2.840.10008.1.2.1` (default) | byte_stable | dicom_validator |
