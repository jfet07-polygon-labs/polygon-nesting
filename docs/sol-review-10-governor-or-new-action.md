# Sol review 10 — one governor round with a kill-gate, then a new search action

> Verbatim final reply from the resumed codex session (max effort, read-only) at HEAD 4d151b0, 2026-08-22.

Verdetto su HEAD `4d151b0`: l’interruzione è ora infrastruttura corretta, ma non crea qualità. Non vedo più un riordino degli operatori esistenti che possa onestamente promettere 5 mm a lavoro fisso. Il prossimo guadagno osservabile viene dal rendere spedibile il braccio wall; da circa 168.5 verso 150 serve invece un operatore nuovo.

## 1. Architettura 10 s

La sola architettura credibile per i prossimi 5 mm è un **canonical-prefix hard-wall**:

1. Fase 0 protetta.
2. Stessa sequenza deterministica della queue, inizialmente limitata a `m34 → m22`.
3. Ogni operatore lungo espone `advance_one_quantum()`.
4. Il clock non sceglie né riordina: decide soltanto quando restituire l’ultimo incumbent già exact-valid.
5. Budget work deliberatamente sovradimensionato; deadline globale con una piccola riserva p99 per l’ultimo quantum e la serializzazione.

È diverso dagli esperimenti falliti:

- `m34past`, `yield2` e densità ridistribuivano gli stessi 24.89 M work units; non acquistavano i 2–3 secondi che il piano lascia inutilizzati. Il risultato negativo è netto in [real-interruption/README.md:430](</var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/real-interruption/README.md:430>).
- L’interruzione corrente riguarda solo m34; infatti il wall stop non può fermare le altre classi [real-interruption/README.md:594](</var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/real-interruption/README.md:594>).
- Il resume corrente non è un vero scheduler marginale: dopo esattamente un’altra azione, m34 viene ripreso prima del ranking perché considerato sunk cost [portfolio.rs:6989](</var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/portfolio.rs:6989>).
- A 10 s m34 pubblicava 9/9, ma l’ultima pubblicazione era m22 in 9/9: il prossimo operatore da rendere interrompibile è quindi **m22**, non un’altra variante m34 [coordinator-v4/README.md:362](</var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/coordinator-v4/README.md:362>), [coordinator-v4/README.md:393](</var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/coordinator-v4/README.md:393>).

Non costruirei ancora un bandit o un altro sistema di prior. Prima proverei la semplice traiettoria canonica con preemption globale: è il confronto causalmente pulito.

Vincolo inevitabile: hard wall e documento deterministico sotto carico arbitrario sono incompatibili. Il repository lo riconosce già [real-interruption/README.md:542](</var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/real-interruption/README.md:542>). Per avere entrambi servono core riservati/admission control e un work prefix calibrato; altrimenti devono esistere due contratti espliciti:

- fixed-work: risultato deterministico, wall variabile;
- hard-wall: exact-valid entro deadline, profondità variabile.

## 2. Gap wall–work

| Braccio mixed-61 @10 s | Mediana | Wall |
|---|---:|---:|
| plan riproducibile | 175.388 | mediana ~7.2 s, max 8.28 s |
| wall | 168.484 | mediana ~10 s, max 10.30 s |

Sono **6.904 mm già osservati**, non ipotizzati [real-interruption/README.md:507](</var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/real-interruption/README.md:507>). Quindi:

- Upper bound comprabile tramite utilizzo del wall: 6.9 mm.
- Quantità oggi dimostrata sotto hard deadline: zero; il braccio osservato sfora fino a 300 ms ed è non riproducibile.
- Target realistico del global wall governor: recuperare la maggior parte di quei 6.9 mm e scendere almeno sotto **170.4**, con p95 ≤10 s.
- Conferme da 0.26 ms, FCV allo 0.5% del leaf e ulteriore batching non spiegano millimetri: possono recuperare frazioni di quantum, non cambiare la curva.

