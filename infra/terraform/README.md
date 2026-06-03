# EC2 Auto-Shutdown Scheduler

Terraform module to automatically stop EC2 instances at 00:00 CEST and start them at 12:00 CEST.

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

- **Stop**: Every day at 22:00 UTC (00:00 CEST; 23:00 CET winter)
- **Start**: Every day at 10:00 UTC (12:00 CEST; 11:00 CET winter)

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
- `cron(0 22 * * ? *)` = 22:00 UTC daily (00:00 CEST)
- `cron(0 10 * * ? *)` = 10:00 UTC daily (12:00 CEST)
