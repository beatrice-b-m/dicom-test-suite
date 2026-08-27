# Locked WSI reconstruction backend

This optional `uv` project independently reconstructs the exact total pixel
matrix of `vl/wsi/tiled_full_small`. It uses highdicom's stored-frame and total
pixel matrix APIs with all modality, VOI, palette, presentation, real-world,
and ICC transforms disabled. A separate implementation derives the implicit
`TILED_FULL` tile order and must produce byte-identical pixels.

The adapter fails closed on SOP Class, transfer syntax, geometry, frame-order,
metadata, or hash drift. It is a payload validator, not the corpus generator or
the independent IOD authority.

Provision and test offline after the pinned artifacts have been cached:

```sh
UV_CACHE_DIR=/private/tmp/dts-uv-cache uv sync --locked --offline
UV_CACHE_DIR=/private/tmp/dts-uv-cache uv run --locked --offline python -m unittest discover -s tests
```

Run it against one generated instance:

```sh
uv run --locked --offline dts-wsi-reconstruct --input /path/to/instance.dcm
```
