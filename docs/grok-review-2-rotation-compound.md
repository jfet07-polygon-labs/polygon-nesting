# Grok review 2 — rotation round, validator, and the compound plan's risks

> Verbatim reply from the local Grok CLI (xhigh effort, read-only) at HEAD a2fd148, 2026-08-21.

Leggo i DIFF e l'evidenza committata sui due round mergiati e sul piano composto, senza fidarmi dei riassunti.I due merge ci sono. Ora i tree degli esperimenti, i DIFF dei commit e il piano in volo.I due README e il piano composto sono i documenti da leggere per primi; i DIFF del Rust e i JSON di evidenza seguono.I README li ho. Ora il codice del broad-phase, il piano composto e i JSON di evidenza — i numeri vanno verificati lì, non nel testo.Ora il corpo di `ClearanceSlabs`, il capitolo del piano sul round in volo e il codice di rotazione continua.Il piano composto è “già in volo”: lo cerco nei checkpoint, nei worktree e nei rami, non solo nel README.I worktree del round in volo e il codice della cache/NFP: da lì escono i rischi del piano composto, non dal README.Devo verificare i buchi di soundness (fori, inner rings) e la tassa di risoluzione nel codice, non nei commenti.Numeri e codice coincidono su alcune claim e le smentiscono su altre. Sotto, solo quello che i DIFF e i JSON committati sostengono.# 1. `fast-contract-validator` (c4b57db)

**I numeri del headline tengono.** Census 5 934 690 coppie, skip 0.9602 (`census.json:139-143`). Wall serial 5.574×, 110/110, walk uguale su 6 campi (`wall-serial.json:4181-4229`). Anytime 172.288→169.572, 30→49 slice, 9/9 paired (`anytime-10s.json:365-407`). Gate digest identici HEAD/off/on (`gates.json:4-7`, `gates-head-vs-on.json:4-7`).

## Soundness — containment
- Lo skip è solo `gap >= threshold` su 4 assi (`general_polygon.rs:769-778`). Containment ⇒ intervalli nidificati in ogni direzione ⇒ nessun skip. Test con distanza esatta 4 mm e overlap vero (`:1443-1478`).
- L’argomento «sample interno ⇒ dentro la propria slab ⇒ fuori dall’altra» (`:753-767`) vale per l’hull, quindi anche per non-convessi. **Non è un falso positivo.**
- **Buco:** `ClearanceSlabs::of` proietta anche i fori (`:641-642` + `region_rings:952-953`). Un pezzo *nel foro* (legale, distanza piccola) ha slab nidificate ⇒ no-skip. Conservativo, ma **nessun fixture a fori**. Il sweep `a_proved_clear_pair_is_one_the_exact_loop_accepts` è quadrati 2 mm (`:1329-1379`). `the_broad_phase_changes_no_verdict` (`:1488-1532`) **non** spegne il filtro: è expected_ok sotto `cfg`, non un A/B.

## Soundness — `!is_finite`
- `minimum` parte da `INFINITY` (`:838-862`); zero segmenti ⇒ reject. `of` torna `None` se zero punti o proiezioni non finite (`:639-673`); `provably_clear` su `None` è false (`:770-772`). **Strutturalmente non si skippa il reject.** Belt-and-braces con `transform_source_ring:300-305`.
- NaN: `>=` è false (`:750-751`). OK.

## Soundness — caso (b)
- Un solo call site di produzione: `scan_row:181`. `measure_approach` è **un’altra funzione** (`general_micro_legalization.rs:1827-1859`), non toccata. Case (b) è davvero fuori.
- Il valore non entra in `raw_source_long_axis_depth_mm` (`general_polygon.rs:326-349`, solo outer rings). I pin 1-ULP sono fuori raggio.

## Soundness — margine
- Claim: errore ≤ ~`1.1e-16 * extent`, margine `1e-9 + 1e-12*extent` (`:698-722`). Direzione pericolosa = slab troppo larga (x+y arrotondato) ⇒ gap sovrastimato ⇒ skip falso. A extent di lastra (~200–400 mm) il bound copre. `hypot` in `segment_distance:865-878` è ~1 ulp del risultato; a clearance campagna 5.0 mm (`smoke-*.json:48`) c’è margine enorme. Il caso stretto 0.0005 mm è **non misurato**.
- Coppie esattamente a clearance non skippate (`:1403-1419`). OK.

