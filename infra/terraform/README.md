# EC2 Auto-Shutdown Scheduler

Terraform module to automatically stop EC2 instances at 00:00 UTC and start them at 16:00 UTC.

## Usage

```bash
cd infra/terraform

# Set your instance IDs
cat > terraform.tfvars <<EOF
instance_ids = ["i-0123456789abcdef0"]
aws_region   = "us-east-1"
EOF

# Deploy
terraform init
terraform plan
terraform apply
```

## Schedule

- **Stop**: Every day at 00:00 UTC (24:00)
- **Start**: Every day at 16:00 UTC

## Architecture

- Lambda (Python) triggered by EventBridge rules
- IAM role with minimal permissions (EC2 start/stop/describe, CloudWatch logs)
- Instance IDs passed via env var

## Modify Schedule

Edit `schedule_expression` in `main.tf`:

```hcl
schedule_expression = "cron(MIN HOUR DAY MONTH DAYOFWEEK YEAR)"
```

Examples:
- `cron(0 0 * * ? *)` = 00:00 UTC daily
- `cron(0 12 * * ? *)` = 12:00 UTC daily
