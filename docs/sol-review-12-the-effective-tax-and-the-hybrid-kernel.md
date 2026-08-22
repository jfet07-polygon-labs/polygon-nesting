# Sol review 12 — the effective tax and the hybrid kernel (2026-08-22)

Independent post-round review of the merged Gate A + m26 audition evidence
(commits 2b3f0b9/4e41996 on 1ca3315). Run on this machine: Codex
(`codex exec resume --last`, model_reasoning_effort=max, sandbox read-only),
same resumed instance as reviews 3-11. Prompt: the round summary with the
three questions. Reply verbatim below.

---

## Verdetto breve

- Gate A centra il risultato principale: il layout Sparrow è legalmente 5.0/5.0 e l’autorità miter corrente lo esclude. Questo sopravvive alla review.
- Tre formulazioni sono troppo forti e vanno corrette prima del push: “`d−2r*` è esattamente costo del join”, “round verificato sulle stesse righe”, e “arm C falsifica la precondizione di Grok”.
- L’audition chiude m26 come spesa 10s contro m34, ma non ha letteralmente testato ogni prefisso cappato a W.
- Non considero conclusa la battery ancora in esecuzione.

## 1. Gate A

### L’evidenza mostra

Il caso 3 è reale e non dipende da `r*`:

- Il validatore materiale accetta.
- Il vero composito miter rifiuta.
- A raggio 2.5, ogni coppia ha distanza materiale ≥5.0 e ogni bordo ≥5.0, quindi il Minkowski continuo con disco di raggio 2.5 è ammissibile.
- Il miter di produzione è effettivamente `JoinType::Miter`, limite 2.0 ([general_polygon.rs:371](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/geometry/general_polygon.rs:371)); Clipper usa il miter finché rispetta il limite e poi passa a square ([offset.rs:883](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/clipper/offset.rs:883)).

Non trovo contraddizioni nei conteggi: 37/4, 33/3, 31/2 miter e 2/0, 1/0, 0/0 round concordano fra `verdicts.json`, `summary.json` e README.

### Difetto 1 — `d−2r*` non è un’attribuzione causale esatta

`d−2r*` è un buon **effective clearance tax** della pipeline discreta. Non isola matematicamente il solo join, perché confronta:

- `d`: source-ring trasformati in `f64`;
- `r*`: trasformazione canonicalizzata, offset Clipper e seconda quantizzazione dell’output.

Il JSON stesso mostra costi negativi fino a −0.00140 mm ([summary.json:267](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/gate-a-sparrow-import/evidence/summary.json:267)), impossibili se fosse un’identità esatta “miter contiene il disco del source”. Non intacca mediana 0.5057 o massimo 2.3343, ma invalida “exactly” e “join alone” al micron.

Inoltre la bisezione presume monotonia dell’offset Clipper discretizzato ([import_gate.rs:387](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/import_gate.rs:387)); i test verificano l’algoritmo su un oracolo monotono, non la monotonia della geometria rasterizzata.

Correzione: rinominare il campo/documentazione in “effective miter tax, quantized”, con intervallo d’errore, e verificare almeno una finestra esaustiva attorno a ogni `r*`.

La causa del rifiuto va attribuita col controfattuale allo stesso raggio — o ancora meglio direttamente con `d < 2r` — non col valore di `d−2r*`.

### Difetto 2 — il budget 0.003614 mm manca un rounding

La derivazione conta una sola quantizzazione bidirezionale ([summarize.py:25](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/gate-a-sparrow-import/drivers/summarize.py:25)). Il percorso ne ha due:

1. `transformed()` ricostruisce il `PolygonSet` canonico;
2. `do_round()` arrotonda nuovamente i vertici dell’offset ([offset.rs:796](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/clipper/offset.rs:796)).

Un budget conservativo semplice è quindi circa:

`2.000 + 1.414 + 1.414 + 0.200 = 5.028 µm`.

L’osservato massimo 2.226 µm passa ancora. Cambia la prova, non il verdetto. Rende però ancora più chiaro che la coppia 38·39, con 0.42 µm di margine sul raggio, non è accettabile da un envelope outward discretizzato a 1 µm senza fallback analitico.

### Difetto 3 — 16 righe round non sono state misurate

A r=2.5, 16 delle 31 failure miter hanno:

```json
"roundAtSameRadiusOverlaps": null,
"causedBy": "join shape"
```

Il driver interpreta l’assenza come accettazione ([failures.py:60](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/gate-a-sparrow-import/drivers/failures.py:60)). L’inferenza è logicamente valida perché il census aggiunge ogni failure round alla popolazione ([import_gate.rs:593](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/import_gate.rs:593)); ma `null` non è una misura negativa.

Correzione: scrivere `roundFailure=false`, più `roundRowBisected=false`, oppure bisecare l’unione delle righe miter/round. Finché non lo fate, toglierei “round deviation sulle stesse righe”.

### Boundary k

La conclusione strutturale è corretta: con limite 2.0 il reach può arrivare a circa `2r`, quindi il ceiling 7.504 mm è coerente, salvo ±griglia.

Il `k=(clearance−inset)/b*` è però un **fattore effettivo osservato**, non necessariamente il `1/sin(half-angle)` del medesimo corner: il vertice binding può cambiare col raggio. Le escursioni misurate direttamente sostengono comunque ~5.502 e ~5.560 mm. Rinominerei `miterReachFactor` in `effectiveReachFactor`.

### Lower bound

L’aritmetica contract-native è corretta, condizionata alla somma certificata:

`249773.80485530035 / 1995 + 5 = 130.19990218310795`.

