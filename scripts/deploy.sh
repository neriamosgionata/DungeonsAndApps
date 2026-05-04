#!/usr/bin/env bash
# Deploy dungeonsandapps to EC2.
# Called from CI with: ./scripts/deploy.sh <host> <image_tag>
set -euo pipefail

HOST="$1"
IMAGE_TAG="$2"
DOMAIN="${DEPLOY_DOMAIN:?DEPLOY_DOMAIN env var required}"
GITHUB_REPOSITORY="${GITHUB_REPOSITORY:?required}"

echo "==> Deploying $IMAGE_TAG to $HOST"

ssh -o StrictHostKeyChecking=no "ec2-user@$HOST" bash -s <<REMOTE
set -euo pipefail

# Substitute nginx config domain
sed 's/DOMAIN_PLACEHOLDER/$DOMAIN/g' /opt/dungeonsandapps/nginx.prod.conf > /opt/dungeonsandapps/nginx.conf

# Pull new image
echo "\${GHCR_TOKEN}" | docker login ghcr.io -u \${GHCR_USER} --password-stdin
GITHUB_REPOSITORY=$GITHUB_REPOSITORY IMAGE_TAG=$IMAGE_TAG docker compose \
  -f /opt/dungeonsandapps/docker-compose.prod.yml pull backend

# Rolling restart (zero downtime: postgres + nginx stay up)
DB_PASSWORD=\$(aws ssm get-parameter --name /dungeonsandapps/prod/DB_PASSWORD --with-decryption \
  --query Parameter.Value --output text --region eu-south-1)

export DB_PASSWORD GITHUB_REPOSITORY IMAGE_TAG
docker compose -f /opt/dungeonsandapps/docker-compose.prod.yml up -d --no-deps backend

# Cleanup old images
docker image prune -f
echo "==> Deploy complete: $IMAGE_TAG"
REMOTE
