# First Boot — DungeonsAndApps

Step-by-step to go from zero to running production in ~10 minutes.

---

## 1. Prerequisites on your machine

```bash
# macOS
brew install terraform awscli git

# Verify
terraform --version   # >= 1.10
aws sts get-caller-identity --profile aws-amos
```

You need a GitHub [personal access token](https://github.com/settings/tokens) with scopes: `repo`, `secrets` (write secrets to the repo for CI/CD).

---

## 2. Generate secrets

```bash
cd infra
bash gen-secrets.sh
```

Edit `infra/secrets.tfvars` — the script generates random passwords. Only these fields need manual input:

| Field | Example | Notes |
|-------|---------|-------|
| `domain_name` | `dapp.dungeonsandapp.com` | Your app domain |
| `github_repo` | `neriamosgionata/DungeonsAndApps` | `owner/repo` |
| `github_token` | `ghp_...` | PAT with repo+secrets |
| `route53_zone_id` | `Z06554431VLB...` | Or `""` to skip auto-DNS |
| `admin_email` | `admin@example.com` | Seeded admin login |

---

## 3. Bootstrap Terraform state (once ever)

```bash
cd infra/bootstrap
terraform init
terraform apply
cd ..
```

Creates the S3 bucket that holds Terraform's own state. Run once per AWS account.

---

## 4. Provision infrastructure

```bash
cd infra
terraform init
AWS_PROFILE=aws-amos terraform apply -var-file=secrets.tfvars
```

Wait ~3-5 minutes. Terraform creates:
- EC2 `t4g.small` (ARM, spot) + Elastic IP
- S3 media bucket + IAM user for backend access
- All secrets in AWS SSM Parameter Store
- GitHub Actions secrets (`EC2_HOST`, `EC2_SSH_KEY`, `DEPLOY_DOMAIN`)
- Route53 A record + Let's Encrypt TLS (if you set `route53_zone_id`)
- RSA 4096 SSH keypair

Output after apply:

```
domain_name = "dapp.dungeonsandapp.com"
public_ip   = "18.102.254.232"
s3_bucket   = "dungeonsandapps-media-579241412831"
```

---

## 5. DNS (only if you skipped Route53)

If you set `route53_zone_id = ""`, point DNS manually:

```
A  your.domain.com  →  <public_ip from terraform output>
```

Then SSH in to get a TLS cert:

```bash
# Get SSH key from SSM
aws ssm get-parameter \
  --name /dungeonsandapps/prod/SSH_PRIVATE_KEY \
  --with-decryption --query Parameter.Value --output text \
  --profile aws-amos --region eu-south-1 \
  > ~/.ssh/dungeonsandapps.pem
chmod 600 ~/.ssh/dungeonsandapps.pem

# SSH in
ssh -i ~/.ssh/dungeonsandapps.pem ec2-user@<public_ip>

# Request cert
sudo certbot --nginx -d your.domain.com
```

---

## 6. Enable GitHub environment

1. Go to repo → **Settings** → **Environments**
2. New environment → name it `production`
3. Add required reviewers (optional — manual approval gate per deploy)

This is what the `deploy` job in `.github/workflows/deploy.yml` references.

---

## 7. First deploy

Deploys trigger on **tags** matching `v*`:

```bash
git tag v0.1.0
git push origin v0.1.0
```

CI/CD pipeline runs:
1. Backend tests (PostgreSQL service container)
2. Frontend tests (Vitest + svelte-check)
3. Cross-compile backend to `aarch64-unknown-linux-musl`
4. Build frontend static (`PUBLIC_API_BASE=https://your.domain.com`)
5. Push ARM64 Docker image to GHCR
6. rsync static files + nginx config to EC2
7. Pull new image, restart backend

Monitor at: `https://github.com/<owner>/<repo>/actions`

---

## 8. Verify

```bash
# Admin login (credentials from secrets.tfvars)
curl -X POST https://dapp.dungeonsandapp.com/api/v1/auth/login \
  -H 'Content-Type: application/json' \
  -d '{"email":"admin@example.com","password":"<ADMIN_PASSWORD>"}'
```

Open `https://dapp.dungeonsandapp.com` in browser. Log in with admin credentials.

---

## 9. Post-deploy checklist

- [ ] Upload a campaign image → appears without 403
- [ ] Upload a character portrait → appears without 403
- [ ] Create a campaign, add a character, enter initiative
- [ ] WS connection works (dice roll in chat)

---

## Common issues

### Images return 403 Forbidden
`S3_PUBLIC_URL` must point to the app proxy, not the raw S3 bucket. Verify:
```bash
ssh -i ~/.ssh/dungeonsandapps.pem ec2-user@<ip> \
  'grep S3_PUBLIC_URL /opt/dungeonsandapps/.env.prod'
# Should be: https://your.domain.com/api/v1/files
```
If wrong, run `terraform apply` in `infra/` then restart backend on the instance.

### Backend won't start
```bash
ssh -i ~/.ssh/dungeonsandapps.pem ec2-user@<ip> \
  'docker compose -f /opt/dungeonsandapps/docker-compose.prod.yml logs backend --tail 50'
```
Common causes: Postgres not ready (wait 10s after boot), wrong DB credentials in SSM.

### Can't SSH — connection refused
EC2 spot instance may have been interrupted. Terraform creates a spot request — if spot capacity is unavailable in that AZ, the instance won't launch. Check:
```bash
aws ec2 describe-spot-instance-requests \
  --spot-instance-request-ids sir-<id> \
  --profile aws-amos --region eu-south-1
```
If stuck, bump `spot_price` in `variables.tf` or switch to on-demand (set `spot_price = ""`).

### DNS-01 TLS failed
- Route53 zone must be public (NS records delegated)
- `route53_zone_id` must be the **hosted zone ID** (not the domain name)
- Certbot logs: `ssh ec2-user@<ip> 'sudo cat /var/log/letsencrypt/letsencrypt.log | tail -30'`

---

## SSH quick reference

```bash
# Store key once
aws ssm get-parameter \
  --name /dungeonsandapps/prod/SSH_PRIVATE_KEY \
  --with-decryption --query Parameter.Value --output text \
  --profile aws-amos --region eu-south-1 \
  > ~/.ssh/dungeonsandapps.pem
chmod 600 ~/.ssh/dungeonsandapps.pem

# SSH
ssh -i ~/.ssh/dungeonsandapps.pem ec2-user@18.102.254.232

# Check containers
docker compose -f /opt/dungeonsandapps/docker-compose.prod.yml ps

# Restart backend
docker compose -f /opt/dungeonsandapps/docker-compose.prod.yml restart backend

# View logs
docker compose -f /opt/dungeonsandapps/docker-compose.prod.yml logs backend -f --tail 30

# Read env vars
cat /opt/dungeonsandapps/.env.prod
```

---

## Architecture at a glance

```
Browser (HTTPS)
  │
  ├─ /api/* ──→ nginx:443 ──→ backend:8080 (Rust/Docker)
  │                              ├── Postgres:5432 (Docker)
  │                              └── S3 (media, IAM auth)
  │
  ├─ /ws ────→ nginx ──→ WebSocket upgrade → backend
  │
  └─ /* ─────→ nginx ──→ static files (SvelteKit SPA)
```

All secrets live in AWS SSM Parameter Store (never in `.env` committed files). GitHub Actions secrets auto-populated by Terraform.

---

*Last updated: 2026-06-16. Keep in sync with infra changes.*