La derivazione è quella giusta ([mixed61-lower-bound-exact-clearance.py:42](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/depth-lower-bound/mixed61-lower-bound-exact-clearance.py:42)). Anche 124.887 è correttamente respinto come bound rafforzato.

L’identity check a 0.0 mm² dimostra soltanto che è stata riutilizzata la stessa somma numerica ([repin-evidence.py:64](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/depth-lower-bound/repin-evidence.py:64)). Non certifica:

- correttezza delle contribuzioni per shape;
- directed rounding del raster NumPy;
- contenimento del disco source nel miter canonicalizzato.

Quindi: 130.199902 è aritmeticamente difendibile; l’identity check non è da solo una prova. Il composite 130.214 richiede esplicitamente un termine per la canonicalizzazione, oppure va etichettato “numerical bound”.

## 2. Audition m26

### L’evidenza mostra

La decisione pratica CUT è robusta:

- A quasi pari wall, m34 `3341379` fa 1.1044 mm in 3.55 s contro 0.2332 in 3.60 s.
- A work aggregato comparabile, m34 fa 7.0129 contro 0.2332 e consuma meno work complessivo.
- Il ladder completo costa 16.39 s già dal parent warm e perde ancora ([README.md:149](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/m26-band-audition/README.md:149)).

Questo basta per lasciare m26 fuori dalla produzione 10s.

### Correzioni necessarie

1. **Non è CUT sotto entrambe le letture a tutti i budget.** Al budget minimo `survivesWeak=true` ([verdict.json:291](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/m26-band-audition/evidence/verdict.json:291)). La frase corretta è:

   - strict rule: CUT 5/5;
   - weak rule: CUT 4/5;
   - controllo designato: CUT sotto entrambe.

2. **“Work-matched” significa median/aggregate-matched, non per-parent.** Il rapporto control/arm per cella varia circa 0.27–2.22. `0/12` è dunque “contro la configurazione il cui work mediano è matched”, non dodici duelli equal-work. La dominance aggregata resta enorme.

3. **La curva non risolve letteralmente ogni W.** Il braccio non ha un cap: un rung costa qui 4.09 M mediani, non i 33.4 M dell’anatomia. Quindi un vero `drop1.0` cappato a 15–35 M avrebbe potuto eseguire un prefisso di 2–5 rungs, non il solo primo. Sono stati misurati gli estremi 1 e 6, non quei prefissi.

   Non spenderei un altro round: m34 domina già a pari wall e il rendimento m26 scende. Ma la chiusura corretta è “executed one-rung and six-rung variants lose”, non “ogni cap è stato provato”.

4. **Arm C non falsifica Grok.** Grok disse che il guadagno esisteva su un parent saturo disponibile dopo ~16 s ([grok-review-5:29](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/grok-review-5-stop-and-consolidate.md:29)). L’audition usa quei parent già forniti e il ladder completo impiega 16.39 s: conferma precisamente la precondizione economica.

   Falsifica soltanto “era un artefatto del contesto in-process”, affermazione che Grok non aveva fatto. La conclusione CUT non cambia.

## 3. Cosa Gate A autorizza

### Ordine raccomandato

1. **Decisione prodotto: miter immutabile oppure no.**

   Gate A autorizza questa escalation adesso: l’autorità composita impone clearance pose-dependent superiore al contratto dichiarato. Se il miter è compatibilità immutabile, la risposta è stop; il target Sparrow non appartiene al legal set pubblicabile.

2. **Certified round kernel, solo se il miter è implementazione sostituibile.**

   Non promuovere il round Clipper attuale: produce 20.6k vertici contro 377, circa 55× ([summary.json:352](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/gate-a-sparrow-import/evidence/summary.json:352)), ed è inscribed.

   Serve un ibrido: broad phase economica + arco/segment-distance analitico nei casi marginali. Kill:

   - zero false accept sulle tre popolazioni e sui casi ±1 µm;
   - zero regressioni sul corpus canonical-valid;
   - costo build/query ≤1.25× legacy;
   - equal-operator-wall: ≥8/12 vittorie e ≥1 mm mediano contro miter.

3. **Off-lattice soltanto dopo il pass del kernel.**

   Le 57/61 pose provano inesprimibilità di quel layout, non che ogni layout da 150 mm richieda quegli angoli. Blanket e sparse rotation restano negativi sotto l’autorità attuale. Dopo un kernel round valido, riuserei gli strumenti esistenti in A/B; non aprirei una nuova famiglia.

   Kill: nessun aumento di pubblicazioni discendenti oppure <8/12 vittorie equal-wall.

4. **Stop** se il miter è immutabile o uno dei due gate fallisce.

### Per muovere davvero la curva 10s

Servono contemporaneamente:

- **Legality:** autorità round/material sound, non il shadow inscribed.
- **Reachability:** proposte off-grid o un layout equivalente raggiungibile; legalizzare la posa Sparrow non la fa apparire.
- **Economics:** nessun offset da 20k vertici nel candidate loop e nessuna ricostruzione per angolo.
- **End-to-end:** da richiesta nuda, exact-valid, paired 3/10/30 s. Un guadagno locale di 0.5–1 mm promuove soltanto lo strumento; per Sparrow servono ancora circa 18 mm rispetto alla wall line ~168.5.

In sintesi: Gate A apre legittimamente una domanda di rappresentazione. Non apre ancora una roadmap prestazionale. Prima del push correggerei i quattro overclaim documentali sopra; non vedo segnali di manomissione, ma vedo normale entusiasmo causale oltre ciò che i dati misurano.
