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
Route53 (optional) ← auto DNS A record + certbot DNS-01 TLS
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

### 1. Generate secrets file

```bash
cd infra
bash gen-secrets.sh
```

Edit `secrets.tfvars` — fill in:
- `domain_name` — e.g. `app.example.com`
- `github_repo` — e.g. `acme/dungeonsandapps`
- `github_token` — `ghp_...` with `repo` + `secrets` scopes
- `route53_zone_id` — Route53 hosted zone ID for auto DNS + TLS, or `""` to skip

Secrets (`jwt_secret`, `db_password`, `admin_password`) are auto-generated.

### 2. Bootstrap Terraform state bucket

Creates the S3 bucket used for remote state. Run once, never again.

```bash
cd infra/bootstrap
terraform init
terraform apply
```

### 3. Apply main infra

```bash
cd infra
terraform init
terraform apply -var-file=secrets.tfvars
```

What this creates:
- EC2 `t4g.small` (ARM64, `al2023`) + EIP + security group (80/443/22)
- S3 media bucket (`dungeonsandapps-media-<ACCOUNT_ID>`) + IAM user/access key
- SSM SecureString parameters for all secrets + SSH private key backup
- RSA 4096 SSH keypair (stored in AWS Key Pairs + SSM)
- GitHub Actions secrets: `EC2_HOST`, `EC2_SSH_KEY`, `DEPLOY_DOMAIN`
- **If `route53_zone_id` set:** Route53 A record + certbot DNS-01 TLS cert (auto-renewed via cron)
- **If not:** TLS skipped — follow step 4 below after DNS propagates

### 4. Point DNS (no Route53 only)

```
A  YOUR_DOMAIN → $(terraform output -raw public_ip)
```

Then SSH in and run certbot manually:

```bash
ssh -i <(aws ssm get-parameter \
  --name /dungeonsandapps/prod/SSH_PRIVATE_KEY \
  --with-decryption --query Parameter.Value --output text) \
  ec2-user@$(terraform output -raw public_ip)

# on the instance
sudo certbot --nginx -d YOUR_DOMAIN
```

Or fetch key to a file:

```bash
aws ssm get-parameter \
  --name /dungeonsandapps/prod/SSH_PRIVATE_KEY \
  --with-decryption --query Parameter.Value --output text \
  > ~/.ssh/dungeonsandapps.pem
chmod 600 ~/.ssh/dungeonsandapps.pem
ssh -i ~/.ssh/dungeonsandapps.pem ec2-user@$(terraform output -raw public_ip)
```

### 5. Enable GitHub environment `production`

Settings → Environments → New → `production`.
Add required reviewers for a manual approval gate before each deploy.

### 6. Push to master

CI/CD runs automatically. All secrets are already in place.

---

## CI/CD pipeline (`.github/workflows/deploy.yml`)

On push to `master`:

1. **test-backend** — `cargo check` + `cargo test` (PostgreSQL service container)
2. **test-frontend** — `bunx svelte-check` + `bun test`
3. **build-backend** — cross-compile to `aarch64-unknown-linux-musl` (static, no glibc dep, `SQLX_OFFLINE=true`)
4. **build-frontend** — `bun run build` (SvelteKit `adapter-static`, `PUBLIC_API_BASE` injected)
5. **docker-push** — builds ARM64 Docker image from pre-compiled binary, pushes to GHCR
6. **deploy** — rsync static files + compose/nginx to EC2, pulls new image, restarts backend

GitHub Actions secrets written by Terraform: `EC2_HOST`, `EC2_SSH_KEY`, `DEPLOY_DOMAIN`.

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

## Re-applying / rotating secrets

```bash
# Edit secrets.tfvars, then:
terraform apply -var-file=secrets.tfvars
```

SSM updated in-place. Restart backend to pick up new env:

```bash
ssh -i ~/.ssh/dungeonsandapps.pem ec2-user@$(terraform output -raw public_ip) \
  "docker compose -f /opt/dungeonsandapps/docker-compose.prod.yml restart backend"
```
