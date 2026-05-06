#!/bin/bash
set -euo pipefail
exec > >(tee /var/log/userdata.log | logger -t userdata) 2>&1

# ── packages ──────────────────────────────────────────────────────────────────
dnf update -y
dnf install -y docker python3-pip git augeas-libs rsync
systemctl enable --now docker
usermod -aG docker ec2-user

# Docker Compose v2
mkdir -p /usr/local/lib/docker/cli-plugins
curl -fsSL "https://github.com/docker/compose/releases/latest/download/docker-compose-linux-aarch64" \
  -o /usr/local/lib/docker/cli-plugins/docker-compose
chmod +x /usr/local/lib/docker/cli-plugins/docker-compose

# certbot with Route53 DNS plugin (no HTTP challenge needed, works before nginx)
pip3 install certbot certbot-dns-route53

# ── secrets from SSM ──────────────────────────────────────────────────────────
region="${aws_region}"
ssm() { aws ssm get-parameter --name "$1" --with-decryption \
          --query Parameter.Value --output text --region "$region"; }

JWT_SECRET=$(ssm /dungeonsandapps/prod/JWT_SECRET)
DB_PASSWORD=$(ssm /dungeonsandapps/prod/DB_PASSWORD)
S3_ACCESS_KEY=$(ssm /dungeonsandapps/prod/S3_ACCESS_KEY)
S3_SECRET_KEY=$(ssm /dungeonsandapps/prod/S3_SECRET_KEY)
S3_PUBLIC_URL=$(ssm /dungeonsandapps/prod/S3_PUBLIC_URL)
ADMIN_PASSWORD=$(ssm /dungeonsandapps/prod/ADMIN_PASSWORD)
ADMIN_EMAIL=$(ssm /dungeonsandapps/prod/ADMIN_EMAIL)

# ── app env ───────────────────────────────────────────────────────────────────
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
S3_PUBLIC_URL=$S3_PUBLIC_URL
ADMIN_PASSWORD=$ADMIN_PASSWORD
ADMIN_EMAIL=$ADMIN_EMAIL
EOF
chmod 600 /opt/dungeonsandapps/.env.prod

# ── TLS certificate ───────────────────────────────────────────────────────────
%{ if use_route53 }
# DNS-01 challenge via Route53 — no HTTP server needed, fully automatic
certbot certonly \
  --dns-route53 \
  --non-interactive \
  --agree-tos \
  --email "$ADMIN_EMAIL" \
  -d "${domain}" \
  --logs-dir /var/log/letsencrypt

# Allow nginx container (runs as root inside Docker) to read certs
chmod 755 /etc/letsencrypt/live /etc/letsencrypt/archive
chmod 755 /etc/letsencrypt/archive/${domain}

# Install cronie for cron support
dnf install -y cronie
systemctl enable --now crond

# Auto-renewal: twice daily (certbot only acts when <30 days remain)
cat > /etc/cron.d/certbot-renew <<'CRON'
0 0,12 * * * root certbot renew --dns-route53 --quiet && chmod 755 /etc/letsencrypt/live /etc/letsencrypt/archive /etc/letsencrypt/archive/DOMAIN_PLACEHOLDER && docker exec dungeonsandapps-nginx nginx -s reload 2>/dev/null || true
CRON
sed -i "s/DOMAIN_PLACEHOLDER/${domain}/g" /etc/cron.d/certbot-renew
chmod 644 /etc/cron.d/certbot-renew
%{ else }
# No Route53 zone provided — HTTP-01 challenge via nginx
# Cert will be obtained on first deploy after DNS propagates

# Install cronie for cron support
dnf install -y cronie
systemctl enable --now crond

# Auto-renewal: twice daily at 06:15 and 18:15 UTC
# Offset from EC2 schedule (start 16:00, stop 00:00) to avoid conflicts
cat > /etc/cron.d/certbot-renew <<'CRON'
# Certbot auto-renewal - runs at 6:15 and 18:15 UTC
15 6,18 * * * root certbot renew --quiet --nginx && docker exec dungeonsandapps-nginx nginx -s reload 2>/dev/null || true
CRON
chmod 644 /etc/cron.d/certbot-renew

mkdir -p /etc/letsencrypt/live/${domain}
%{ endif }

# ── docker compose stub ───────────────────────────────────────────────────────
# Placeholder compose file so systemd service starts on first boot
# Real files synced by deploy.sh on each deploy
cat > /opt/dungeonsandapps/docker-compose.prod.yml <<'EOF'
services:
  postgres:
    image: postgres:17-alpine
    container_name: dungeonsandapps-postgres
    restart: unless-stopped
    environment:
      POSTGRES_USER: cinghiale
      POSTGRES_PASSWORD: ${DB_PASSWORD}
      POSTGRES_DB: dungeonsandapps
    volumes:
      - pgdata:/var/lib/postgresql/data
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U cinghiale -d dungeonsandapps"]
      interval: 5s
      timeout: 3s
      retries: 10
    networks:
      - internal

  backend:
    image: ghcr.io/neriamosgionata/DungeonsAndApps/backend:latest
    container_name: dungeonsandapps-backend
    restart: unless-stopped
    env_file: .env.prod
    depends_on:
      postgres:
        condition: service_healthy
    networks:
      - internal
    expose:
      - "8080"

  nginx:
    image: nginx:1.27-alpine
    container_name: dungeonsandapps-nginx
    restart: unless-stopped
    ports:
      - "80:80"
      - "443:443"
    volumes:
      - /opt/dungeonsandapps/nginx.conf:/etc/nginx/conf.d/default.conf:ro
      - /opt/dungeonsandapps/web:/var/www/dungeonsandapps:ro
      - /etc/letsencrypt:/etc/letsencrypt:ro
      - /var/lib/letsencrypt:/var/lib/letsencrypt:ro
    depends_on:
      - backend
    networks:
      - internal

volumes:
  pgdata:
    driver: local

networks:
  internal:
    driver: bridge
EOF

# ── nginx config stub ─────────────────────────────────────────────────────────
mkdir -p /opt/dungeonsandapps/web
touch /opt/dungeonsandapps/nginx.conf
chown -R ec2-user:ec2-user /opt/dungeonsandapps

# ── done ──────────────────────────────────────────────────────────────────────
# Note: Docker containers auto-start on boot via restart: unless-stopped
# No systemd service needed - deploy.sh handles first deploy
touch /opt/dungeonsandapps/.userdata_complete
echo "userdata complete"
