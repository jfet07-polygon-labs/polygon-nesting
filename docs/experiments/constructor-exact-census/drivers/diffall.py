#!/usr/bin/env python3
"""`diff.py` over every gate, summarised to one line each.

    python3 diffall.py <aLabel> <aBinary> <bLabel> <bBinary> [gate ...]
"""
import json
import os
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
a_label, a_binary, b_label, b_binary = sys.argv[1:5]
tags = sys.argv[5:] or ['g1', 'g2', 'g3', 'g4']
summary = {}
for tag in tags:
    proc = subprocess.run(
        [sys.executable, f'{HERE}/diff.py', a_label, a_binary, b_label,
         b_binary, tag],
        capture_output=True, text=True, check=False)
    try:
        doc = json.loads(proc.stdout)
    except json.JSONDecodeError:
        summary[tag] = {'error': proc.stderr[-500:]}
        continue
    summary[tag] = {
        'fieldsCompared': doc['fieldsCompared'],
        'fieldsDiffering': doc['fieldsDiffering'],
        'differingFields': [row['field'] for row in doc['diffs']],
    }
    print(json.dumps({tag: summary[tag]}), flush=True)
print(json.dumps(summary, indent=1))
