# Grok review 3 — FCV promotion audited, sparse-rotation null decomposed, the next 5mm

> Verbatim reply from the local Grok CLI (xhigh, read-only) at HEAD a131a72, 2026-08-21.


---

## 1. FCV (6d5cdbf)

### Catena `transform → sheet → interior_sample`

Il lemma `!is_finite ⇒ nessun segmento` **non segue** da `of() != None`. Lo dicono loro, e il codice lo conferma: `point_segment_distance` ha prodotti e una divisione (`general_polygon.rs:1385-1395`), `f64::min` scarta i NaN, `scan_row` tratta un minimo non finito come reject.

La catena scritta è:

| anello | tiene? |
|---|---|
| `PolygonRing::new` ⇒ \|source\| ≤ (2⁵³−1)/1000 | sì (`general_polygon.rs:90-104`, `offset_policy.rs:27-38`) |
| il bound **non** sopravvive a `translate_*` | sì: `placement_rotation` controlla solo finitezza (`:292-303`); `transform_source_ring` solo il risultato (`:332-336`) |
| `validate_sheet` non chiude | sì: gira dopo, solo outer, `sheet_width` solo finito (`:421-440`) |
| `transform_placement` rifiuta se `interior_sample` è `None` | sì (`:408-414`) |
| due y-level distinti **e** due x-intersezioni ⇒ \|coord\| ≤ 2.29e29 | **buco** |

Buchi nel bound 2.29e29:

1. **Il diametro FP non è il diametro esatto proprio alla scala derivata.** Due double distinti di magnitudine M distano ≥ M·2⁻⁵³. Al ceiling, ulp(2.29e29) ≈ D = 2.55e13. L’argomento «\|y₁−y₂\| ≤ diametro sorgente» è circolare proprio dove deriva il numero (`general_polygon.rs:713-725`, README §7.1 `:504-516`). Il test ricalcola la *formula*, non un layout contrattuale worst-case (`:1955-1972`).
2. **Le x-intersezioni non sono vertici.** `parameter = (scan_y−start.y)/(end.y−start.y)` (`:504-505`) non è dimostrato in (0,1) in FP; la differenza delle intersezioni interpolate non è ≤ D. L’argomento x poggia su questo anello, non sul diametro dei vertici.
3. **Ordine di quantificazione.** Il bound y chiude i *vertici*. Il bound x delle *intersezioni* dovrebbe chiudere i vertici a x grande (collasso sotto ulp). È un argomento di scala, non una disuguaglianza chiusa su tutti gli intermedi.

Cosa *tiene* lo stesso: a 2¹¹² l’ulp è ~1e18, il diametro sorgente è 2.55e13. Un pezzo contrattuale collassa a un solo double molto prima del guard. Il guard è fail-closed (`of` → `None` → exact loop, `:847-856`, `:1103-1106`). Completeness dello skip (non rifiutare layout ammessi) poggia sul margine 2.3e4, non sulla chiusura del lemma. Soundness dello skip **non** poggia su 2.29e29: poggia sul guard che prova finitezza *da solo* a 2¹¹² (`:734-764`).

`orient2d` al guard: splitter 2²⁷, diff ≤ 2¹¹³, intermedi ≪ overflow. Il crate `robust` è usato per il segno (`predicates.rs:95-100`); l’esattezza del segno sul dominio è load-bearing per la metà overlap (`:986-994`). Tiene.

### Derivazione `16.5 · C · u`

La *struttura* è sana: p clippato in [0,1] ⇒ Q = S+p(E−S) è un punto reale *sul* segmento ⇒ `true_d ≤ |P−Q|` ⇒ serve un lower bound sul `hypot` calcolato (`:973-984`). Non serve l’errore su p.

Le costanti no:

- `|E−S|` arriva a **2C**, non C. `fl(p·dx)` vs `p·(E.x−S.x)` è ~4Cu, non 2.1Cu (`README.md:573-578`).
- Higham su `closest = fl(S + fl(p·fl(E−S)))`: errore ≈ C·u + 2C·3u = 7Cu, non 5.3. Per-componente dopo la sottrazione ~9Cu. Vettore ~12.7 + hypot 6 ≈ **19Cu**, non 16.5.
- `hypot` è libm, non correctly-rounded 2u (`Cargo.toml:430`).
- Il test asserisce `32 ≥ 16.5` e `shipped ≥ derived` (`:1935-1948`). Non verifica l’algebra.

32u·extent vs ~19Cu: se extent ≥ C c’è slack. 1e-12 è **281×** 3.55e-15 (`:656-670`, `:592-596`). Non è blocking per la promozione. È una derivazione con slack, non un bound stretto. `max(shipped, derived)` (`:1031`) rende strutturale il *floor*, non la correttezza di 16.5.

