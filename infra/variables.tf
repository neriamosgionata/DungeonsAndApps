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

variable "key_pair_name" {
  description = "EC2 key pair name for SSH access"
  type        = string
}

variable "allowed_ssh_cidrs" {
  description = "CIDRs allowed to SSH into the instance"
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
