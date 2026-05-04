# Run once with a local backend to create the S3 + DynamoDB used by the remote backend.
# After applying, add the backend "s3" block to main.tf and run `terraform init -migrate-state`.
#
# Usage:
#   terraform -chdir=infra/bootstrap apply
#
# This file is NOT part of the main infra module — kept here as reference.
# Create infra/bootstrap/ dir and copy this block there if needed.

# resource "aws_s3_bucket" "tfstate" {
#   bucket = "dungeonsandapps-tfstate"
#   tags   = { Name = "dungeonsandapps-tfstate" }
# }
#
# resource "aws_s3_bucket_versioning" "tfstate" {
#   bucket = aws_s3_bucket.tfstate.id
#   versioning_configuration { status = "Enabled" }
# }
#
# resource "aws_s3_bucket_server_side_encryption_configuration" "tfstate" {
#   bucket = aws_s3_bucket.tfstate.id
#   rule {
#     apply_server_side_encryption_by_default { sse_algorithm = "AES256" }
#   }
# }
#
# DynamoDB lock table removed — S3 native locking used instead (use_lockfile = true, TF >= 1.10)