## Equivalenza — buchi
- I 4 gate sono stream pinnati, **non** l’anytime wall. L’anytime **deve** divergere (più slice).
- Leaf-diff: ≤2 foglie (`confirmationMs`, `repairMs`) — le stesse 2 tra due processi dello stesso binario (`determinism.json:25-31`, README §4.2). Non è un buco di semantica.
- **3 seed, non 9.** 8/9 (seed, arm) ripetono la profondità; seed 0 on varia 169.572/168.756 (`anytime-10s.json:41-53` vs `:136-148`). «9/9» è 3 risultati ripetuti, come il README ammette (`README.md:261-267`).
- **Seed 2: slice 2→2, +0.601 mm** (`anytime-10s.json:410-428`, coppie 0). Il meccanismo «più slice m34» **non** spiega 3/9 celle. `operatorCalls` 8→9, `publications` 4→5: il millimetro extra è un’azione non-m34. Loose end dichiarato, non chiuso.
- Costshare è profilato, profondità diverse dall’anytime (seed 0 on = 172.288, non 169.572) (`costshare.json:41-61`). 3.914%→0.496% e 236→330 sono **mediane di 3 run profilate**, non il battery.
- shapes-17 / triangle-20 **non eseguiti**. Skip 96% è mixed-61 @ 171–179 mm, padding 5 mm. A 155 mm la densità sale, lo skip scende; degrado graceful, win più piccolo, non unsound.
- `debug_assert` sullo skip (`:155-173`) è **fuori in release**. Il census da 5.9 M coppie non ricalcola il loop esatto.
- VOLATILE: il primo run determinism è fallito su **ogni** cella incluso flag-off. Non è un finding nuovo; è `m34-wall-price/gatelib.py` pre-riparazione. N copie della lista.

## Promozione
- Feature off, no spec key (`Cargo.toml:375`). Il 169.572 vive solo nel binario di misura.
- Promuovere a default cambia l’anytime (direzione giusta) e **non** i gate work-budget. Il rischio non è il documento, è un false skip in release su geometria non coperta (fori, contatto a 0.0005, 155 mm).
- Inner `O(E₁·E₂)` sui ~73 sopravvissuti lasciato a posta: 0.86 ms di uno slice da 826 ms (`README.md:409-416`). Headroom residuo irrilevante rispetto a Sparrow.
- Wall 110/110 è 120−10: seed 3 ha 0 confirmation (`census.json:40-47`). Corretto, non 110 genitori.

---

# 2. `continuous-rotation` (d7c33b5)

**Wall negativo e meccanismo positivo coincidono con l’evidenza.** mixed-61 10 s +3.721 mm, 0/9 (`curves-summary.json:148-165`). 655 477 iter, acceptance 0.083, loss share 0.560, 11 slice / 8 published (`:197-222`). Equal-work +0.0046 mm, 6/6 (`workgate-band.json:2738-2745`). Deep 0.000 su 2/2, refused=0 (`workgate-deep.json:468-494`).

## La decomposizione della tassa **non regge come scritta**

README §3: 0.87→1.94 s, «solo un sesto è i build (0.32 s)», il resto è resolution tax. I totali 26.079 s/30 e 21.388 s/11 (`curves-summary.json:185-214`) **danno** 0.869 e 1.944. Ma è una **media su slice eterogenee**, e lo smoke lo mostra.

Slice produttiva, stessi 1616 step, entrambe published:

| | base (`smoke-base.json:722-786`) | crot (`smoke-crot-fine-pass.json:662-726`) |
|---|---:|---:|
| wall | 0.890 s | 1.141 s (**+0.251 s, non +1.07**) |
| `repairMs` | 562 | 786 |
| `confirmationMs` | 273 (287 conf) | 290 (307 conf) |
| `rotationSurrogateBuildMs` | 0 | 173 |

I 173 ms di build stanno **dentro** `repairMs`. Extra non-build repair ≈ 51 ms. Extra conferma ≈ 17 ms. Sulla slice comparabile la tassa è **~1.28×**, non 2.2×, e i build sono la voce grande.

La seconda slice armata (`smoke-crot-fine-pass.json:774-843`): **3.133 s, 0 confirmation, 1574 skippedInfeasible, `repairMs` 3073, build 530 ms**. È questa che tira la media a 1.94 s. Non è «due BTreeMap get sul hot path». È uno slice che cammina 1577 step, il proxy accetta, l’esatto rifiuta, e non pubblica.

«Cinque sesti = resolution tax» conta il baseline 0.87 s *dentro* l’1.94 e mescola slice feconde con slice sterili. **Il 1.07 s extra medio non è un costo per lookup.**

Altri numeri del README **contraddetti dall’evidenza committata:**
- Build/iter: README 0.73; evidence **1.299** (`curves-summary.json:220`). 851 391/655 477.
- Cache hit: 7 196 709 hit / 9 run = **~0.80 M/run**, non 1.2 M (`:207-208`).
- «8.3% delle iterazioni migliorano»: numeratore = rungsImproved+togglesImproved, denominatore = proposals/2 (`summarize.py:63-68`). È 2p, non P(almeno un miglioramento). Per-proposal: 54 462/1 310 954 = **4.16%**.
- 46/61 off-grid: **un** run (seed 0). Seed 1 base è **0/61** (`offgrid-mixed61.json:20-26`); seed 1 crot 40; seed 0 base già 27. `maxOffGridDeltaDeg` ≤ 1.22° — missano il catalogo 2.5° per bit esatti, non per angoli selvaggi.

