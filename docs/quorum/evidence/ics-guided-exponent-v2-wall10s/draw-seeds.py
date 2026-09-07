#!/usr/bin/env python3
"""draw-seeds.py <nonce-hex> <n> <registry.txt> [--log draw-log.txt]
The recorded-nonce SHA-256 counter procedure of GPT-6 Astra review 7 (Q17): for counter k = 0, 1, 2, ...
seed_k = the unsigned 64-bit big-endian integer of the first eight bytes of SHA-256(nonce_bytes || k as 8-byte big-endian);
reject a candidate that is in the exclusion registry or already drawn; keep the first n admissible. Prints the seeds and
writes the complete draw log (every candidate, admitted or rejected, with the reason)."""
import sys,hashlib
nonce=bytes.fromhex(sys.argv[1]); n=int(sys.argv[2]); reg=sys.argv[3]
log=sys.argv[sys.argv.index('--log')+1] if '--log' in sys.argv else None
excluded=set(int(l.split()[0]) for l in open(reg) if l.strip() and not l.startswith('#'))
seeds=[]; lines=[]; k=0
while len(seeds)<n:
    h=hashlib.sha256(nonce+k.to_bytes(8,'big')).digest(); s=int.from_bytes(h[:8],'big')
    if s in excluded: why='rejected: in the exclusion registry'
    elif s in seeds: why='rejected: duplicate'
    else: seeds.append(s); why='admitted'
    lines.append(f'{k}\t{s}\t{why}'); k+=1
if log: open(log,'w').write(f'# nonce {nonce.hex()}  n {n}  registry {reg}  counters tried {k}\n'+'\n'.join(lines)+'\n')
print('\n'.join(str(s) for s in seeds))