Hot loop: `gap` è due sottrazioni e un `max` (`:926-928`); `next_up` è sulla soglia (`:1043-1045`). `next_down(g) ≥ t ⇔ g ≥ next_up(t)` per finiti. Byte-identico al 5.57×: credibile.

Rounding outward solo su `x+y` / `x−y`, non su x,y (esatti da storage): corretto (`:857-860`).

### Witness

`the_numeric_domain_guard_fails_closed_where_the_lemma_does_not_hold` (`:1988-2029`) è un test **sintetico**: due strip collineari a ±1.3e308, `material_sample` inchiodato a mano, `of` chiamato diretto. Non passa da `transform_placement`.

- Inversione: `far − (−far) = +inf` (`:2013-2021`) — non ricostruisce il vecchio `of()`; inlinea il gap. Per *questa* geometria coincide.
- `interior_sample` su tre punti collineari a x costante: tutte le intersezioni coincidono, `windows(2)` vuoto (`:489-518`) → `None`. Irraggiungibile **anche a x piccolo**.
- Quindi: dimostra che il lemma *come enunciato* è falso, e che il guard fail-close. **Non** dimostra l’irraggiungibilità; quella è §7.1, e ha i buchi sopra.

Valido come P0 di Sol. Non è un controesempio di produzione.

### Shadow / census / fattoriale

Numeri committati tengono:

- Shadow: 0 mismatch su 1 051 980 layout / 1 695 677 coppie / 1 002 726 certified, tightest 2.138e-9 mm (`shadow-corpus.json:13-31`). Corpus sintetico (buchi, multi-region, near-threshold assiale). Complementa, non sostituisce, il census campagna.
- Census 5.93M bit-identico: 3 243 / 5 934 690 / 5 698 534 / 0.9602075255826337 (`README.md:712-715`). Il guard non ha mosso una decisione. Skip 96.47% a 155 mm (`census-density.json:16`) — predizione di densità refutata, direzione giusta.
- Factorial *micro*: 0.7952 → 0.2774 ms, 2.87× (`factorial-10s.json:949,973`). Tassa di dispatch refutata.
- Factorial *depth*: fcv solo **+3.122 mm, 9/0/0** ma tre delta unici ripetuti (`:981-997`). `fcv→both` **+1.527, 5/3/1** (`:1035-1050`).
- Le due batterie: pconfirm=0 identico al millimetro tra box; pconfirm=1 si muove 2.4 mm (`README.md:845-848`). Qualificazione core: vera.
- **Buco non scritto in §12:** la cella vincente `fcvon-pconfirm1` ha `medianProcessWallSeconds: 10.508` (`factorial-10s.json:977`) — unica mediana > 10 s sulla box quieta. Sulla box carica la stessa cella sta a 9.92 s e il contrasto è +0.000. Parte del +1.527 mm è **overrun**, non parallelismo. `withinArmSpreadMm: 8.62` sulla stessa cella (`:972`) > effetto. §12 non menziona l’overrun.

### §13 basta per armare fcv+pconfirm come default *dentro v3*?

**FCV: sì, con un lever. pconfirm: no.**

- I cinque P0 di Sol review 7 sono chiusi in direzione fail-closed (guard, outward+bound, shadow release, coverage con due assenze oneste, fattoriale).
- Il fattoriale è *già* `v3=1` (`factorial-10s.json:50`). I +3.1 mm seriali sono un risultato v3.
- FCV **non ha spec key** (`Cargo.toml:375`, `portfolio.rs` default). Non è scoped a v3. Compilarlo nel binario che sa fare `v3=0` lo accende anche lì. §13.2.1 lo chiede e non lo consegna (`README.md:913-917`).
- `m34pconfirm` default è **false** anche con v3 (`portfolio.rs:1711`). I 168.5 mm sono spec esplicita `m34pconfirm=1`, non un default v3.
- Tre seed, un box, tre fixture in cui il validatore non è mai chiamato (`README.md:743-752`). Per un filtro fail-closed va bene; per un default di scheduling no.
- Armare pconfirm come default v3 su wall 10 s, con overrun 10.51 s non dichiarato e dipendenza dai core, è esattamente il millimetro disonesto che `(b)` esiste per impedire.

Raccomandazione: compilare `fast-contract-validator` nel combo v3; lasciare `m34pconfirm` come spec (oggi). Disarm fcv = rebuild, oppure aggiungere la key che §13.2.1 chiede *prima* del flip. v3 resta `v3=1` (`portfolio.rs:1706`).

---

## 2. Rotazione sparsa (0336282)

### Equivarianza miter

In aritmetica esatta `offset(T(p)) = T(offset(p))` per join miter: le due normali ruotano col ring (`general_relaxed.rs:17076-17086`). Sound.

