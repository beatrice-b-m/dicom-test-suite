# Composition classic-image source note

- Checked: 2026-08-28
- DICOM edition: 2026b
- Scope: classic and single/multi-frame visible-light image composition
- Qualification owners: `classic_*` and `visible_light`

The classic-family composition templates follow the applicable IOD module
tables in PS3.3 and native Pixel Data encoding rules in PS3.5. Shared typed
plans provide Patient, General Study, General Series, Frame of Reference when
required, General Equipment, General Image, Image Pixel, and SOP Common
modules. Family plans add the modality acquisition, geometry, detector,
display, and content-identification attributes required by each IOD.

SOP identity, Study and Series identity, Frame of Reference identity when
present, pixel shape, and Pixel Data are resolved structural state. Callers may
set documented non-structural metadata but cannot contradict those fields.
Default content is bounded, deterministic, synthetic, and contains no PHI.

Qualification is specific to the transfer syntax and pixel model recorded by
each descriptor. Native qualification does not imply compressed codec support,
and same-project validation is supplemented by the pinned independent IOD
adapter before a descriptor is promoted to qualified.

## Official anchors

- PS3.3, the IOD module tables for CR, CT, MR, DX, Mammography, Ultrasound,
  Nuclear Medicine, PET, VL, XA, XRF, and multi-frame SC storage classes.
- PS3.3, common Patient, Study, Series, Equipment, General Image, Image Pixel,
  acquisition, detector, display, and SOP Common modules.
- PS3.5, native Pixel Data layout, VR selection, byte order, and even-length
  padding.
