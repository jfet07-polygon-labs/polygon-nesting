# Grok review 6 — no operator exists; Gate A: does Sparrow's solution pass our composite?

> Verbatim reply (local Grok CLI, xhigh, read-only) at HEAD 5ccab1c, 2026-08-22.


HEAD `5ccab1c`. Consolidamento fatto (`docs/shipped-surface.md`). Governor escluso. I leftover sotto sono uccisi dalla causa già nominata, con i millimetri, non per analogia.

---

## 1. Perché non c’è l’operatore

Ogni classe che muove un fronte di contatto, sul legale set attuale, ha un tetto misurato di **micron–mezzo millimetro**. Il gap wall-ref è **18.319 mm** (`docs/experiments/real-interruption/README.md:508`). Le due scale non si toccano.

| classe | tetto committato | perché non è i 18 mm |
|---|---|---|
| Contact-block, gate **composito** (quello che l’engine accetta) | 1/12, **0.0438 mm**, mm/work **0.0030** vs m34 | `docs/experiments/contact-block/README.md:189–203` |
| Stesso operatore, gate **solo contratto** (ritratto) | mediana **0.506 mm**, **0/12** accettati come parent | `README.md:147–151` — il numero grosso è illegale |
| Headroom strutturale del blocco da 5 | mediana **0.0036 mm** (trust 0.5); **0.0 mm** a trust 1.0 | `docs/experiments/contact-block/evidence/why.json:27,55` |
| Witness SE(2) one-shot | profondità finale mossa **0/12** | già in mappa |
| m34 da parent 171–179 | **1.104 mm** a 3.34 M | `contact-block/README.md:194` — e a 10 s lo slice è *distance-limited* a **−1.616 mm** e esce su `bound` (`docs/experiments/robust-plan/README.md:576–594`) |

Il leftover di Sol in `contact-block/README.md:407–413` (ε > 0 sulle righe envelope) **non evade** la causa di morte. La causa non è “manca slack nel LP”. È: il vincolo che lega è la **griglia miter dell’envelope**, modellata come contatto a `contract_mm = 0.0`; risolvere meglio il modello **peggiora** (`README.md:390–394`, sweep `iters=2000`). Mettere ε rimpicciolisce il passo linearizzato. Il full-step già sopravvive **una volta su quattro** e lo scale mediano è **0.01** (`why.json:29–30`). Non c’è un secondo programma nello stesso grafo da 5 pezzi: il headroom del blocco è 3.6 µm.

Lo stesso per le altre tentazioni:

- **Proiettare i 0.506 mm ritirati** (contract-step → `global_legalize` / m16). I 0.506 mm sono un moto *locale*. `global_legalize` sui residui grandi “walks back out of the basin” (`crates/polygon-nesting-core/src/search/general_micro_legalization.rs:5–7`). Compose envelope-valido è già **−0.030 mm** (`contact-block/README.md:353–364`). Non è 18 mm, e non è un basin nuovo.
- **Pack esatto delle 5 pezzi più profonde** (mode 18). Mode 19 ha già evacuato la banda e il greedy l’ha rimessa nei pozzetti (`docs/next-generation-engine-plan.md:284`). A 10 s from-request il parent è ~179, non un lock a 155. Stesso tetto di headroom.
- **Costruttore “più forte”** senza meccanismo. La curva **chiude a 1.4 s @ ~180 mm** (`docs/experiments/quality-frontier-trace/README.md:85–88`). I tre incrementi nominati sono già stati misurati: gap-stations **REJECTED**, mating **NEUTRAL**, pocket-refill **NOT ADOPTED** e 61/61 identico al controllo (`docs/next-generation-engine-plan.md:834–854`). I restart m20 producono bacini **peggiori** e vengono rifiutati (`quality-frontier-trace/README.md:136–139`). Non ho un quarto meccanismo concreto che evada “la qualità del costruttore satura in 1.4 s”.
- **Separator più a lungo, “come Sparrow”.** m0+coupled è **piatto dopo 1.4 s** (`quality-frontier-trace/README.md:85–88`). I trapianti isolati del lifecycle Sparrow sono una catena di negativi (`next-generation-engine-plan.md:151–233`). Il substrato è già a parità: **3.775 M vs 3.742 M eval/s**, e noi facciamo **più** mosse (33.9 K vs 14.2 K) (`next-generation-engine-plan.md:2031–2032`). Non è eval/s.
- Scheduler, `m34past`, densità, race, `lanes=8`, crot/sparse, `adopt`: superficie ritirata (`docs/shipped-surface.md:96–116`). Non li riapro.