Sul grid Clipper **non** è identità: offset-then-rotate vs rotate-then-offset. Il test lo fissa da entrambi i lati: bounds ≤ 4 unità (osservato 1 = 1 µm), aree < 1e-3 relativi, ring **disuguali** (`:21102-21156`). Fallback permanente se il transform collide (`:4175-4178`). 0 fallback su 1.4M: corpus, non prova.

**36 celle non bastano come prova miter.** Bastano per la licence che si sono dati («se il gate fallisce, tieni per-rung»). 12 parent mixed-61 171–179 mm × 3 round, allowance 0.002 (`armgate.json:3-8`). Nessuno shapes-17 / triangle-20 / 155 mm / spike acuto. `1e-3` relativo sull’area può nascondere un disaccordo di collision. 1 µm vs contratto record 0.0005 mm è irrilevante: il surrogate è proxy di search, la pubblicazione resta `validate_publication` sulle source ring.

−0.040 mm, 27/36 (`README.md:503-504`): traiettoria di search, non fedeltà geometrica. Due spiegazioni (miter sul grid non ruotato più fedele vs luck di 36 celle) **non separate** (`:515-520`). Lo dicono. Giusto.

Il refactor `finish_oriented_surrogate` **non** è feature-gated (`:17116-17120`). I digest identici su quattro binari (`README.md:567-581`) sono il check che conta. Tiene.

### Trigger B

Forma giusta: rotazione solo quando la translation del repair non paga, solo sui pezzi che `collision_pairs` già nomina, scoped allo step (`:6708-6763`, fan-out clone `:6805-6846`). shapes-17 / triangle-20: 0 stall ⇒ 0 surrogate vs 355k / 1.3M di A (`README.md:419-436`). `rotbit` non ha sparato: corretto, zero episodi non è evidenza (`:429-432`).

Due problemi:

1. **Il codice non è il commento.** «Did not lower the loss it was handed» suona come sweep precedente. Il codice confronta col **min storico dello step** incluso l’entry (`:6750-6757`: `if now < stall_loss { disarm } else { arm }; stall_loss = stall_loss.min(now)`). Un repair che recupera da un episodio di rotazione verso, ma non sotto, la loss di entry **resta armato**. Candidato meccanico della perdita a 30 s (`README.md:737-741`: loss share 31.8% → 40.1%).
2. **Non è witness-driven.** Sol 7 §3.2 chiedeva witness/m33. B è stall-on-violating-pairs. C è il witness, e viene dominato (depth finale mossa 0/12, `:277-288`). Accettazione 5.22× dice che i pezzi nominati migliorano in locale; pubblicazione 0 a 10 s e −1.48 a 30 s dice che il locale non è il published depth.

Larghezza 2.6/61: la coda violating non è «il pezzo che dovrebbe ruotare per fare spazio a un terzo». Rischio strutturale, non misurato.

### Null: strumento o meccanismo?

| cella | cosa è |
|---|---|
| 10 s, −0.290 mm, spread within-seed 4.0 mm (`pool-10s.json:11-70`) | **strumento**. Seed 3 base spread 4.67 mm; seed 0 armed 3.65 mm. Il segno non è risolvibile a 6 seed. |
| flag-off 9/9 a work 40M (`reproduce-flagoff.json`, spec `v3=1`) vs 2–5 mm tra sessioni wall | **box**, non codice. Lo dicono in §7.2 e tengono. |
| equal-work `sparseEq−base` −0.077 mm, 24/36 (`README.md:523-529`) | **meccanismo piccolo**, due ordini sotto il gap. Design A riproduce +0.0046 vs +0.005 storico: lo strumento equal-work è stabile. |
| 30 s +1.483 mm, 5/6 seed (`README.md:378-380`) | **meccanismo**. Stesso wall/slice (~1.00×), slice 57 vs 60, depth peggiore. I rung portano la search dove non deve. |

«La tassa è sparita e il gap no» è il finding di meccanismo (slice 1.064× vs 2.12×, acceptance 5.22×, pubblicazione 31/31). Il headline 10 s è un null di strumento. Non contraddicono.

Design C: 1.42 ms, accettato 6–16 volte, depth finale 0/12 perché 1 600 step da 1 µm coprono il trust box (`README.md:315-322`). Dominato per ragione, non per costo. Non riarmare.

---

## 3. Mossa in volo, e i prossimi 5 mm

### `(a)+(b)` è la mossa giusta?

**`(b)` sì, e prima. `(a)` sì solo per FCV, sotto `(b)`, non per pconfirm.**

