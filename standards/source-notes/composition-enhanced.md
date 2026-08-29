# Enhanced image composition source note

Checked against the locked DICOM 2026b base edition on 2026-08-29.

- PS3.3 Enhanced CT, Enhanced MR, and Enhanced PET IOD module tables establish
  their mandatory multi-frame functional-group, dimension, equipment,
  acquisition, anatomy, and pixel modules.
- PS3.3 Multi-frame Functional Groups, Dimension Organization, and Dimension
  Index modules establish one shared item, one per-frame item per frame, and
  consistent dimension-index cardinality and ordering.
- PS3.3 Concatenation attributes establish one-based instance numbering and
  frame offsets across a concatenation.
- PS3.5 native Pixel Data encoding establishes exact frame byte length, sample
  layout, value padding, and transfer-syntax byte order.

Composition defaults reuse the suite's already qualified synthetic objects and
then resolve them through the shared typed plan. Caller content is accepted
only when its complete frame shape matches the selected qualified structural
variant. This evidence does not claim arbitrary unqualified dimensions,
photometric models, compressed syntaxes, or full-scale resource behavior.
