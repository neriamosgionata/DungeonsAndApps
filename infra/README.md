# Cinghialapp Infrastructure

## Architecture

Single EC2 `t4g.small` (ARM64) + S3 + Postgres on EBS.

```
GitHub Actions → GHCR (Docker image) → EC2
                                     ├── nginx (80/443, static + proxy)
                                     ├── backend:8080 (Rust binary in Docker)
                                     └── postgres:5432 (Docker)
S3 ← backend (media uploads)
```

## First-time setup

### 1. Bootstrap Terraform state bucket

```bash
# Create infra/bootstrap/main.tf with the commented block in tfstate-bootstrap.tf
# then:
cd infra/bootstrap
terraform init && terraform apply
```

### 2. Create EC2 key pair

```bash
aws ec2 create-key-pair --key-name dungeonsandapps-prod \
  --query 'KeyMaterial' --output text > ~/.ssh/dungeonsandapps-prod.pem
chmod 600 ~/.ssh/dungeonsandapps-prod.pem
```

### 3. Apply Terraform

```bash
cd infra
terraform init
terraform apply \
  -var="key_pair_name=dungeonsandapps-prod" \
  -var="domain_name=YOUR_DOMAIN" \
  -var="jwt_secret=$(openssl rand -hex 32)" \
  -var="db_password=$(openssl rand -hex 16)" \
  -var="admin_password=$(openssl rand -hex 16)" \
  -var='allowed_ssh_cidrs=["YOUR_IP/32"]'
```

### 4. Point DNS

Add an A record: `YOUR_DOMAIN → <public_ip from terraform output>`.

### 5. Get TLS certificate (on EC2)

```bash
ssh ec2-user@<ip>
sudo certbot --nginx -d YOUR_DOMAIN
```

### 6. Set GitHub repository secrets

| Secret | Value |
|--------|-------|
| `EC2_HOST` | Public IP from `terraform output public_ip` |
| `EC2_SSH_KEY` | Contents of `~/.ssh/dungeonsandapps-prod.pem` |
| `EC2_KNOWN_HOSTS` | Run `ssh-keyscan <IP>` |
| `DEPLOY_DOMAIN` | Your domain name |

### 7. Enable GitHub environment `production`

Settings → Environments → New → `production`. Add required reviewers if you want manual approval gate.

## SQLX offline mode

CI uses `SQLX_OFFLINE=true`. You must commit the `.sqlx/` query cache:

```bash
cd backend
cargo sqlx prepare
git add .sqlx && git commit -m "chore: update sqlx query cache"
```

## Costs (~$7.35/mo on-demand)

| Item | $/mo |
|------|------|
| t4g.small EC2 | $6.05 |
| EBS gp3 10GB | $0.80 |
| S3 (<1GB) | $0.50 |
