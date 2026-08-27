# Locked WSI reconstruction backend

This optional `uv` project independently reconstructs the exact total pixel
matrices of `vl/wsi/tiled_full_small` and `vl/wsi/tiled_sparse_small`. It uses
highdicom's stored-frame APIs with all modality, VOI, palette, presentation,
real-world, and ICC transforms disabled. For `TILED_FULL`, the adapter also
cross-checks highdicom's total-pixel-matrix API against a separate implicit
tile-order implementation. For `TILED_SPARSE`, it independently cross-binds
the Dimension Index Sequence and all required per-frame macros, places only
the two encoded tiles into a zero-sentinel matrix, and emits the exact
occupancy mask so absent tiles cannot be mistaken for encoded black pixels.

The adapter fails closed on SOP Class, transfer syntax, geometry, frame order,
dimension indices, explicit positions, macro placement, occupancy, payload,
or hash drift. It is a payload validator, not the corpus generator or the
independent IOD authority.

Provision and test offline after the pinned artifacts have been cached:

```sh
UV_CACHE_DIR=/private/tmp/dts-uv-cache uv sync --locked --offline
UV_CACHE_DIR=/private/tmp/dts-uv-cache uv run --locked --offline python -m unittest discover -s tests
```

Run it against one generated instance:

```sh
uv run --locked --offline dts-wsi-reconstruct --input /path/to/instance.dcm
```
