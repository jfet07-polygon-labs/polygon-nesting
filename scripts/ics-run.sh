#!/bin/sh
# One command, one log, one exit status - read directly, never through a pipe.
#
#   scripts/ics-run.sh LOGPATH CMD [ARG ...]
#
# The overlap-ICS round's protocol forbids `cmd | tee log` and `cmd | tail`,
# because both report the pipe's status instead of the command's. This wrapper
# is the only indirection allowed: it redirects, reads `$?` on the next line,
# and prints it.
log="$1"
shift
"$@" > "$log" 2>&1
status=$?
echo "EXIT=$status"
exit 0
