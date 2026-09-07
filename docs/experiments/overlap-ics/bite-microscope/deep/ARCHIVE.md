# The depth-microscope archive

The 42 microscope documents (`deep-<A|B>-wall10s-s<seed>.json`, 18–30 MB each) and the 100
live-horizon replays (`replays/rp-…json`, 3–30 MB each) total 2.4 GB raw; they stay on the
measurement host under `/var/lib/t3/tmp/astra/deep/` and are identified here by sha256
(`archive-manifest.txt`). Committed beside this file: the four documents of seeds 27 and 31 (both
arms, gzipped), the summariser's full printout (`deep-cut-output.txt`), the runners
(`deep-run.sh`, `deep-replays.sh`, `deep-chain.sh`), the seed list (`deep-seeds.txt`: 27–35 and
twelve v2 seeds), the chain log and the freeze log of the binary `frozen-5dddb1e`
(sha256 c56c58538be086a3…, bit-identical to `frozen-1e913f4` on the default and the treatment
paths), and the analysis scripts with their outputs under `analysis/`.
