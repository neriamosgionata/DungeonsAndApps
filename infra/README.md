# DungeonsAndApps Infrastructure

## Architecture

Single EC2 `t4g.small` (ARM64) + S3 + Postgres on EBS. ~$7.35/mo on-demand.

```
GitHub Actions → GHCR (Docker image) → EC2
                                     ├── nginx (80/443, static + proxy)
                                     ├── backend:8080 (Rust/Docker)
                                     └── postgres:5432 (Docker)
S3 ← backend (media uploads)
SSM Parameter Store ← secrets (JWT, DB password, S3 keys, SSH private key)
```

Terraform manages all infra **and** pushes GitHub Actions secrets automatically.
No manual secret handling required after first apply.

---

## Prerequisites

- Terraform >= 1.10
- AWS CLI configured (`aws sts get-caller-identity` must succeed)
- GitHub personal access token with scopes: `repo`, `secrets`

---

## First-time setup

### 1. Bootstrap Terraform state bucket

Creates the S3 bucket used for remote state. Run once, never again.

```bash
cd infra/bootstrap
terraform init
terraform apply
```

### 2. Apply main infra

```bash
cd infra
terraform init

terraform apply \
  -var="domain_name=YOUR_DOMAIN" \
  -var="github_repo=OWNER/REPO" \
  -var="github_token=ghp_..." \
  -var="jwt_secret=$(openssl rand -hex 32)" \
  -var="db_password=$(openssl rand -hex 16)" \
  -var="admin_password=$(openssl rand -hex 16)"
```

What this creates:
- EC2 instance + EIP + security group
- S3 media bucket + IAM user/access key
- SSM SecureString parameters for all secrets
- RSA SSH keypair (stored in AWS + SSM backup)
- GitHub Actions secrets: `EC2_HOST`, `EC2_SSH_KEY`, `DEPLOY_DOMAIN`

### 3. Point DNS

```
A  YOUR_DOMAIN → $(terraform output -raw public_ip)
```

Wait for propagation before the next step.

### 4. TLS certificate (on EC2)

SSH is available immediately after apply — key is managed by Terraform.

```bash
# Get IP
terraform output -raw public_ip

# SSH in
ssh -i <(aws ssm get-parameter \
  --name /dungeonsandapps/prod/SSH_PRIVATE_KEY \
  --with-decryption --query Parameter.Value --output text) \
  ec2-user@$(terraform output -raw public_ip)

# On the instance
sudo certbot --nginx -d YOUR_DOMAIN
```

Or if you prefer a local key file:

```bash
terraform output -raw public_ip   # note the IP
aws ssm get-parameter \
  --name /dungeonsandapps/prod/SSH_PRIVATE_KEY \
  --with-decryption --query Parameter.Value --output text \
  > ~/.ssh/dungeonsandapps.pem
chmod 600 ~/.ssh/dungeonsandapps.pem
ssh -i ~/.ssh/dungeonsandapps.pem ec2-user@<IP>
```

### 5. Enable GitHub environment `production`

Settings → Environments → New → `production`.
Add required reviewers if you want a manual approval gate before each deploy.

### 6. Push to master

CI/CD runs automatically. All secrets are already in place.

---

## SQLX offline cache

CI uses `SQLX_OFFLINE=true`. Commit the query cache after schema changes:

```bash
cd backend
cargo sqlx prepare
git add .sqlx && git commit -m "chore: update sqlx query cache"
```

---

## Costs (~$7.35/mo on-demand)

| Item | $/mo |
|------|------|
| t4g.small EC2 | $6.05 |
| EBS gp3 10GB | $0.80 |
| S3 (<1GB) | $0.50 |

---

## Re-applying / updating secrets

To rotate a secret:

```bash
terraform apply -var="jwt_secret=<new_value>" ...
```

SSM is updated in-place. EC2 reads SSM at startup — restart the backend container to pick up new values:

```bash
ssh ec2-user@$(terraform output -raw public_ip) \
  "docker compose -f /opt/dungeonsandapps/docker-compose.prod.yml restart backend"
```
