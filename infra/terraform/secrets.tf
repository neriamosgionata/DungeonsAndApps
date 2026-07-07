// =====================================================================
// AWS Secrets Manager — dungeonsandapps/prod
//
// Replaces the prior SSM /dungeonsandapps/prod/* parameter tree.
// The full secret payload (all key/value pairs) is read from a local
// JSON file at `infra/secrets/secrets.json` and uploaded as a single
// Secrets Manager secret. The IAM policy on the EC2 role
// (`dungeonsandapps-ec2-role`) is broadened so the deploy script can
// `aws secretsmanager get-secret-value` instead of `ssm get-parameter`.
//
// To (re)create the secret after editing the local JSON:
//   terraform apply
//
// To rotate a value: edit `infra/secrets/secrets.json`, run
//   terraform apply
// SM creates a new version on each apply.
// =====================================================================

# Read the local secrets JSON at plan time. The file is git-ignored
# (see .gitignore `infra/*.tfvars` and the secrets/ dir contents are
# expected to live outside the repo). To bootstrap from a fresh clone,
# copy infra/secrets/secrets.json.example to infra/secrets/secrets.json
# and fill in the values (or import them from SSM with the helper at
# infra/secrets/import-from-ssm.sh).
data "local_file" "secrets_json" {
  filename = "${path.module}/../secrets/secrets.json"
}

# The Secrets Manager secret. Stored as a single JSON blob
# (secretsmanager supports up to 65536 bytes — well above our 8 keys).
# Recovery window 0 = immediate delete on destroy (use
# `terraform destroy` to remove). For real rotation, raise this.
resource "aws_secretsmanager_secret" "prod" {
  name                    = "dungeonsandapps/prod"
  description             = "DungeonsAndApps production secrets (DB password, JWT, S3, admin, SSH key)."
  recovery_window_in_days = 0
}

resource "aws_secretsmanager_secret_version" "prod_v1" {
  secret_id     = aws_secretsmanager_secret.prod.id
  secret_string = data.local_file.secrets_json.content
}

# IAM policy for the EC2 instance role + CI runner to read the secret.
# The CI runner uses the GitHub Actions OIDC role (see .github/workflows/
# deploy.yml) and needs an inline or attached policy granting
# secretsmanager:GetSecretValue on this ARN.
data "aws_iam_policy_document" "ec2_sm_read" {
  statement {
    sid    = "ReadProdSecret"
    effect = "Allow"
    actions = [
      "secretsmanager:GetSecretValue",
      "secretsmanager:DescribeSecret",
    ]
    resources = [aws_secretsmanager_secret.prod.arn]
  }
}

resource "aws_iam_policy" "ec2_sm_read" {
  name   = "dungeonsandapps-ec2-secretsmanager-read"
  policy = data.aws_iam_policy_document.ec2_sm_read.json
}

# Attach to the EC2 role that was provisioned out-of-band (the
# instance profile `dungeonsandapps-ec2-profile` is referenced from
# the instance, not from this terraform). This is idempotent.
resource "aws_iam_role_policy_attachment" "ec2_sm_read" {
  role       = "dungeonsandapps-ec2-role"
  policy_arn = aws_iam_policy.ec2_sm_read.arn
}
