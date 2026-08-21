#!/usr/bin/env python3
"""The controlled competing load, owned by this round rather than borrowed.

    python3 stress.py <workers> [duty] [period_ms]

`docs/experiments/replan/` §7 made every wall claim it had against a load it did
not control: a second measurement campaign happened to be running on the same
host, so "loaded" meant whatever that campaign was doing at that minute. That is
enough to *find* the quiet-box caveat and not enough to *fix* it, because a fix
has to be measured against a load that is the same in every arm.

So this round generates its own, and states it:

* **`workers`** busy processes, each a pure-arithmetic loop on one core with no
  allocation and no I/O, so the contention is for CPU and nothing else. The
  engine is pinned at 8 threads on this 16-core box, so `workers=8` is the load
  that makes the box exactly oversubscribed and `workers=16` is the load that
  makes it 1.5x oversubscribed.
* **`duty`** in `[0, 1]` and **`period_ms`**: each worker runs `duty` of every
  period and sleeps the rest. A steady 100% load is the *easy* case for a rate
  probe - it is a constant, and a constant is calibratable. The case that breaks
  a single reading is a load that is not there when the probe looks and is there
  afterwards, so the default duty cycle is deliberately below 1.

Prints its own PID, then one line per second with the load average, so the
window a battery ran in can be reconstructed from the driver's log alone. Stops
on SIGTERM/SIGINT, and every worker is a child that dies with it.
"""
import multiprocessing
import os
import signal
import sys
import time


def burn(duty, period):
    """One core, `duty` of the time. Pure arithmetic, no allocation."""
    on = period * duty
    off = period - on
    accumulator = 1.000_000_1
    while True:
        deadline = time.monotonic() + on
        while time.monotonic() < deadline:
            for _ in range(4096):
                accumulator = accumulator * 1.000_000_1 + 1e-9
                if accumulator > 1e12:
                    accumulator = 1.000_000_1
        if off > 0:
            time.sleep(off)


def main():
    workers = int(sys.argv[1]) if len(sys.argv) > 1 else 8
    duty = float(sys.argv[2]) if len(sys.argv) > 2 else 0.7
    period = (float(sys.argv[3]) if len(sys.argv) > 3 else 250.0) / 1000.0
    print(f'stress pid={os.getpid()} workers={workers} duty={duty} '
          f'periodMs={period * 1000:.0f}', flush=True)
    children = []
    for _ in range(workers):
        child = multiprocessing.Process(target=burn, args=(duty, period),
                                        daemon=True)
        child.start()
        children.append(child)

    def stop(*_):
        for child in children:
            child.terminate()
        sys.exit(0)

    signal.signal(signal.SIGTERM, stop)
    signal.signal(signal.SIGINT, stop)
    while True:
        time.sleep(1.0)
        try:
            one, five, fifteen = os.getloadavg()
        except OSError:
            one = five = fifteen = -1.0
        print(f'{time.time():.1f}\t{one:.2f}\t{five:.2f}\t{fifteen:.2f}',
              flush=True)


if __name__ == '__main__':
    main()
