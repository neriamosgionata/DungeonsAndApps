resource "aws_ssm_parameter" "jwt_secret" {
  name  = "/dungeonsandapps/prod/JWT_SECRET"
  type      = "SecureString"
  overwrite = true
  value = var.jwt_secret
  tags  = { Name = "dungeonsandapps-jwt-secret" }
}

resource "aws_ssm_parameter" "db_password" {
  name  = "/dungeonsandapps/prod/DB_PASSWORD"
  type      = "SecureString"
  overwrite = true
  value = var.db_password
  tags  = { Name = "dungeonsandapps-db-password" }
}

resource "aws_ssm_parameter" "admin_password" {
  name  = "/dungeonsandapps/prod/ADMIN_PASSWORD"
  type      = "SecureString"
  overwrite = true
  value = var.admin_password
  tags  = { Name = "dungeonsandapps-admin-password" }
}

resource "aws_ssm_parameter" "admin_email" {
  name      = "/dungeonsandapps/prod/ADMIN_EMAIL"
  type      = "String"
  overwrite = true
  value     = var.admin_email
  tags      = { Name = "dungeonsandapps-admin-email" }
}

resource "aws_ssm_parameter" "s3_access_key" {
  name  = "/dungeonsandapps/prod/S3_ACCESS_KEY"
  type      = "SecureString"
  overwrite = true
  value = aws_iam_access_key.app.id
  tags  = { Name = "dungeonsandapps-s3-access-key" }
}

resource "aws_ssm_parameter" "s3_secret_key" {
  name  = "/dungeonsandapps/prod/S3_SECRET_KEY"
  type      = "SecureString"
  overwrite = true
  value = aws_iam_access_key.app.secret
  tags  = { Name = "dungeonsandapps-s3-secret-key" }
}

resource "aws_ssm_parameter" "s3_public_url" {
  name  = "/dungeonsandapps/prod/S3_PUBLIC_URL"
  type      = "String"
  overwrite = true
  value = "https://${var.domain_name}/api/v1/files"
  tags  = { Name = "dungeonsandapps-s3-public-url" }
}