## I due difetti sono no-op **solo vs il binario del battery**, non in sé

**Residency vs `MAX_CELLS_PER_JOB` cumulativo** (`general_relaxed.rs:15883-15920`, `prepare_continuous_candidate:13863-13872`).
- `rotationBuildsRefused == 0` ovunque nel battery (`curves-summary.json:211`) e nel workgate (`workgate-band.json:2764`). Il path refused **non è stato preso** sulle celle misurate.
- Non è no-op del *difetto*: senza lo scratch counter il cap 524 288 scatta. Deep: 1 248 717 e 1 553 934 celle generate (`workgate-deep.json:204,431`). Slice anytime 2: 356 643 (`smoke-crot-fine-pass.json:830`). Il battery **esiste perché** la fix c’è. Due comportamenti diversi se mai bindasse: `prepare` propone l’incumbent (`:13863-13872`), `ensure_rotation_surrogate` **alza** `missing_orientation` (`:15917-15919`).

**Mirror companion ∩ `allow_rotation`** (`:13591-13604`, test `:20319-20378`).
- mixed-61: 61/61 `allowRotation: true`, `allowMirror: true` (fixture, zero `allowMirror: false`). Sulle tre request della campagna è no-op. **Non** è no-op in promozione: `allowRotation: false` + `allowMirror: true` falliva lo slice intero a publication. Il test è stato verificato contro il bug (`README.md:469-471`).

Workgate è `final-meas` (`workgate-band.json:2-3`); anytime è `new-meas` (`curves-summary.json:4`). Stesso SHA non dimostrato. Se `allow_rotation` è universale, i numeri coincidono.

## Il negativo è **del costo, e di un costo che il meccanismo stesso crea**

- Equal work: +0.005 mm, coda −1.681 mm (`workgate-band.json:2738-2744`). I rungs valgono i candidati.
- Equal wall: 0/9. Base 9 call, discesa fino a 172.29; armed 7 call, stall a 175.22 (`README.md:249-256`, smoke).
- **11 slice, 8 published** (`curves-summary.json:212-213`): 3 slice armate non pubblicano. La 3.13 s è il tipo. Il meccanismo accetta rungs (proxy), poi lo schedule brucia secondi su layout che l’esatto non firma.
- Deep: 33% acceptance (stessa formula gonfiata), 0 publication, `confirmationsAccepted: 0` (`workgate-deep.json:31-32`). Proxy loss ≠ depth, forma forte.
- Workgate `pconfirm=0` (`workgate.py:40`); anytime `pconfirm=1`. Equal-work non è lo stesso regime di conferma.
- Workgate 1.17× process work units (`workgate-band.json:2747-2749`): il cap è sulle candidate query; exact pair test extra fuori cap.
- Assi: unarmed triangle-poles = 4 (H/V/diag); armed = 6 (`random_coordinate_axis:14318-14350`). `can_refine_rotation` è DynamicPoles, mutualmente esclusivo (`:13582-13587`). 1/6 delle probe è rotazione **in più** rispetto al lane unarmed, non «invece di» una ladder legacy.

`pair_collides` metodo **non** è stato patchato a `resolve_surrogate` (`:15546-15550`); lo scan caldo sì (`scan_fixed_neighbors:16498`). `ENABLE_NFP_AXIS_MINIMIZER = false` (`:101`). Il pair-NFP **non è sul path di design A**.

---

# 3. Piano composto in volo — rischi (non gate-arlo)

Il piano è il §6 crot (`README.md:481-489`) + ribattere l’anytime con fcv in entrambi i bracci. Worktree `wf_08a442a7-1aa-1` è locked su `a2fd148`, working tree pulito: il round parte da qui.

**(a) Indicizzare la deque 48.**
- Hit reali ~0.80 M/run 10 s, non 1.2 M (`curves-summary.json:207`). `touch_rotation_probe` / `acquire_rotation_key` scansionano `VecDeque` (`:16022-16063`). 0.8 M × ~24 confronti ≈ decine di ms, **non** 1.07 s.
- La struttura calda è `BTreeMap` catalogo **poi** overflow (`resolve_surrogate:3785-3793`, chiamato per ogni vicino off-grid in `scan_fixed_neighbors:16498`). (a) indicizza la struttura sbagliata.
- Rischio: Hash+ordine LRU che cambia l’eviction ⇒ determinismo. Rischio vero: spendere il giro su (a) e non misurare (b).

