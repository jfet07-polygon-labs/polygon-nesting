# Sol review 11 — the Certified Round-Envelope Kernel, or stop

> Verbatim final reply (resumed codex session, max effort, read-only) at HEAD 5ccab1c, 2026-08-22.

Verdetto onesto: dentro l’attuale contratto composito — clearance materiale **AND** envelope canonico miterizzato — non vedo una nuova direzione evidence-qualified. Proporne una sarebbe rinominare un lever già bocciato.

L’unica direzione concreta che merita un round è esterna allo spazio operatori: sostituire, su un percorso sperimentale separato, l’envelope canonico miterizzato con un’autorità di collisione che rappresenti esattamente \(P \oplus \mathrm{disc}(2.5)\).

### Direzione: Certified Round-Envelope Kernel

Meccanismo:

- Envelope con join circolari analitici — segmenti e archi — oppure distanza robusta diretta come fallback per fori/topologie difficili.
- Discretizzazione soltanto outward, con errore formalmente incluso nel margine.
- Trasformazione rigida dell’envelope precomputato: niente nuovo offset Clipper per ogni angolo, perché rotazione e somma con un disco commutano.
- L’attuale validatore indipendente sui source-ring resta l’ultima autorità 5.0/5.0.
- Trait opt-in; percorso canonico, golden e legacy intatti.

È un cambio di rappresentazione/contratto interno, non un nuovo scheduler.

Perché è l’unica fessura rimasta:

- Il contact-block materiale concedeva 0.506 mm, mentre il gate composito ne lasciava 0.044: la perdita è nella rappresentazione canonica, non nel solver ([contact-block/README.md:149](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/contact-block/README.md:149), [contact-block/README.md:231](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/contact-block/README.md:231)).
- Il codice documenta già che il miter envelope può collidere quando la clearance materiale è sufficiente ([general_micro_legalization.rs:20](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/general_micro_legalization.rs:20)).
- Questo elimina specificamente gli spigoli miter e la griglia canonica che respingono i passi SE(2); non “linearizza meglio” lo stesso modello sbagliato.
- Non ripropongo NFP event-driven: blanket, escalation locale e union-NFP sono già falliti ([next-generation-engine-plan.md:155](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/next-generation-engine-plan.md:155), [next-generation-engine-plan.md:203](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/next-generation-engine-plan.md:203)).
- Non ripropongo PGS/PBD: micro e global legalization lo implementano già ([general_micro_legalization.rs:43](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/general_micro_legalization.rs:43), [general_micro_legalization.rs:96](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/crates/polygon-nesting-core/src/search/general_micro_legalization.rs:96)).

### Gate da un round

Userei il contact-block solo come corpus diagnostico, non come operatore candidato.

1. Costruire il round-envelope shadow e confrontarlo con il validatore source-ring su:

   - corpus canonico esistente;
   - proposte material-valid/canonical-invalid già committate;
   - sweep al confine ±1 µm.

   Gate di soundness: zero false accept; ogni layout canonical-valid deve restare valido.

2. Sui medesimi 12 parent: `round-envelope → m34 invariato`, confrontato col m34 canonico a pari operator-wall, non con gli attuali work units.

3. Promozione soltanto se:

   - almeno 8/12 vittorie appaiate;
   - efficienza mm/s superiore a m34;
   - overhead ≤1.25×;
   - ogni pubblicazione passa il validatore materiale intatto.

Al primo mismatch di soundness, oppure se le nuove ammissioni restano attorno al ceiling già osservato di 0.506 mm contro gli 1.104 mm della slice m34, si chiude senza seconda campagna ([contact-block/README.md:188](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/experiments/contact-block/README.md:188)).

Se l’envelope canonico miterizzato è parte immutabile del contratto, allora la risposta è: **fermarsi e consolidare**. Il repository stesso conclude che lo spazio operatori corrente è esaurito ([shipped-surface.md:211](/var/lib/t3/worktrees/polygon-nesting/t3code-ae6e3e8a/docs/shipped-surface.md:211)). Tre volte l’hardware/lavoro comprerebbero circa i 5.96 mm osservati, non i ~25 mm necessari per Sparrow; neppure quello, da solo, riapre credibilmente 150@10s.