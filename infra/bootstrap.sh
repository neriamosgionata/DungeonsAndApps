#!/usr/bin/env bash
# Full first-boot setup. Run from repo root or infra/.
# Usage: bash infra/bootstrap.sh
set -euo pipefail

INFRA="$(cd "$(dirname "$0")" && pwd)"
cd "$INFRA"

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; BOLD='\033[1m'; NC='\033[0m'
info()  { echo -e "${GREEN}==>${NC} ${BOLD}$*${NC}"; }
warn()  { echo -e "${YELLOW}[!]${NC} $*"; }
die()   { echo -e "${RED}[✗]${NC} $*" >&2; exit 1; }
ok()    { echo -e "${GREEN}[✓]${NC} $*"; }
ask()   { echo -en "${BOLD}$*${NC} "; }

# ── prereq check ──────────────────────────────────────────────────────────────
info "Checking prerequisites"
command -v terraform >/dev/null || die "terraform not found (need >= 1.10)"
command -v aws       >/dev/null || die "aws CLI not found"
command -v openssl   >/dev/null || die "openssl not found"

TF_VER=$(terraform version -json | python3 -c "import sys,json; print(json.load(sys.stdin)['terraform_version'])")
TF_MAJOR=$(echo "$TF_VER" | cut -d. -f1)
TF_MINOR=$(echo "$TF_VER" | cut -d. -f2)
[[ "$TF_MAJOR" -gt 1 || ("$TF_MAJOR" -eq 1 && "$TF_MINOR" -ge 10) ]] \
  || die "Terraform >= 1.10 required (found $TF_VER)"

aws sts get-caller-identity --output text > /dev/null \
  || die "AWS credentials not configured (run: aws configure)"

ok "Prerequisites OK (Terraform $TF_VER)"

# ── secrets.tfvars ────────────────────────────────────────────────────────────
TFVARS="$INFRA/secrets.tfvars"

if [[ ! -f "$TFVARS" ]]; then
  info "Generating secrets.tfvars"
  bash "$INFRA/gen-secrets.sh"
fi

# Check for unfilled placeholders
if grep -q "FILL_ME" "$TFVARS"; then
  warn "secrets.tfvars has unfilled values. Opening for edit..."
  echo ""
  cat -n "$TFVARS"
  echo ""
  ask "Press Enter after editing $TFVARS, or Ctrl-C to abort."
  read -r
  grep -q "FILL_ME" "$TFVARS" && die "Still has FILL_ME values — aborting."
fi

ok "secrets.tfvars ready"

# ── bootstrap: tfstate S3 bucket ──────────────────────────────────────────────
info "Step 1/4 — Bootstrap Terraform state bucket"

BOOTSTRAP_STATE="$INFRA/bootstrap/terraform.tfstate"
if [[ -f "$BOOTSTRAP_STATE" ]] && grep -q '"resources":\s*\[' "$BOOTSTRAP_STATE" 2>/dev/null \
   && [[ $(python3 -c "import json; d=json.load(open('$BOOTSTRAP_STATE')); print(len(d.get('resources',[])))") -gt 0 ]]; then
  ok "Bootstrap already applied — skipping"
else
  cd "$INFRA/bootstrap"
  terraform init -upgrade -input=false
  terraform apply -auto-approve -input=false
  cd "$INFRA"
fi

# ── main infra ────────────────────────────────────────────────────────────────
info "Step 2/4 — Init main infra"
terraform init -upgrade -input=false

info "Step 3/4 — Plan"
terraform plan -var-file=secrets.tfvars -out=tfplan -input=false

echo ""
ask "Review plan above. Apply? [y/N]"
read -r CONFIRM
[[ "$CONFIRM" =~ ^[Yy]$ ]] || { warn "Aborted."; exit 0; }

info "Step 4/4 — Apply"
terraform apply tfplan
rm -f tfplan

# ── outputs ───────────────────────────────────────────────────────────────────
echo ""
info "Done. Summary:"
PUBLIC_IP=$(terraform output -raw public_ip 2>/dev/null || echo "unknown")
DOMAIN=$(terraform output -raw domain_name 2>/dev/null || echo "unknown")
DNS_AUTO=$(terraform output -raw dns_auto_configured 2>/dev/null || echo "false")

ok "Public IP:  $PUBLIC_IP"
ok "Domain:     $DOMAIN"
ok "DNS auto:   $DNS_AUTO"
echo ""

if [[ "$DNS_AUTO" == "false" ]]; then
  warn "DNS not managed by Terraform."
  warn "Add A record: $DOMAIN → $PUBLIC_IP"
  warn "Then SSH in and run: sudo certbot --nginx -d $DOMAIN"
  warn "SSH key: aws ssm get-parameter --name /dungeonsandapps/prod/SSH_PRIVATE_KEY --with-decryption --query Parameter.Value --output text > ~/.ssh/dda.pem && chmod 600 ~/.ssh/dda.pem"
else
  ok "DNS A record + TLS cert provisioned automatically."
fi

echo ""
warn "Next: GitHub → Settings → Environments → create 'production' env"
warn "Then push to master — CI deploys automatically."
