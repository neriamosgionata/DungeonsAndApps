output "instance_id" {
  description = "EC2 instance ID"
  value       = aws_instance.app.id
}

output "public_ip" {
  description = "Static public IP (EIP)"
  value       = aws_eip.app.public_ip
}

output "s3_bucket" {
  description = "S3 media bucket name"
  value       = aws_s3_bucket.media.id
}

output "domain_name" {
  description = "Configured domain name"
  value       = var.domain_name
}

output "s3_access_key_id" {
  description = "IAM access key for S3 (stored in SSM)"
  value       = aws_iam_access_key.app.id
  sensitive   = true
}

output "deploy_key_pair" {
  description = "EC2 key pair name (TF-managed)"
  value       = aws_key_pair.deploy.key_name
}

output "github_secrets_pushed" {
  description = "GitHub Actions secrets written by Terraform"
  value       = ["EC2_HOST", "EC2_SSH_KEY", "DEPLOY_DOMAIN"]
}
