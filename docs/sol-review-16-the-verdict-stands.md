# Sol review 16 — the verdict stands (2026-08-23)

Final confirmation review of the pivot round's C175 verdict (merged 791a434),
under Sol 15's own pre-committed reading. Codex resumed instance, max effort,
read-only. Verbatim.

---

Verdetto: **CONFERMO lo STOP pre-committato**, con una correzione di portata: è falsificato il membro Round-1 specificato, non l’intera famiglia matematica overlap-tolerant.

## 1. Il verdetto C175 regge

Le condizioni erano ≥1/3 child exact-valid entro 240k proposal, pivot corretto, jump installato e clearance corretta ([README:39](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/overlap-ics/gate0-pivot-rerun/README.md:39)). Il risultato è inequivoco: 0/3, entry esattamente sul target, zero checkpoint esatti ([README:121](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/overlap-ics/gate0-pivot-rerun/README.md:121)).

Non riaprono il gate:

- **2.223 s:** seed 0 ha ricevuto l’11% di wall in più, non meno; la quota work è rimasta 239,974. Gli altri due seed falliscono sotto 2 s. È una violazione del claim wall, non un falso negativo ([README:637](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/overlap-ics/gate0-pivot-rerun/README.md:637)).

- **Census armato presto:** invalida soltanto la vecchia spiegazione sulle penalità terminali; il census è read-only e non può cambiare 0/3 ([README:334](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/overlap-ics/gate0-pivot-rerun/README.md:334)).

- **11–14/32 direzioni first-order-ascent:** è una debolezza reale del membro. `incident_gradient` usa un witness corrente per ogni max ([energy.rs:260](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/energy.rs:260)); dopo uno switch del massimizzatore quella direzione può salire. Non emerge però un secondo mismatch di coordinate come il pivot.

- **Jump catastrofico:** non salva né contamina il verdetto, perché l’A/B senza jump è comunque 0/3 e converge nella stessa fascia `max_g≈1.60 mm` ([README:211](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/overlap-ics/gate0-pivot-rerun/README.md:211)).

Due overclaim da correggere nel linguaggio, non nel verdetto:

1. È un **attraction band riproducibile**, non ancora un fixed point certificato: mancano una continuazione lunga post-pivot e l’identità delle pose terminali. La vicinanza di Φ e `max_g` non prova lo stesso punto.

2. `piecesSqueezedOnOppositeSides=0` non implica “nessuna mossa mono-pezzo di qualsiasi dimensione” ([README:239](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/overlap-ics/gate0-pivot-rerun/README.md:239)). Il census non contiene connettività, normali e mobilità del grafo di contatto. Prova solo che nessun pezzo viola contemporaneamente top e bottom.

## 2. Cosa è stato falsificato

Precisamente:

> Il membro formato da proposta SE(2) mono-pezzo, unica direzione subgradiente aggregata, ladder e accettazione stretta sull’energia incidente, GLS a una riga e un singolo jump strip/ball non legalizza il 10%-residual shock mixed-61 entro 240k proposal.

Il vincolo decisivo è visibile nell’accettazione `after < before` ([descent.rs:311](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/descent.rs:311), [descent.rs:404](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/descent.rs:404)).

Non direi “basin <O(1 mm)”. La distanza SE(2) di ±0.5 mm/±2° non è 0.5 mm, e una perturbazione casuale non è confrontabile direttamente con una compressione assiale strutturata. La frase supportata è:

> Questo membro recupera S1, ma non una compressione affine one-shot di 6.714 mm; in quest’ultima raggiunge entro quota una fascia ripetibile ancora circa 1.60 mm infeasible.

Chain move, true two-endpoint PGS o bundle multi-witness **potrebbero** attraversarla; non esiste né prova positiva né negativa. Il residuo suggerisce trasmissione attraverso contatti, ma non identifica l’operatore senza prima censire il grafo attivo.

## 3. Ranking

1. **(a) STOP — raccomandato.** È l’unica decisione autorizzata dal gate scritto prima dei risultati. Congelare il prototipo e non implementare `homotopy.rs`, che resta deliberatamente stub ([homotopy.rs:1](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/overlap_ics/homotopy.rs:1)).

2. **(b) Nuovo membro — soltanto come nuovo progetto esplicito, costo M, 3–5 giorni.** Prima scelta: vero PGS row-wise a due endpoint, con proiezione congiunta della coppia e senza veto `after < before` per singolo pezzo. Gate immutato: stesso C175 10%, stessi tre seed, 240k, ≥1/3 child dual-valid; S0/S1/triangle/numeric/throughput restano regression floor. Se fallisce, nessun ulteriore ladder di operatori.

3. **(c) Shock 2–3% — non è la stessa spec.** Il 10% era esplicitamente congelato ([sol-review-14:755](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/sol-review-14-the-overlap-engine-design.md:755)). Rilanciare un C175 più facile sarebbe corpse rescue. Potrebbe diventare un nuovo esperimento solo con gate end-to-end: otto epoch al 3%, ≤960k proposal totali, ≥1/3 seed a ≤168.484 exact-valid. Otto successi ideali porterebbero 182.976→168.457; una singola prima pubblicazione non conterebbe.

**Azione raccomandata all’owner:** chiudere questa linea come `Gate0-stopped`. Se decide consapevolmente di sovrascrivere il kill e finanziare un’ultima nuova ipotesi, scegliere il two-endpoint PGS con C175 invariato—notare che sarebbe un nuovo solver member, non una correzione del risultato.
