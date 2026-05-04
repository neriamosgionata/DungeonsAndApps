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
ADMIN_PASSWORD=$(ssm /dungeonsandapps/prod/ADMIN_PASSWORD)

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
ADMIN_PASSWORD=$ADMIN_PASSWORD
EOF
chmod 600 /opt/dungeonsandapps/.env.prod

# ── TLS certificate ───────────────────────────────────────────────────────────
%{ if use_route53 }
# DNS-01 challenge via Route53 — no HTTP server needed, fully automatic
certbot certonly \
  --dns-route53 \
  --non-interactive \
  --agree-tos \
  --email "admin@${domain}" \
  -d "${domain}" \
  --logs-dir /var/log/letsencrypt

# Auto-renewal: twice daily (certbot only acts when <30 days remain)
echo "0 0,12 * * * root certbot renew --dns-route53 --quiet && \
  docker exec dungeonsandapps-nginx nginx -s reload" \
  > /etc/cron.d/certbot-renew
%{ else }
# No Route53 zone provided — TLS not configured automatically.
# After DNS propagates, SSH in and run:
#   sudo certbot --nginx -d ${domain}
mkdir -p /etc/letsencrypt/live/${domain}
%{ endif }

# ── nginx config ──────────────────────────────────────────────────────────────
# Injected by deploy.sh on each deploy; stub here so nginx starts if already present
mkdir -p /opt/dungeonsandapps/web
touch /opt/dungeonsandapps/nginx.conf

# ── done ──────────────────────────────────────────────────────────────────────
touch /opt/dungeonsandapps/.userdata_complete
echo "userdata complete"