Il piano corto è intenzionale: bias `1.70`, headroom e quantizzazione verso il basso sacrificano capacità per stabilità [portfolio.rs:1237](</var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/portfolio.rs:1237>), [portfolio.rs:1280](</var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/portfolio.rs:1280>). Non è più un problema di nanosecondi.

Dopo 168.484, il residuo è strutturale:

- wall → Sparrow: **18.319 mm**;
- 40 M → 120 M compra soltanto **5.964 mm**: 169.891 → 163.927 [coordinator-v4/README.md:405](</var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/coordinator-v4/README.md:405>);
- anche eliminando idealmente tutto il lavoro di crossover e descent che pubblicano zero, restano circa 212 M unità pooled nelle classi produttive, cioè ~70.7 M/seed: ancora molto oltre il volume da 10 s [coordinator-v4/README.md:422](</var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/coordinator-v4/README.md:422>).

Quindi un oracle di scheduling migliore non comprime onestamente la traiettoria 120 M dentro 10 s.

## 3. Qualità per azione

Il verdetto sugli operatori attuali è: **nessun lever esistente è oggi credibile per 150@10 s**.

Il record non dimostra trasferibilità della rotazione o del sub-grid. Dimostra invece composizione condizionata dal contact state:

- 13/18 adozioni sono ingressi rotazionali che riaprono flatten/legalization;
- servono 7.204 arm;
- i guadagni per arm sono 0.0003–0.013 mm;
- non esiste alcuna claim wall-clock [orientation-floor/README.md:118](</var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/orientation-floor/README.md:118>), [orientation-floor/README.md:129](</var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/orientation-floor/README.md:129>), [orientation-floor/README.md:503](</var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/orientation-floor/README.md:503>).

La rotazione sparsa ha già eliminato la tassa ed è rimasta un null: quindi non è il trigger che manca [sparse-rotation/README.md:723](</var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/sparse-rotation/README.md:723>).

L’idea nuova che proverei è un **active-contact block SE(2)**:

- costruire il grafo dei contatti quasi binding che toccano i pezzi depth-setting;
- scegliere un piccolo componente connesso;
- proporre congiuntamente `Δx, Δy, Δθ` e riduzione della frontiera tramite trust-region/convessificazione sequenziale;
- applicare il candidato come blocco, fare una breve legalization m34/m22 e pubblicare soltanto dopo il gate esatto.

Non è m33 riproposto: m33 muove un pezzo e spera che gli operatori successivi compongano l’uscita. Qui rotazione, rilascio laterale e compressione dei partner diventano **una singola azione coerente**. È anche coerente con il vecchio finding che la separazione translation-only è insufficiente e servono mosse simultanee sul contact graph [next-generation-engine-plan.md:297](</var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/next-generation-engine-plan.md:297>), [next-generation-engine-plan.md:303](</var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/next-generation-engine-plan.md:303>).

## Prossime tre spese

1. **M — Global hard-wall governor.** Rendere m22 interrompibile, poi ogni azione capace di superare la riserva. Gate: mixed-61 mediana ≤170.4, p95 ≤10 s, tutti gli output exact-valid. Se fallisce, fermare il lavoro sul coordinatore.

2. **M — Audition m26 resumibile.** Un solo short rung dopo stall congiunto m34+m22, checkpoint soltanto su incumbent exact-valid. È l’unico meccanismo corrente con evidenza di basin break da 4.3–5.0 mm su parent 174–179 [next-generation-engine-plan.md:4364](</var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/next-generation-engine-plan.md:4364>). Se non supera m34/m22 a pari work, tagliarlo definitivamente dal percorso 10 s.

3. **L — Active-contact block SE(2).** Gate prima dell’integrazione: parent from-request 171–179, lavoro uguale, almeno 2/3 seed mossi e miglioramento netto di mm/work; poi A/B 10 s su richieste held-out.

Taglierei ora ogni altro round su `m34past/share/yield/grid`, race/m20, blanket/sparse rotation, witness→m33 standalone, crossover più profondo e ulteriori prior. Il verdetto onesto è: **il wall engineering può probabilmente portare 175→circa169; 169→150 richiede una nuova azione di ricerca, non un altro scheduler.**