Il braccio base si muove 2–5 mm tra sessioni wall; flag-off riproduce 9/9 a work (`sparse-rotation/README.md:591-607`). Spread within-seed a 10 s = 4 mm > ogni effetto di questo round. Sol review 4 lo aveva già chiesto (`sol-review-4-portfolio-scheduling.md:160`). Ogni ulteriore spend sulla curva 10 s è illeggibile senza piano-di-lavoro calibrato (probe → work cap sotto p95 wall, determinismo per-seed, overrun onesti).

`(a)` come «default fcv+pconfirm dentro v3»:

- FCV è l’unico millimetro *provato* (+3.1 seriali, 9/0/0, validatore ora 0.496% del leaf). Compilarlo nel combo v3 è corretto. Non è un default engine: `jagua-experimental` non lo include (`Cargo.toml:19`).
- pconfirm=1 **non** è default v3 (`portfolio.rs:1711`). Flipparlo su wall, con overrun 10.51 s sulla cella che «vince» e 8.6 mm di spread, confonde core-availability, jitter e extra-wall. `(b)` è il pretesto per *non* farlo ora.
- Ordine: probe di `(b)` → work budget → flip FCV sotto quel budget (contrasto che viaggia) → pconfirm resta spec finché il probe non dimensiona anche il suo jitter.

Non mischiare i due in un battery wall. È così che è nata la ritrattazione mid-round di §12.1.

### Dove stanno i prossimi 5 mm @ 10 s

168.5 @ 10 s è costruttore + **1–6** schedule action (`factorial-10s.json:951,975`: medianScheduleActions 2 seriali FCV, 6 col combo che overrunna). Rotazione null. Validatore 0.5%. Inner `O(E₁·E₂)` sui ~73 sopravvissuti: qualche percento di uno slice da ~0.8 s (`README.md:409-416`). Nanosecondi no. Sol 7 l’ha predetto; questo round l’ha misurato.

Il record 155 mm è un’altra lineage (clamp sub-grid, ladder, legalize globale, entry di orientazione) a work che 10 s non compra (`record-line-cascade/README.md:52-59`). A 30 s v3 i millimetri sono un crossover derived-cut (#13, 1.7 mm) poi un treno di compression 177→169 (`coordinator-v3/README.md:223-258`). A 10 s v3 vs v2 era 0.000 su 6/9 (`:186-188`). La curva 10 s non sta ancora in quel loop.

### Prossime 3 spese (dopo `(b)` + FCV)

| # | spesa | meccanismo | rischio |
|---|---|---|---|
| **1** | **Densità di conferma del primo m34** (step 0.25 / `confirm_every`, ora che una conferma è 0.28 ms) | Il record line ha comprato 1.0 mm con `step=0.25` a 20M da un parent 159 (`record-line-cascade/README.md:54`). FCV sblocca lo stesso lever *from-request*: più pressione exact per millimetro di clamp, stesso repair (~540 ms). Non è più tassa. | 4× conferme per µm ⇒ meno slice / meno azioni v3 nel cap. Gate: equal-work sullo stesso parent, depth per query non peggiore, overrun onesto. |
| **2** | **Secondo bacino in 10 s** (crossover derived-cut o ladder *dopo* il primo publish m34) | I 5–7 mm di v3 a 30 s non sono «più slice m34 sullo stesso parent»; sono un unlock e un treno su incumbenti nuovi (`coordinator-v3/README.md:249-255`). A 10 s c’è wall per una seconda classe (crossover ~1.9 s, ladder ~2.6 s) se m34 non mangia il resto. | Ranking/prior: `m34wall` ha già costato 0.65 mm a 30 s prezzando la classe troppo cara (`m34-wall-price/README.md:69-80`). Archivio a 10 s troppo magro per un cut utile. Pazienza barren < 8 distrugge #13 (`coordinator-v3/README.md:260-262`). |
| **3** | **Parent del costruttore** (primo exact-valid ~0.54 s @ 231 mm, ≤200 mm @ 0.67 s — `quality-frontier-trace/README.md:76-78`) | Un parent 5 mm più profondo per il primo m34 sposta l’intera curva. Mode 20 è quasi gratis in work units (`opportunity-ledger/README.md:241`) e il work budget lo sotto-prezza. | Sol 3: costruttore peggiore → bacino migliore. `(b)` dimensionato su probe m34 può affamare mode 20. Gate: discendente sotto work fisso, non depth immediata del costruttore. |

Non spendere: rotazione (null @10, contro @30), design C, inner nest del validatore, pair-NFP, `m34lanes`, tassa catalog→overflow come «i 5 mm». Equivariance *nel catalogo*, senza operatore di rotazione, è un taglio di costo per ogni angolo (`sparse-rotation/README.md:742-746`) — S, non i 5 mm.

Il vincolo che lega le tre: senza `(b)`, la (1) e la (2) sono indistinguibili da 4 mm di rumore di box.