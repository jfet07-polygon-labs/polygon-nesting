# Grok review 5 — the lever ledger is closed: stop and consolidate

> Verbatim reply from the local Grok CLI (xhigh, read-only) at HEAD 4d151b0, 2026-08-22.


HEAD `4d151b0`. `coordinator_v3` resta off (`portfolio.rs:2693`).

---

## 1. Architettura 10s per i prossimi 5mm

Non ce n’è una, data l’evidenza. L’interruzione compra tre cose: `Stop` con incumbent exact-valid, `Suspend` vivo sulla coda, sblocco del bound a checkpoint (`general_relaxed.rs:7011-7034`, `7079-7094`, `7115-7118`; `portfolio.rs:2171-2281`). Le tre sono state armate e misurate sullo stesso strumento (`plan=10000` + `plancal`):

| lever | 10s median Δ | vs cosa già fallita |
|---|---:|---|
| `m34past=1` full share | **+0.331 mm** | «il bound, non il grid» (`robust-plan/README.md:574-609`; `portfolio.rs:7624-7631`) |
| `m34yield=2` | **+0.361 mm** | «slice sospeso in coda» (Grok-4 §4) |
| `m34wallstop` | **0.000 mm** | Sol-8 §3.3; a 10s il plan è già 0/9 over (`real-interruption/README.md:432-438`) |
| `past25` / `past50` | −0.264 / −0.221 | stesso lever, share; non 5mm |
| `past` + `step=0.25` | **+3.570 / +3.618** | peggio del control bounded +2.622 (`real-interruption/README.md:39,479-484`) |

La coda shipped a 10s **è già** la policy che l’interruzione avrebbe dovuto scoprire: lo slice è `SCHEDULE_RUNGS = 9` (`portfolio.rs:5545`), esce su `bound`, drop **esatto 1.6160 mm**, `confirmationsAttempted == confirmationsAccepted` (`robust-plan/README.md:576-594`). Preemption marginale al checkpoint durante il walk bounded è un no-op rispetto a quello. Sbloccare il bound e far spendere m34 il resto è esattamente `m34past`, e a 10s «le altre classi spendono meglio» sopravvive (`real-interruption/README.md:587-592`).

Le famiglie già chiuse non diventano vive perché lo slice è sospendibile:

- **secondo bacino m20:** cur2 rende il draw visibile (585 → ~8.17M, 33% del plan) e la race resta **0/9**, peggio a pari piano (`work-currency/README.md:27,476-510,533-539`).
- **rotazione:** blanket +3.721 mm @10s, 0/9 better (`continuous-rotation/README.md:20-21`); sparsa null@10 / contro@30.
- **ranking/pricing:** schedule è già la classe migliore per unità (0.581 mm/M, 17/19 pub; crossover 0/12, descent 0/7) (`coordinator-v4/README.md:427-432`). Un prior a tre livelli non estrae millimetri da classi che pubblicano zero.
- **ladder forzata post phase-0:** l’unico 5mm di classe è arm C su archivio *saturo* a 174 mm dopo ~16 s v2 (`opportunity-ledger/README.md:16-25`), costo 2.39 phase-0 ≈ 21M (`portfolio.rs:1564,1545-1547`). Il plan 10s è 24.9M di cui ~9M già in phase 0. A 10s v3 vs v2 è **0.000 su 6/9**; i 5mm v3 arrivano a **30s** (`coordinator-v3/README.md:186-188,249-262`).

I 5mm che *esistono* sono un’altra curva: v4 40M median-of-seeds **169.891** → 120M **163.927** = **5.964 mm** a 3× work (`coordinator-v4/README.md:407-412`). 10s di wall non contiene 80M di unità a eval/s piatto da mesi (`quality-frontier-trace/README.md:82-108`; `lanes=8` **−2.158 mm**, 0/9 — `parallel-compression-schedule/README.md:26-27`).

---

## 2. Gap wall-vs-work (~4–6 mm)

Misura canonica, non ~4–6: **+6.904 mm** plan vs wall, mixed-61 10s (`calibrated-plan/README.md:459-466,511-518`). HEAD: callive **175.388** @ ≤8.28 s vs wall **168.484** @ 10.30 s (`real-interruption/README.md:507-508`).

| pezzo | mm | comprabile? |
|---|---:|---|
| `PLAN_PHASE_ZERO_BIAS = 1.70` (`portfolio.rs:1245`) | **3.741** | No come ingegneria residua. Un costante non fittà un range 1.12–1.59× per seed; sotto carico è il trade già prezzato (`robust-plan/README.md:44,378-384`: plan 0/60 over *sotto-comprando*, callive 14/60). `planhead=0.85` è il dial, non un millimetro gratis. |
| counter tax (`profiling` on per tutto il plan) | **1.882** | **Sì, ~tutto.** `cur2` non lo recupera (`work-currency/README.md:586-621`). Il spend è sollevare `surrogate_evaluations` sulla relaxed lane come già fa `CompressionSchedule::work_spent` (`compression_schedule.rs:580-587`) e far girare il work budget a `profiling::set_enabled(false)`. |
| `PLAN_QUANTUM_STEP = 1.15` floor (`portfolio.rs:1290,3505-3507`) | **1.281** | Già speso: replan compra **2.808 mm su un seed, 0.252 mm di mediana** (`next-generation-engine-plan.md:6908-6913`). `planraw` è l’altro capo. |
| leftover wall (~7.2 s su 10, `real-interruption/README.md:432`) | dentro i tre | Una coda opportunistica post-work è il modo wall: quiet→~168.5, load→distribuzione. Non è un lever nuovo. |

