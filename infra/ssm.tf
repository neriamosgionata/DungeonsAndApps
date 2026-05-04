resource "aws_ssm_parameter" "jwt_secret" {
  name  = "/dungeonsandapps/prod/JWT_SECRET"
  type  = "SecureString"
  value = var.jwt_secret
  tags  = { Name = "dungeonsandapps-jwt-secret" }
}

resource "aws_ssm_parameter" "db_password" {
  name  = "/dungeonsandapps/prod/DB_PASSWORD"
  type  = "SecureString"
  value = var.db_password
  tags  = { Name = "dungeonsandapps-db-password" }
}

resource "aws_ssm_parameter" "admin_password" {
  name  = "/dungeonsandapps/prod/ADMIN_PASSWORD"
  type  = "SecureString"
  value = var.admin_password
  tags  = { Name = "dungeonsandapps-admin-password" }
}

resource "aws_ssm_parameter" "s3_access_key" {
  name  = "/dungeonsandapps/prod/S3_ACCESS_KEY"
  type  = "SecureString"
  value = aws_iam_access_key.app.id
  tags  = { Name = "dungeonsandapps-s3-access-key" }
}

resource "aws_ssm_parameter" "s3_secret_key" {
  name  = "/dungeonsandapps/prod/S3_SECRET_KEY"
  type  = "SecureString"
  value = aws_iam_access_key.app.secret
  tags  = { Name = "dungeonsandapps-s3-secret-key" }
}
