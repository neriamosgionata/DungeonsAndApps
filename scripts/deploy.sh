#!/usr/bin/env bash
# Deploy dungeonsandapps to EC2.
# Called from CI with: ./scripts/deploy.sh <host> <image_tag>
# Set SKIP_BACKEND=true to skip backend container update (frontend-only deploys).
set -euo pipefail

HOST="$1"
IMAGE_TAG="$2"
DOMAIN="${DEPLOY_DOMAIN:?DEPLOY_DOMAIN env var required}"
GITHUB_REPOSITORY="${GITHUB_REPOSITORY:?required}"
GITHUB_REPOSITORY="${GITHUB_REPOSITORY,,}"
GHCR_TOKEN="${GHCR_TOKEN:?GHCR_TOKEN env var required}"
GHCR_USER="${GHCR_USER:?GHCR_USER env var required}"
AWS_REGION="${AWS_REGION:?AWS_REGION env var required}"
SKIP_BACKEND="${SKIP_BACKEND:-false}"
DEPLOY_KEY="${DEPLOY_KEY:-${HOME}/.ssh/id_rsa}"

echo "==> Deploying $IMAGE_TAG to $HOST (skip_backend=$SKIP_BACKEND)"

# All $VAR below expand on the CI runner (heredoc unquoted).
# Remote-side dynamic values (DB_PASSWORD) use \$(...) to defer to EC2.
ssh -i "$DEPLOY_KEY" -o StrictHostKeyChecking=accept-new "ec2-user@$HOST" bash -s <<REMOTE
set -euo pipefail

sed "s|DOMAIN_PLACEHOLDER|$DOMAIN|g" /opt/dungeonsandapps/nginx.prod.conf > /opt/dungeonsandapps/nginx.conf

if [ "$SKIP_BACKEND" != "true" ]; then
  echo "$GHCR_TOKEN" | docker login ghcr.io -u "$GHCR_USER" --password-stdin

  # Fetch the full secrets blob from AWS Secrets Manager (one round trip
  # instead of one per parameter). The SM resource is created by
  # infra/terraform/secrets.tf. The EC2 role
  # (`dungeonsandapps-ec2-role`) has `secretsmanager:GetSecretValue`
  # attached by terraform.
  SECRETS_JSON=\$(aws secretsmanager get-secret-value \
    --secret-id dungeonsandapps/prod \
    --query SecretString --output text --region "$AWS_REGION")
  DB_PASSWORD=\$(echo "\$SECRETS_JSON" | python3 -c "import json,sys; print(json.load(sys.stdin)['DB_PASSWORD'])")
  S3_PUBLIC_URL=\$(echo "\$SECRETS_JSON" | python3 -c "import json,sys; print(json.load(sys.stdin)['S3_PUBLIC_URL'])")

  # Write secrets to temp env file so they don't leak in ps aux
  printf 'DB_PASSWORD=%s\nGITHUB_REPOSITORY=%s\nIMAGE_TAG=%s\n' \
    "\$DB_PASSWORD" "$GITHUB_REPOSITORY" "$IMAGE_TAG" > /opt/dungeonsandapps/.env.deploy

  # Update .env.prod with fresh S3_PUBLIC_URL from SSM
  if grep -q "^S3_PUBLIC_URL=" /opt/dungeonsandapps/.env.prod 2>/dev/null; then
    sed -i "s|^S3_PUBLIC_URL=.*|S3_PUBLIC_URL=\$S3_PUBLIC_URL|" /opt/dungeonsandapps/.env.prod
  else
    echo "S3_PUBLIC_URL=\$S3_PUBLIC_URL" >> /opt/dungeonsandapps/.env.prod
  fi

  docker compose --env-file /opt/dungeonsandapps/.env.deploy \
    -f /opt/dungeonsandapps/docker-compose.prod.yml pull backend

  # First deploy: bring up all services. Subsequent: rolling restart backend only.
  if docker compose --env-file /opt/dungeonsandapps/.env.deploy \
       -f /opt/dungeonsandapps/docker-compose.prod.yml ps -q postgres 2>/dev/null | grep -q .; then
    docker compose --env-file /opt/dungeonsandapps/.env.deploy \
      -f /opt/dungeonsandapps/docker-compose.prod.yml up -d --no-deps backend
  else
    docker compose --env-file /opt/dungeonsandapps/.env.deploy \
      -f /opt/dungeonsandapps/docker-compose.prod.yml up -d
  fi

  rm -f /opt/dungeonsandapps/.env.deploy

  docker image prune -f
else
  echo "==> Skipping backend deploy (frontend-only)"
fi

docker exec dungeonsandapps-nginx nginx -s reload 2>/dev/null || true

echo "==> Deploy complete: $IMAGE_TAG"
REMOTE
