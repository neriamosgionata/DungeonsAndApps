# FIRST BOOT
# 1. bash infra/gen-secrets.sh          → creates infra/secrets.tfvars (git-ignored)
#    Fill: domain_name, github_repo, github_token, route53_zone_id (or "" to skip)
# 2. cd infra/bootstrap && terraform init && terraform apply
# 3. cd .. && terraform init
# 4. terraform apply -var-file=secrets.tfvars
#    → EC2 + S3 + SSM secrets + SSH keypair + GitHub secrets + DNS A record + TLS cert (if route53_zone_id set)
# 5. [no Route53 only] Add A record manually: domain → terraform output -raw public_ip
#                       then SSH in and run: sudo certbot --nginx -d YOUR_DOMAIN
# 6. GitHub → Settings → Environments → new "production" env
# 7. Push to master — CI deploys automatically

terraform {
  required_version = ">= 1.10"

  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = "~> 5.0"
    }
    tls = {
      source  = "hashicorp/tls"
      version = "~> 4.0"
    }
    github = {
      source  = "integrations/github"
      version = "~> 6.0"
    }
  }

  # bucket name must match bootstrap output: dungeonsandapps-tfstate-<ACCOUNT_ID>
  # run: aws sts get-caller-identity --query Account --output text
  backend "s3" {
    bucket       = "dungeonsandapps-tfstate-579241412831"
    key          = "prod/terraform.tfstate"
    region       = "eu-south-1"
    encrypt      = true
    use_lockfile = true
  }
}

locals {
  github_owner = split("/", var.github_repo)[0]
  github_name  = split("/", var.github_repo)[1]
}

provider "aws" {
  region = var.aws_region
}

provider "github" {
  token = var.github_token
  owner = local.github_owner
}
