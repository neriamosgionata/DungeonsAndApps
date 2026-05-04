terraform {
  required_version = ">= 1.7"

  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = "~> 5.0"
    }
  }

  backend "s3" {
    bucket         = "dungeonsandapps-tfstate"
    key            = "prod/terraform.tfstate"
    region         = "eu-south-1"
    encrypt        = true
    dynamodb_table = "dungeonsandapps-tflock"
  }
}

provider "aws" {
  region = var.aws_region
}
