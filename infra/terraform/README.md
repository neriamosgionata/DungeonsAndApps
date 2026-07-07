# infra/terraform

Terraform for the DungeonsAndApps infrastructure:

- `main.tf` — EC2 start/stop scheduler (Lambda + EventBridge rules).
- `variables.tf` — input variables.
- `secrets.tf` — AWS Secrets Manager secret `dungeonsandapps/prod` + IAM
  permissions for the EC2 role + CI runner.

## EC2 schedule (current AWS state)

| Event        | Cron (UTC)     | Local (Rome) |
|--------------|----------------|--------------|
| Stop at night | `0 22 * * ? *` | 00:00 CEST   |
| Start afternoon | `0 15 * * ? *` | 17:00 CEST |

If you change the schedule in AWS by hand, also update the
`schedule_expression` in `main.tf` so the next `terraform apply`
doesn't reset it. (Set `prevent_destroy` on the rules to make
terraform skip re-creating them on apply.)

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

## Modify Schedule

Edit `schedule_expression` in `main.tf`:

```hcl
schedule_expression = "cron(MIN HOUR DAY MONTH DAYOFWEEK YEAR)"
```

Examples:
- `cron(0 22 * * ? *)` = 22:00 UTC daily (00:00 CEST)
- `cron(0 10 * * ? *)` = 10:00 UTC daily (12:00 CEST)
