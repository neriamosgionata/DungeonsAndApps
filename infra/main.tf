# FIRST BOOT
# 1. cd infra/bootstrap && terraform init && terraform apply
# 2. cd infra && terraform init
# 3. terraform apply \
#      -var="domain_name=YOUR_DOMAIN" \
#      -var="github_repo=OWNER/REPO" \
#      -var="github_token=ghp_..." \
#      -var="jwt_secret=$(openssl rand -hex 32)" \
#      -var="db_password=$(openssl rand -hex 16)" \
#      -var="admin_password=$(openssl rand -hex 16)"
# 4. Add DNS A record: YOUR_DOMAIN → terraform output -raw public_ip
# 5. SSH in and run: sudo certbot --nginx -d YOUR_DOMAIN
#    (SSH key: aws ssm get-parameter --name /dungeonsandapps/prod/SSH_PRIVATE_KEY --with-decryption --query Parameter.Value --output text)
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

  backend "s3" {
    bucket       = "dungeonsandapps-tfstate"
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
