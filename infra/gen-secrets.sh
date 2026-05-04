#!/usr/bin/env bash
# Generates secrets.tfvars. Edit the FILL_ME values, then run terraform apply.
set -euo pipefail

OUT="$(dirname "$0")/secrets.tfvars"

if [[ -f "$OUT" ]]; then
  echo "secrets.tfvars already exists. Delete it first to regenerate."
  exit 1
fi

cat > "$OUT" <<EOF
# --- fill these in ---
domain_name     = "FILL_ME"         # e.g. app.example.com
github_repo     = "FILL_ME"         # e.g. acme/dungeonsandapps
github_token    = "FILL_ME"         # ghp_...  (scopes: repo + secrets)
route53_zone_id = "FILL_ME_OR_EMPTY" # Route53 hosted zone ID for auto DNS+TLS
                                     # leave as "" to skip (manage DNS manually)

# --- auto-generated ---
jwt_secret     = "$(openssl rand -hex 32)"
db_password    = "$(openssl rand -hex 16)"
admin_password = "$(openssl rand -hex 16)"
EOF

chmod 600 "$OUT"
echo "Written to $OUT — fill in the FILL_ME values, then:"
echo "  cd infra/bootstrap && terraform init && terraform apply"
echo "  cd ../  && terraform init"
echo "  terraform apply -var-file=secrets.tfvars"
