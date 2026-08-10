# Canonical quality golden

The current canonical matrix protects 18 production-scale rows: Triangle-20, Mixed-61, and Shapes-17 on 2000×2700, 600×400, and 300×300 sheets, under both Compact and Short Side profiles.

Every normal CI run must match the accepted placement counts, layout fingerprint, and score metrics in `tests/fixtures/canonical-quality-golden.json`. A successful engine outcome is not sufficient: a legal but looser layout fails the gate.

The protected metrics cover occupied bounds, normalized sheet consumption, hull waste, free-material fragmentation and slivers, largest reusable free region, and structural contact. Layout fingerprints use normalized complete placed-collision geometry and unplaced IDs, so a different layout always requires explicit review even when its aggregate score happens to match.

## Promotion policy

Run the same matrix with `--update-golden` only after inspecting an intentional algorithm change:

```sh
node scripts/run-current-canonical-matrix.mjs \
  --adapter target/release/parity-desktop-request-adapter \
  --cli target/release/polygon-nesting \
  --update-golden
```

The command refuses the update unless all of these conditions hold:

- no row places fewer pieces or leaves more pieces unplaced;
- at least one continuous metric improves by 0.25% or one count metric improves by at least one;
- no continuous metric regresses by more than 0.5%;
- no count metric regresses by more than one;
- the 18-row corpus is unchanged.

This deliberately allows a small trade-off only when another measured outcome materially improves. An unchanged result, a layout-only change, a lost placement, or a material regression cannot replace the golden.

Review the reported improvements and slight regressions together with the JSON diff before committing an updated golden. Changes to the policy itself require the focused tests in `scripts/canonical-quality.test.mjs` to be updated in the same pull request.