Due gap, non uno, e nessuno dei due sta in un’azione da 10 s:

| tratto | mm | dove vive | perché 10 s non lo compra |
|---|---:|---|---|
| wall-ref 168.484 → record 155.264 | **13.2** | **dentro** il legale set attuale (`exactValid` ∧ `contractValid`, stesso 5.0/5.0) | 40 M → 120 M = **+5.964 mm** (`docs/experiments/coordinator-v4/README.md:407–412`); 10 s ≈ banda 40 M; il record è cascata a ore, 7 204 arm, parent già 159–164, allowance `0.0005` (`docs/experiments/orientation-floor/README.md:42,129–133`; `docs/experiments/record-line-cascade/README.md:51–61`). Le tre precondizioni di quel trasferimento a 10 s from-request sono false (slice distance-limited; exact che non rifiuta; parent ~179). |
| record 155.264 → Sparrow 150.165 | **5.1** | forse **fuori** dal legale set di search | vedi §2. Ore di grind sullo stesso contratto non l’hanno chiuso. |

Grok-1 aveva messo il tetto onesto a ~166 @10 s. Il wall-ref **168.484** è quel tetto. Non è un operatore mancante nella coda.

---

## 2. Cosa riapre il gioco (e cosa no)

Il 5.0/5.0 materiale è già allineato a Sparrow (`docs/next-generation-engine-plan.md:730–741, 763–784`). Quello che Sparrow **non** ha, e che questo engine usa come autorità di accettazione, è il secondo predicato del composito:

```3531:3538:crates/polygon-nesting-core/src/search/general_fast.rs
/// The composite acceptance check: search-envelope admissibility *and* contract
/// validity, in that order.
///
/// The envelope half rebuilds every placement's canonical collision polygon -
/// the source offset by [`collision_expansion_mm`], ...
/// requires each to fit the sheet and to be pairwise disjoint on the canonical
/// grid.
```

L’envelope è un offset **miter** (`JoinType::Miter`, `CLIPPER_MITER_LIMIT = 2.0`) (`crates/polygon-nesting-core/src/geometry/general_polygon.rs:28,387`), via `PolygonSet::offset` (`crates/polygon-nesting-core/src/search/kernel/exact.rs:136–148`). Il miter è un **sovrainsieme** del disco di Minkowski: `offset_miter(P, e) ⊇ P ⊕ disc(e)` (`docs/experiments/constructor-inner-certificate/README.md:37`). Due spigoli convessi in diagonale spingono gli envelope a toccarsi mentre il materiale è ancora a 5.9 mm (`crates/polygon-nesting-core/src/search/general_micro_legalization.rs:26–30`). Sui parent record, `EnvelopePair` slack è **esattamente 0.0** — gli envelope toccano, il materiale no (`docs/next-generation-engine-plan.md:5795–5798`). È la stessa frase con cui il contact-block è morto (`contact-block/README.md:252–257`).

`validate_publication` (anelli sorgente, 5.0/5.0) e `validate_and_measure_placements` (griglia envelope) **non si implicano** (`general_micro_legalization.rs:16–24, 3452–3467`). L’accettazione usa il composito. Sparrow è stato validato solo sul materiale: min pair 5.00084, bordo 5.00096 (`docs/experiments/sparrow-mixed61/validation-10s-x86.json:13–21`). **Non risulta, dopo il ricalibro 5.0/5.0, un import di `solution-10s-x86.json` attraverso il composito.** Il mode-13 che fallì era sul contratto 5.5 (`docs/experiments/persistent-vacancy-descent/seeded-reconstruction-evidence.json:6–8`).

Quindi il cambiamento esterno che riapre è uno di questi due, in quest’ordine, e **non** è un operatore:

### A. Rappresentazione / predicato di accettazione (il lever vero)

Due forme, stessa fisica:

