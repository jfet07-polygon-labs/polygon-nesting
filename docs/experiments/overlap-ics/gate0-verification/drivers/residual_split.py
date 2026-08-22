#!/usr/bin/env python3
"""Where a stalled cell's residual is NOT: the strip's own boundary row.

    python3 residual_split.py <cell.json> [cell.json ...]

A cell document reports `census.activeEdgeRows` and `census.maxEdgeViolationMm`
but not which of the four boundaries carry them. One of the four - the top row,
the strip target `T` - can be bounded exactly from numbers the document already
prints, with no new run and no new instrument.

Two conventions meet in this engine and they are **not** the same number:

  * `state::raw_source_depth_mm` and the `proxy_depth <= T` publication gate use
    `Contract::sheet_edge_clearance_mm`, the settings field;
  * `broad_phase::boundary_residuals` uses `Contract::edge_clearance_mm()`,
    which is that field **plus the flattening sag tolerance**.

Writing `E` for `edge_clearance_mm()` and `s` for the sag, the deepest material
point is `y_max = depth - (E - s)` and the top row's threshold is `T - E`, so

    max top-row residual  =  y_max - (T - E)  =  depth - T + s.

`max_i box_i[3]` is a maximum over the same ring points `raw_source_depth_mm`
maximises over, so this is the maximum over pieces, not a bound on one of them.

The consequence is the reason this file exists. When `maxEdgeViolationMm`
exceeds `depth - T + s`, at least one active boundary row is a **sheet** row -
left, right or bottom - and therefore a row the round kernel's own
`boundary_admissible` scan can see. The residual is then not an artefact of the
strip target being an objective device that the exact authorities do not model.

On mixed-61's exact-clearance contract `s = 0` and the two conventions coincide.
On triangle-20 `s = 0.25 mm`, so Phi's strip boundary is one sag tolerance
*stricter* than the publication depth gate it is descending toward: a layout can
satisfy `proxy_depth <= T` while Phi still charges it up to 0.25 mm of top-row
violation. That asymmetry is reported here as a fact about the instrument.
"""
import json
import sys


def split(path):
    doc = json.load(open(path))
    contract = doc.get('contract', {})
    outcome = doc.get('outcome', {})
    proxy = outcome.get('proxy', {})
    census = outcome.get('census', {})
    edge_e = contract.get('sheetEdgeClearanceMm')
    sag = contract.get('flatteningSagToleranceMm')
    target = doc.get('entry', {}).get('lockedTargetMm')
    depth = proxy.get('rawSourceDepthMm')
    max_edge = census.get('maxEdgeViolationMm')
    if None in (edge_e, sag, target, depth, max_edge):
        return {'cell': path, 'error': 'document lacks a field this derivation needs'}
    top = depth - target + sag
    return {
        'cell': doc.get('cell'),
        'document': path,
        'lockedTargetMm': target,
        'finalRawDepthMm': depth,
        'edgeClearanceWithSagMm': edge_e,
        'flatteningSagToleranceMm': sag,
        'depthConventionEdgeClearanceMm': edge_e - sag,
        'publishedDepthSlackMm': target - depth,
        'maxTopRowResidualMm': top,
        'maxEdgeViolationMm': max_edge,
        'maxPairViolationMm': census.get('maxPairViolationMm'),
        'activeEdgeRows': census.get('activeEdgeRows'),
        'activePairRows': census.get('activePairRows'),
        'topRowIsViolated': top > 0.0,
        'aSheetBoundaryRowIsViolated': max_edge > top,
        'sheetBoundaryResidualAtLeastMm': max(0.0, max_edge) if max_edge > top else 0.0,
    }


def main():
    rows = [split(path) for path in sys.argv[1:]]
    print(json.dumps({
        'experiment': 'overlap-ics',
        'derivation': 'max top-row residual = final raw depth - T + sag',
        'cells': rows,
    }, indent=1))


if __name__ == '__main__':
    main()
