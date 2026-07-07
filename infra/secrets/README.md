# infra/secrets

This directory holds the local source-of-truth for app secrets that
are uploaded to AWS Secrets Manager by `infra/terraform/secrets.tf`.

The directory is **git-ignored** (see root `.gitignore`:
`infra/*.tfvars` and any file in `infra/secrets/` except `.gitkeep`).
Do NOT commit real values.

## Files

- `.gitkeep` — keeps the directory in git.
- `secrets.json` — JSON object with all secret key/value pairs. The
  keys here become fields in the SM secret `dungeonsandapps/prod`.

## Bootstrap from AWS SSM (one-time migration)

If you have existing values in SSM at `/dungeonsandapps/prod/*`, run:

```bash
aws ssm get-parameters-by-path \
  --path /dungeonsandapps/prod \
  --with-decryption --region eu-south-1 \
  --query 'Parameters[].[Name,Value]' \
  --output text | \
  python3 -c "
import json, sys
out = {}
for line in sys.stdin:
    name, value = line.split('\t', 1)
    key = name.rsplit('/', 1)[-1]
    out[key] = value
print(json.dumps(out, indent=2))
" > infra/secrets/secrets.json
```

This pulls every parameter under the prefix and writes a JSON
object whose keys are the parameter suffixes.

## Bootstrap from scratch (new deployment)

Copy the example file and fill in the values:

```bash
cp infra/secrets/secrets.json.example infra/secrets/secrets.json
$EDITOR infra/secrets/secrets.json
```

Required keys (see `backend/src/config.rs` for the full list):

| Key | Used by |
|-----|---------|
| `ADMIN_EMAIL` | first-boot admin user |
| `ADMIN_PASSWORD` | first-boot admin user |
| `DB_PASSWORD` | `DATABASE_URL` build |
| `JWT_SECRET` | JWT signing |
| `S3_ACCESS_KEY` | file uploads |
| `S3_SECRET_KEY` | file uploads |
| `S3_PUBLIC_URL` | file URL builder |
| `SSH_PRIVATE_KEY` | (optional) for EC2 ssh access |

The backend reads the values as a JSON blob via the SM ARN passed
as `SECRETS_MANAGER_ARN` (or the legacy `DATABASE_URL` env var
which is still supported for back-compat).

## Rotate a value

1. Edit `infra/secrets/secrets.json`.
2. `cd infra/terraform && terraform apply`.
3. Re-deploy the backend so it picks up the new version.

Secrets Manager keeps a versioned history; you can roll back via
the AWS console (Secrets Manager → dungeonsandandapps/prod →
Version stage) if a rotation breaks the app.
