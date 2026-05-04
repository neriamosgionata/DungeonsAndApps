resource "tls_private_key" "deploy" {
  algorithm = "RSA"
  rsa_bits  = 4096
}

resource "aws_key_pair" "deploy" {
  key_name   = "dungeonsandapps-deploy"
  public_key = tls_private_key.deploy.public_key_openssh
  tags       = { Name = "dungeonsandapps-deploy" }
}

resource "aws_ssm_parameter" "ssh_private_key" {
  name  = "/dungeonsandapps/prod/SSH_PRIVATE_KEY"
  type  = "SecureString"
  value = tls_private_key.deploy.private_key_pem
  tags  = { Name = "dungeonsandapps-ssh-key" }
}

data "github_repository" "repo" {
  full_name = var.github_repo
}

resource "github_actions_secret" "ec2_host" {
  repository      = local.github_name
  secret_name     = "EC2_HOST"
  plaintext_value = aws_eip.app.public_ip
}

resource "github_actions_secret" "ec2_ssh_key" {
  repository      = local.github_name
  secret_name     = "EC2_SSH_KEY"
  plaintext_value = tls_private_key.deploy.private_key_openssh
}

resource "github_actions_secret" "deploy_domain" {
  repository      = local.github_name
  secret_name     = "DEPLOY_DOMAIN"
  plaintext_value = var.domain_name
}