1. **Join tondo (o square che contiene il disco) al posto del miter**, sullo stesso raggio `total_padding/2`. La costante `FUTURE_ROUND_JOIN_ARC_TOLERANCE_MM` è già nel tree e **non è sul path di search** (`crates/polygon-nesting-core/src/geometry/collision_builder.rs:136–137` — path legacy, miter 10; il search usa `general_polygon.rs:387`). Non è la probe sull’allowance 0.002: quella cambia il **raggio** del miter e ha chiuso a rumore, un slack-release (`next-generation-engine-plan.md:1005–1013`). Il join cambia la **forma agli spigoli**, che è dove il contact-block è pinato.
2. **Pubblicare sul solo contratto materiale** (`validate_placements_against_contract`), tenendo l’envelope come filtro di search o abbandonandolo. È un cambio di contratto prodotto: il core oggi promette la griglia canonica come autorità (`docs/architecture.md` + `docs/next-generation-engine-plan.md:36–42`).

Cosa compra: al massimo il tratto **155.264 → 150.165** (e la raggiungibilità di topologie che il miter vieta). **Non** comprime 120 M in 10 s e **non** fa emettere al costruttore un bacino da 155 in 1.4 s.

Gate da un round, prima di toccare la coda:

1. Mappare `docs/experiments/sparrow-mixed61/solution-10s-x86.json` con il converter già committato (`persistent-vacancy-descent/sparrow-to-hint-fixture.py`).
2. Tre verdetti sulla stessa posa: contratto solo / composito miter (HEAD) / composito round-join.
3. Se il composito miter rifiuta e il contratto (o il round) accetta: la rappresentazione è il gap residuo da 5 mm; un A/B 10 s miter vs round è il round successivo, non questo.
4. Se il composito miter **accetta** 150.165: il legale set contiene Sparrow e ore di cascade hanno comunque chiuso a 155.264 — allora A non basta, serve B.
5. Se anche il contratto rifiuta: i 150.165 non sono comparabili; smettere di usare quel numero come target.

### B. Architettura di search (solo se A.4)

Un layout infeasible, compresso-e-riparato in continuo, mai ricominciato — il lifecycle Sparrow intero, non un trapianto di scalare, non una voce v3. I pezzi isolati sono morti; m0+coupled satura a 180 in 1.4 s. Questo **non** è un’azione nel coordinatore. È sostituire lo stack 10 s (costruttore exact-valid → slice m34 discrete). Licenza/prodotto, non un round di feature flag. Gate: un processo, from-request, 10 s, stesso box; se la mediana non batte 160, tagliare. Non lo venderei prima di A.4.

### Cosa non riapre

| cambio | perché è morto come via a 150 @10 s |
|---|---|
| **Hardware** | eval/s già a parità. `lanes=8` è **0.912×** e **−2.158 mm** (`docs/shipped-surface.md:110`). 3× work = +5.96 mm, non +18. |
| **Definizione del budget** | 30 s / 120 M → ~164 (`coordinator-v4/README.md:410–412`). Ore → **155.264**. Sparrow fa 150 in **10 s**. Allungare il budget non batte Sparrow-10 s. Compra al più 168 → 164. |
| **Governor hard-wall** | Escluso (`docs/shipped-surface.md:227–228`). Comunque il gap wall-vs-work è **6.904 mm** (`docs/experiments/calibrated-plan/README.md:459–466`): 175 → ~169, non 169 → 150. Sol 10 lo aveva già detto. |
| **Contratto materiale 5.0/5.0** | Già fatto. Il 7 mm di overhead era il 5.5 (`docs/experiments/depth-lower-bound/depth-lower-bound-evidence.json:16–18`) e si è realizzato (`next-generation-engine-plan.md:741–744`). Non è più in gioco. |

---

**Verdetto.** Su questo box, con accettazione composita miter-grid, non c’è un meccanismo nuovo che avvicini 150.165 @10 s fondato sui fallimenti misurati. Lo spazio `{m20,m22,m23,m26,m31,m33,m34 + rotazione + overlay + race + contact-block}` è chiuso; il leftover ε è la stessa morte. L’unica spesa onesta da ≤1 round è il **gate A** sull’import Sparrow — e quello decide se il gioco si riapre sulla rappresentazione, o se anche il legale set contiene 150 e allora l’unica leva rimasta è cambiare motore, non la coda.