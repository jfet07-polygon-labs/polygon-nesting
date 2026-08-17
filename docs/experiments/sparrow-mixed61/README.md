# Sparrow Mixed-61 calibration

This directory pins the portable evidence for the external Mixed-61 quality bound used by the next-generation engine plan. It is a calibration against upstream Sparrow, not evidence about Lapas implementation lineage.

## Recorded result

- Sparrow revision: `14f4868fcd7e97036700dbebaf193fb159180aa9`
- Sparrow executable SHA-256: `3c18f30c13fd853ccc7cca04fed5d28b60947247632347abdcd621292272c591`
- Sparrow `Cargo.lock` SHA-256: `157e75a93916337a3dc8330a22817a009bf2d9f7ccd2431e2236f108dcc1f5ae`
- toolchain: rustc `1.95.0 (59807616e 2026-04-14)`, LLVM `22.1.2`, target `aarch64-apple-darwin`
- build flags: `RUSTFLAGS='-C target-cpu=native'`, release mode, locked dependencies, feature `only_final_svg`
- run policy: three-second global budget, seed `0`, eight workers, continuous rotations, 5 mm part separation, 5 mm sheet inset
- result: all 61 pieces placed at `154.44858 mm` strip depth
- independent validation: exact 61-item demand multiset, minimum pair distance `5.000259999999969 mm`, minimum sheet-boundary distance `5.0003288600859594 mm`, valid with a `0.002 mm` export-precision tolerance
- canonical placed-items SHA-256: `a91cf81f22ad6088e4a6a8aaa26dd7371861cb32b4aaf4cc0b43bec39efe3be1`

The lower edge of a `~150 mm` target remains aspirational. A historical same-machine ten-second observation reached `152.49449 mm`, but its full raw output was not retained in this checkpoint and it must be reproduced before it is used as an acceptance artifact. The durable demonstrated reference here is `154.44858 mm`.

## Artifact hashes

```text
39dbc276cb1ce94fed5ec95daf66dc4adc1d2e5ffebbdd2c7756f1a8276817e9  input.json
5cd160c13ec7a3bd8f063214bd58502537c7595ed3ec21ee976251fb045a6e86  mixed61-to-sparrow.mjs
fc16d934db61499d33aaa5a2c9dc7fbcbeaf9b60caa8c8f8bb65738f4d25bc55  solution-3s.json
49301747a4a25ddb1b37d8de12312d9909e22330ab7b2ada918f9934cbb42447  validate-sparrow-solution.mjs
60d651f3f2f574f098c3ca1890f50590aaa4e9a68a1181b183a292412d779c8a  validation-3s.json
```

## Reproduction

From a polygon-nesting checkout:

```sh
node docs/experiments/sparrow-mixed61/mixed61-to-sparrow.mjs \
  tests/fixtures/mixed-61/mixed61-request.json \
  docs/experiments/sparrow-mixed61/input.json

git clone https://github.com/JeroenGar/sparrow.git ../sparrow
git -C ../sparrow checkout 14f4868fcd7e97036700dbebaf193fb159180aa9
(cd ../sparrow && RUSTFLAGS='-C target-cpu=native' \
  cargo build --release --locked --features only_final_svg)
(cd ../sparrow && /usr/bin/time -l target/release/sparrow \
  --input ../polygon-nesting/docs/experiments/sparrow-mixed61/input.json \
  --global-time 3 --rng-seed 0 --min-item-separation 5 --workers 8)

cp ../sparrow/output/final_mixed61-polygon-nesting.json \
  docs/experiments/sparrow-mixed61/solution-3s.json
node docs/experiments/sparrow-mixed61/validate-sparrow-solution.mjs \
  docs/experiments/sparrow-mixed61/input.json \
  docs/experiments/sparrow-mixed61/solution-3s.json 5 \
  > docs/experiments/sparrow-mixed61/validation-3s.json

jq -cS '.solution.layout.placed_items | sort_by(.item_id)' \
  docs/experiments/sparrow-mixed61/solution-3s.json | shasum -a 256
```

## x86_64 same-machine addendum (2026-08-17)

Re-run on the project's x86_64 Linux box (16 cores, Linux 6.18, rustc
1.93-era toolchain, `-C target-cpu=native`, dependencies resolved fresh -
upstream checkout at the pinned revision carries no `Cargo.lock`, so
`--locked` is not reproducible; deviation noted):

- 3-second budget, seed 0, 8 workers: `157.97073 mm`, exact-valid
  (61/61, minimum pair distance `5.003296 mm`) - `~3.5 mm` behind the
  M4 calibration at the same budget, attributable to hardware/toolchain.
- 10-second budget, seed 0, 8 workers: `150.16547 mm`, exact-valid
  (61/61, pair distances >= 5.0), raw solution and validation RETAINED
  (`solution-10s-x86.json`, `validation-10s-x86.json`) - this replaces
  the unretained historical M4 ten-second observation as acceptance-grade
  evidence that the ~150 mm band is reachable in seconds on this machine.
- Measured throughput during the separation phase: `3742 K evals/s`,
  `14.2 K moves/s`, `460 iter/s` across 8 workers - the
  orders-of-magnitude per-iteration cost gap against our engine is the
  production bottleneck, not mechanism quality.