**(b) «La vera tassa è il pair-NFP su angoli continui» — ipotesi quasi certamente falsa.**
- `PairNfpKey = (class, i64, bool)×2` (`:3768`). `pair_nfp_component_count` / `build_pair_nfp_value` leggono **solo** `catalog.orientations` (`:15391-15401`, `:16818-16829`). Overflow invisibile.
- Design A arma solo `RollbackTriangle + StructuredTrianglePoles` (`continuous_rotation_lane:2698-2702`). Directional (l’unico che *usa* pair-NFP) è rifiutato. Axis minimizer spento (`:101`).
- Se unissero crot + directional, lo slice morirebbe con `"missing fixed NFP surrogate"` — bomba latente, non la tassa misurata.
- Lo smoke dice dove vanno i millisecondi **senza** ipotesi: slice feconda +0.25 s (build 173 ms); slice sterile 3.13 s / 0 conf / 1574 infeasible. Misurare prima è giusto; partire da pair-NFP è partire dal morto.
- Cosa misurare, in ordine: `repairMs` vs `confirmationMs` vs `rotationSurrogateBuildMs` per slice (già nei JSON); `ScanCatalogDescents`; `pair_nfp_builds` (dovrebbe restare 0); frazione skippedInfeasible; assi extra (4 vs 6).

**(c) Ribattere 3/10/30 s, fcv+pconfirm **entrambi** i bracci, crot 0 vs 1.**
- Il battery crot è su `92a1a08` **senza** fcv. 175.219 vs 169.572 è cross-round, non un A/B.
- fcv non paga 1.07 s/slice. Sulla slice feconda (307 conf × ~0.9 ms) fcv può togliere ~0.2 s. Sulla slice sterile (0 conf, `smoke-crot-fine-pass.json:790-794`) **fcv è zero**.
- «49 slice invece di 30 cambiano il budget»: 49 è il **controllo** fcv, non l’armato. L’armato aveva 11. Con fcv il base sale a 49; l’armato resta ~11 se le slice sterili restano. Il gap di slice **si allarga**.
- 3 s: 0 slice m34 in entrambi (`curves-summary.json:43-44`). (c) a 3 s non misura rotazione.
- shapes-17 / triangle-20: m34 pubblica 0/9 entrambi i bracci (`README.md:178-182`). (c) ripeterà +0.000 mm pagando wall (10.1→12.9, 16.4→22.3).
- Rischio: batteria 3 request × 3 budget × 9 celle per **riconfermare** un negativo che lo smoke già spiega. Farla **dopo** aver spezzato le slice sterili e i 0.25 s della feconda, non prima.

Altro rischio di composizione: crot anytime **senza** fcv; fcv anytime **senza** crot. Nessuna cella committata ha entrambi. `rotationSurrogateCells: 0` nel summary anytime (`curves-summary.json:210`) — il campo non è plumbato nel battery, solo nel workgate. Strumento cieco sulla residency in (c).

---

# 4. Prossime 3 spese (169.572 vs Sparrow 150.165 = 19.4 mm)

fcv ha già portato il confirmation a 0.50% del leaf (`costshare.json:165-170`). Un altro giro sul validatore non chiude 19 mm. Design A ha il DOF (SE(2) riprodotto in search) e perde l’orologio. Il record di campagna 155.26 è 5 mm dietro Sparrow-10 s, a work che 10 s non compra.

1. **Spezzare il 1.07 s medio per tipo di slice** — feconda vs sterile vs infeasible — con i contatori già nel documento (`repairMs`, `confirmationMs`, `rotationSurrogateBuildMs`, `confirmationsSkippedInfeasible`, `ScanCatalogDescents`). Non indicizzare la deque, non toccare pair-NFP finché `pair_nfp_builds` non esce da zero. Senza questo, (a)/(b)/(c) sparano alla cieca.
2. **Attaccare quello che (1) mostra.** Se è la slice sterile (proxy accetta, esatto rifiuta, 3 s persi): early-exit / acceptance-rate stop / non commettere rungs che alzano skippedInfeasible. Se è il doppio get BTreeMap sui 46/61 off-grid: HashMap overflow o catalogo continuo, non LRU da 48. Se è 173 ms di build sulla feconda: fine-pass already; non si recupera 19 mm lì.
3. **Solo allora** riarma A a wall, fcv+pconfirm **simmetrici**, crot 0 vs 1. Se (2) non taglia le slice sterili, non rifare (c): spendere su B/C (poche rotazioni quando il clamp/SE(2) lo chiede). Un rung accettato mette 46 pezzi off-grid e la tassa di lookup è **per-vicino per il resto dello slice**, non per-rung. B/C pagano la stessa tassa al primo accept se la keying resta overflow-di-griglia.

Non spendere: inner nest del validatore; `lanes=8` (già −2.2 mm); (a) da solo; (c) prima di (1).