#!/usr/bin/env bash
# Lists current open-question statuses from docs/OPEN_QUESTIONS.md. Most
# items are now resolved as of Aug 22, this is mainly useful to check
# items 5 (Miner attribution mechanics) and 7 (registration status),
# the two still genuinely open.

set -euo pipefail

echo "Current open question statuses:"
grep -A1 "^### " docs/OPEN_QUESTIONS.md | grep -E "^###|Answer:"
