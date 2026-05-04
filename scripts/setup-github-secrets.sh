#!/usr/bin/env bash
# Reads Terraform outputs + sets all GitHub Actions secrets.
# Requires: terraform (applied), gh (authenticated), EC2 key at ~/.ssh/dungeonsandapps-prod.pem
# Usage: ./scripts/setup-github-secrets.sh [path/to/key.pem]
set -euo pipefail

KEY_PATH="${1:-$HOME/.ssh/dungeonsandapps-prod.pem}"

if [[ ! -f "$KEY_PATH" ]]; then
  echo "SSH key not found at $KEY_PATH — pass path as first arg: $0 /path/to/key.pem"
  exit 1
fi

cd "$(dirname "$0")/../infra"

echo "==> Reading Terraform outputs..."
EC2_HOST=$(terraform output -raw public_ip)
DEPLOY_DOMAIN=$(terraform output -raw domain_name)

echo "    EC2_HOST=$EC2_HOST"
echo "    DEPLOY_DOMAIN=$DEPLOY_DOMAIN"

echo "==> Fetching known_hosts for $EC2_HOST..."
EC2_KNOWN_HOSTS=$(ssh-keyscan -H "$EC2_HOST" 2>/dev/null)

echo "==> Setting GitHub secrets..."
gh secret set EC2_HOST        --body "$EC2_HOST"
gh secret set EC2_SSH_KEY     < "$KEY_PATH"
gh secret set EC2_KNOWN_HOSTS --body "$EC2_KNOWN_HOSTS"
gh secret set DEPLOY_DOMAIN   --body "$DEPLOY_DOMAIN"

echo "==> Done."
gh secret list
