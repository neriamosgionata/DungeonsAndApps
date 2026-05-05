#!/usr/bin/env bash
# Deploy dungeonsandapps to EC2.
# Called from CI with: ./scripts/deploy.sh <host> <image_tag>
set -euo pipefail

HOST="$1"
IMAGE_TAG="$2"
DOMAIN="${DEPLOY_DOMAIN:?DEPLOY_DOMAIN env var required}"
GITHUB_REPOSITORY="${GITHUB_REPOSITORY:?required}"
GITHUB_REPOSITORY="${GITHUB_REPOSITORY,,}"
GHCR_TOKEN="${GHCR_TOKEN:?GHCR_TOKEN env var required}"
GHCR_USER="${GHCR_USER:?GHCR_USER env var required}"
AWS_REGION="${AWS_REGION:?AWS_REGION env var required}"

echo "==> Deploying $IMAGE_TAG to $HOST"

# All $VAR below expand on the CI runner (heredoc unquoted).
# Remote-side dynamic values (DB_PASSWORD) use \$(...) to defer to EC2.
ssh -o StrictHostKeyChecking=no "ec2-user@$HOST" bash -s <<REMOTE
set -euo pipefail

sed "s|DOMAIN_PLACEHOLDER|$DOMAIN|g" /opt/dungeonsandapps/nginx.prod.conf > /opt/dungeonsandapps/nginx.conf

echo "$GHCR_TOKEN" | docker login ghcr.io -u "$GHCR_USER" --password-stdin

DB_PASSWORD=\$(aws ssm get-parameter --name /dungeonsandapps/prod/DB_PASSWORD \
  --with-decryption --query Parameter.Value --output text --region "$AWS_REGION")
S3_PUBLIC_URL=\$(aws ssm get-parameter --name /dungeonsandapps/prod/S3_PUBLIC_URL \
  --query Parameter.Value --output text --region "$AWS_REGION")

# Update .env.prod with S3_PUBLIC_URL if not already set
if ! grep -q "S3_PUBLIC_URL" /opt/dungeonsandapps/.env.prod 2>/dev/null; then
  echo "S3_PUBLIC_URL=\$S3_PUBLIC_URL" >> /opt/dungeonsandapps/.env.prod
fi

GITHUB_REPOSITORY="$GITHUB_REPOSITORY" IMAGE_TAG="$IMAGE_TAG" DB_PASSWORD="\$DB_PASSWORD" \
  docker compose -f /opt/dungeonsandapps/docker-compose.prod.yml pull backend

# First deploy: bring up all services. Subsequent: rolling restart backend only.
if docker compose -f /opt/dungeonsandapps/docker-compose.prod.yml ps -q postgres 2>/dev/null | grep -q .; then
  GITHUB_REPOSITORY="$GITHUB_REPOSITORY" IMAGE_TAG="$IMAGE_TAG" DB_PASSWORD="\$DB_PASSWORD" \
    docker compose -f /opt/dungeonsandapps/docker-compose.prod.yml up -d --no-deps backend
else
  GITHUB_REPOSITORY="$GITHUB_REPOSITORY" IMAGE_TAG="$IMAGE_TAG" DB_PASSWORD="\$DB_PASSWORD" \
    docker compose -f /opt/dungeonsandapps/docker-compose.prod.yml up -d
fi

docker exec dungeonsandapps-nginx nginx -s reload 2>/dev/null || true

docker image prune -f
echo "==> Deploy complete: $IMAGE_TAG"
REMOTE
