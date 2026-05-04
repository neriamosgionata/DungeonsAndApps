variable "aws_region" {
  description = "AWS region"
  type        = string
  default     = "eu-south-1"
}

variable "instance_type" {
  description = "EC2 instance type (ARM)"
  type        = string
  default     = "t4g.small"
}

variable "allowed_ssh_cidrs" {
  description = "CIDRs allowed to SSH. Empty = locked down (GitHub Actions uses SSM Session Manager instead)"
  type        = list(string)
  default     = []
}

variable "domain_name" {
  description = "Public domain name (e.g. dungeonsandapps.example.com)"
  type        = string
}

variable "jwt_secret" {
  description = "JWT signing secret (min 32 chars)"
  type        = string
  sensitive   = true
}

variable "db_password" {
  description = "Postgres password"
  type        = string
  sensitive   = true
}

variable "admin_password" {
  description = "Seeded admin account password"
  type        = string
  sensitive   = true
}

variable "github_token" {
  description = "GitHub personal access token with repo + secrets scope"
  type        = string
  sensitive   = true
}

variable "github_repo" {
  description = "GitHub repo in owner/name format (e.g. acme/dungeonsandapps)"
  type        = string
}
