# infra/terraform

Terraform for the DungeonsAndApps infrastructure:

- `secrets.tf` — AWS Secrets Manager secret `dungeonsandapps/prod` + IAM
  permissions for the EC2 role + CI runner.

EC2 start/stop scheduler was removed. Instance now runs 24/7.

## Secrets

All app secrets live in a single AWS Secrets Manager secret,
`dungeonsandapps/prod`. The local source-of-truth file is
`infra/secrets/secrets.json` (git-ignored). On `terraform apply`,
the file is uploaded as a new SM version. To bootstrap from a
fresh clone, see `infra/secrets/README.md`.

## Usage

```bash
cd infra/terraform

# Local secrets (git-ignored). Edit values then:
terraform init
terraform plan
terraform apply
```

To rotate a value: edit `infra/secrets/secrets.json`, then
`terraform apply`. SM keeps a versioned history.
