# Polygon input v1

`run-polygons` accepts a small, versioned JSON document for callers that already have polygon vertices but do not construct the complete protocol `EngineRequest`.

The published JSON Schema is `schemas/cli/polygon-input-v1.schema.json` in the npm package and OCI image.

```json
{
  "version": 1,
  "polygons": [
    {
      "id": "rectangle",
      "quantity": 2,
      "allowRotation": true,
      "allowMirror": false,
      "points": [[0, 0], [80, 0], [80, 40], [0, 40]]
    },
    {
      "id": "triangle",
      "points": [
        { "x": 10, "y": 20 },
        { "x": 70, "y": 20 },
        { "x": 40, "y": 60 }
      ]
    }
  ]
}
```

## Fields

The top-level `version` must be `1`. The UTF-8 JSON document may contain at most 67108864 bytes. `polygons` must contain between 1 and 1000 definitions. IDs must contain 1 through 256 characters and at least one non-whitespace character. Each definition may contain at most 4096 vertices, the aggregate may contain at most 1000000 vertices, the sum of all quantities must not exceed 10000, and the aggregate quadratic simple-ring validation budget is 10000000 non-adjacent edge pairs.

Each definition has these fields:

| Field | Required | Meaning |
| --- | --- | --- |
| `id` | yes | Non-empty unique source-piece ID. Definitions are sorted by this value before quantity expansion. |
| `points` | yes | Ordered polygon boundary in millimetres, with at least three non-collinear vertices. Each point may be `[x, y]` or `{ "x": x, "y": y }`. |
| `quantity` | no | Positive integer, default `1`. Instances are named `<id>#1`, `<id>#2`, and so on. |
| `allowRotation` | no | Per-polygon rotation permission, default `true`. |
| `allowMirror` | no | Per-polygon mirror permission, default `true`. The command-level `--allow-mirror false` disables mirroring for every polygon. |

Unknown fields, non-finite coordinates, repeated non-closing vertices, and self-intersecting rings are rejected. A repeated closing point and consecutive duplicate points are removed. Coordinates may use any finite origin: the adapter translates each polygon so its minimum X and Y are zero while retaining its dimensions and boundary. Clockwise and counter-clockwise simple boundaries are both accepted, and the ring is canonicalized so its starting vertex and winding do not change the generated request.

The engine's collision contract remains unchanged: it computes the conservative convex hull of the supplied boundary and applies the configured clearance. The canonicalized equivalent boundary remains in `sourcePieces` for result projection and export. Use the canonical `run --input` command when a caller needs curves, holes, multiple contours, custom optimizer settings, or any other complete protocol capability.
