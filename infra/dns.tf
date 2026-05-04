locals {
  use_route53 = var.route53_zone_id != ""
}

data "aws_route53_zone" "main" {
  count   = local.use_route53 ? 1 : 0
  zone_id = var.route53_zone_id
}

resource "aws_route53_record" "app" {
  count   = local.use_route53 ? 1 : 0
  zone_id = var.route53_zone_id
  name    = var.domain_name
  type    = "A"
  ttl     = 60
  records = [aws_eip.app.public_ip]
}