Cosa **non** chiude il gap:

- conferme a 0.26 ms: già dentro entrambi i bracci (`m34pconfirm` default in v3, `calibrated-plan/README.md:53-58`; 3.11× sul serial).
- batch: 1118/27 bit-identici al monolite; non cambiano la quantità di ricerca.
- `m34wallstop`: 0 mm @10s; @30s taglia wallMax 36.42→31.98 e **3/9 sforano ancora**, perché la policy non lega le altre classi né un batch in volo (`real-interruption/README.md:38,594-601`).

Split onesto: **~1.9 mm ingegneria (debit lane-local). ~5 mm strutturali** al contratto «un documento per seed sotto carico» ∧ «spendere i 10 s». Nessuna formula dà entrambi (`sol-review-9-m34cap-provenance.md:91-94`).

Il 40M→120M (+5.96) è *altro* gap. A 30s plan è già **164.188**, banda 120M. A 10s wall sei già sulla banda 40M (~168.5). I 5.96 non stanno in 10 s.

---

## 3. Qualità-per-azione dal record 155.264

Il record è un’altra lineage, allowance `0.0005`, ore, parent già a 164→159 (`record-line-cascade/README.md:42-43,51-61`; `orientation-floor/README.md:20-40`):

- clamp sub-grid `step=0.25` a `past=1, work=20M` su parent **159.668** → −1.000 mm
- poi m22 / flatten→m33 / m26 ladder / 266 arm grind → 155.422
- poi rung 0.00128°, flatten 0.25→m30, rotazione *in entry* → **155.264**

La fisica trasferibile era una sola: passo sub-grid = più pressione exact per µm **quando il walk è budget-limited e il parent è già stretto sul floor** (`record-line-cascade/README.md:16-26,99-106`). A 10s from-request le tre premesse sono false:

1. lo slice è distance-limited, non budget-limited (`portfolio.rs:7624-7631`);
2. l’exact non rifiuta nulla (`robust-plan/README.md:592-594`) — densità extra = 25.7× work per 0 mm;
3. il parent del primo slice è ~179 mm, non 159.

Trasferimento misurato: `step=0.25` bounded +2.622; con bound aperto **+3.6** (`real-interruption/README.md:39`). Ladder 10s: v4 chiude in compression 9/9 (`coordinator-v4/README.md:397-398`). Rotazione continua *nel search* — quello che Grok-1 indicava per 162→150 — a 10s è **+3.721 mm** (`continuous-rotation/README.md:20-46`; `grok-review-1-independent-v5.md:205`). Witness/m33: 0/12 pubblicazioni discendenti.

Il costruttore non è un parent più profondo in 10s: curva di qualità chiusa in **1.4 s @ ~180 mm**, poi 25 s di coda a **0.000 mm** (`quality-frontier-trace/README.md:85-108`). La coda v3 a 10s non pesca m20 (`work-currency/README.md:533-539`).

Lower bound d’area: **131.98 mm**; 155 non è escluso (`depth-lower-bound-evidence.json:16-18`). Sparrow 150.165 @10s stessa macchina. Gap wall-ref: **18.3 mm**; gap plan: **25.2 mm**; gap record-ore vs Sparrow-10s: **5.1 mm**.

**Verdetto:** 150@10s su questo box richiede un’idea fuori dallo spazio operatori {m20,m22,m23,m26,m31,m33,m34 + rotazione continua/sparsa + overlay + race}. Grok-1 aveva già messo il tetto onesto a ~166@10s; il wall-ref 168.5 *è* quel tetto. I 12–18 mm sotto non sono schedule.

---

## Prossime 3 spese

**Fermarsi e consolidare.**

La campagna 10s ha chiuso il P0 (interruzione vera, m34cap ritratto), ha reso il plan load-robusto (`plancal` 60/60, `robust-plan/README.md:41`), e ha esaurito scheduling / pricing / strumento con segno. Ogni round in più su un lever di questa famiglia replica un negativo già nominato.

Se si spende comunque, non è per i 5mm@10s:

1. **Consolidare la superficie shipped:** v3+`plancal`+pconfirm+interruption default-off; ritirare dalla board race, `cur2=1`, `m34past`/`yield`/`grid1`/`confirm1`, `lanes=8`, `adopt`, `crot`. Tenere `m34wallstop` come dial del contratto 30s, non come qualità 10s.
2. **Unico millimetro d’ingegneria residuo:** debit lane-local m22/m23 (`work-currency/README.md:608-621`) — recupera fino a 1.882 mm del plan, non avvicina Sparrow.
3. **Igiene 30s:** wall-stop su *tutte* le classi, non solo m34 (`real-interruption/README.md:594-601`). Chiude 3/9, 0 mm di profondità.

Non aprire un round «scheduler V2 / secondo bacino / densità sul bound aperto / ladder forzata». Sono le domande che i tre round recenti hanno già risposto.