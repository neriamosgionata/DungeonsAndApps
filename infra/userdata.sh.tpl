#!/bin/bash
set -euo pipefail

# Install Docker + helpers
dnf update -y
dnf install -y docker python3-pip git
systemctl enable --now docker
usermod -aG docker ec2-user

# Docker Compose v2
mkdir -p /usr/local/lib/docker/cli-plugins
curl -SL "https://github.com/docker/compose/releases/latest/download/docker-compose-linux-aarch64" \
  -o /usr/local/lib/docker/cli-plugins/docker-compose
chmod +x /usr/local/lib/docker/cli-plugins/docker-compose

# Pull secrets from SSM
JWT_SECRET=$(aws ssm get-parameter --name /dungeonsandapps/prod/JWT_SECRET --with-decryption --query Parameter.Value --output text --region ${aws_region})
DB_PASSWORD=$(aws ssm get-parameter --name /dungeonsandapps/prod/DB_PASSWORD --with-decryption --query Parameter.Value --output text --region ${aws_region})
S3_ACCESS_KEY=$(aws ssm get-parameter --name /dungeonsandapps/prod/S3_ACCESS_KEY --with-decryption --query Parameter.Value --output text --region ${aws_region})
S3_SECRET_KEY=$(aws ssm get-parameter --name /dungeonsandapps/prod/S3_SECRET_KEY --with-decryption --query Parameter.Value --output text --region ${aws_region})
ADMIN_PASSWORD=$(aws ssm get-parameter --name /dungeonsandapps/prod/ADMIN_PASSWORD --with-decryption --query Parameter.Value --output text --region ${aws_region})

# App directory
mkdir -p /opt/dungeonsandapps
cat > /opt/dungeonsandapps/.env.prod <<EOF
DATABASE_URL=postgres://cinghiale:$DB_PASSWORD@postgres:5432/dungeonsandapps
JWT_SECRET=$JWT_SECRET
BIND_ADDR=0.0.0.0:8080
CORS_ORIGIN=https://${domain}
RUST_LOG=info
S3_ENDPOINT=https://s3.${aws_region}.amazonaws.com
S3_BUCKET=${s3_bucket}
S3_ACCESS_KEY=$S3_ACCESS_KEY
S3_SECRET_KEY=$S3_SECRET_KEY
S3_REGION=${aws_region}
ADMIN_PASSWORD=$ADMIN_PASSWORD
EOF
chmod 600 /opt/dungeonsandapps/.env.prod

# Certbot for Let's Encrypt
pip3 install certbot certbot-nginx

# Signal readiness — CI will deploy via deploy.sh after instance is healthy
touch /opt/dungeonsandapps/.userdata_complete
