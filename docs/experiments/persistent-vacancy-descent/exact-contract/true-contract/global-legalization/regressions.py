#!/usr/bin/env python3
"""The two pinned regression anchors, plus the mode 27/28/29 no-op checks."""

import sys

sys.path.insert(0, '/var/lib/t3/tmp/mode31')
from run import RECORD, run, line, population  # noqa: E402

OUT = '/var/lib/t3/tmp/mode31/regressions'

print(line('mode20-salt320', run('mode20-salt320', 20,
                                 '/var/lib/t3/tmp/ex5-seed-native.json',
                                 '320.000', 0, OUT)))
print(line('mode22-record', run('mode22-record', 22, RECORD, '164.842', 0, OUT)))
