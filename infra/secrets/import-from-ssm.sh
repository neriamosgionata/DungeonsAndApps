#!/usr/bin/env bash
# One-time migration: pull every SSM parameter under
# /dungeonsandapps/prod/* and write a JSON object to
# infra/secrets/secrets.json. The JSON keys are the parameter suffixes
# (so /dungeonsandapps/prod/DB_PASSWORD becomes {"DB_PASSWORD": "..."}).
#
# Usage:
#   AWS_REGION=eu-south-1 ./infra/secrets/import-from-ssm.sh
#
# Output: infra/secrets/secrets.json (git-ignored).

set -euo pipefail

REGION="${AWS_REGION:-eu-south-1}"
OUT="$(dirname "$0")/secrets.json"

if ! command -v aws >/dev/null 2>&1; then
  echo "error: aws cli not installed" >&2
  exit 1
fi
if ! command -v python3 >/dev/null 2>&1; then
  echo "error: python3 not installed" >&2
  exit 1
fi

echo "==> Fetching /dungeonsandapps/prod/* from SSM (region=$REGION)..."

aws ssm get-parameters-by-path \
  --path /dungeonsandapps/prod \
  --with-decryption --region "$REGION" \
  --query 'Parameters[].[Name,Value]' \
  --output text | \
  python3 -c "
import json, sys
out = {}
for line in sys.stdin:
    name, value = line.split('\t', 1)
    key = name.rsplit('/', 1)[-1]
    out[key] = value
print(json.dumps(out, indent=2, sort_keys=True))
" > "$OUT"

echo "==> Wrote $OUT"
echo "==> Contents:"
cat "$OUT" | python3 -c "import json, sys; d=json.load(sys.stdin); [print(f'  {k:20s} ({len(v)} chars)') for k, v in d.items()]"
echo
echo "Next: cd infra/terraform && terraform apply"
