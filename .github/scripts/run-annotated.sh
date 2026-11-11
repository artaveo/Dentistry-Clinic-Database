#!/usr/bin/env bash
# Runs a command and keeps its output in the log as usual. If the command fails, the last lines of its
# output also become a GitHub error annotation, so the reason can be read on the run page without
# signing in to open the logs. A notice records how long it took either way.
# Usage: run-annotated.sh "<title, no commas or colons>" <command...>
set -o pipefail
title=$1
shift
log=$(mktemp)
start=$(date +%s)
"$@" 2>&1 | tee "$log"
code=${PIPESTATUS[0]}
secs=$(($(date +%s) - start))
if [ "$code" -ne 0 ]; then
  msg=$(tail -40 "$log" | tr -d '\r' | cut -c1-300 | sed -e 's/%/%25/g' | awk '{printf "%s%%0A", $0}')
  echo "::error title=$title failed (exit $code after ${secs}s)::$msg"
else
  echo "::notice title=$title::done in ${secs}s"
fi
exit "$code"
