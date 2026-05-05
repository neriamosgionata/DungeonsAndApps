data "aws_ami" "amazon_linux_arm" {
  most_recent = true
  owners      = ["amazon"]

  filter {
    name   = "name"
    values = ["al2023-ami-*-arm64"]
  }

  filter {
    name   = "architecture"
    values = ["arm64"]
  }
}

resource "aws_spot_instance_request" "app" {
  ami                    = data.aws_ami.amazon_linux_arm.id
  instance_type          = var.instance_type
  key_name               = aws_key_pair.deploy.key_name
  subnet_id              = tolist(data.aws_subnets.default.ids)[0]
  vpc_security_group_ids = [aws_security_group.app.id]
  iam_instance_profile   = aws_iam_instance_profile.ec2.name

  spot_price                      = var.spot_price
  wait_for_fulfillment            = true
  instance_interruption_behavior  = "stop"

  root_block_device {
    volume_type           = "gp3"
    volume_size           = 10
    delete_on_termination = false
    encrypted             = true

    tags = { Name = "dungeonsandapps-root" }
  }

  user_data = templatefile("${path.module}/userdata.sh.tpl", {
    domain       = var.domain_name
    aws_region   = var.aws_region
    s3_bucket    = aws_s3_bucket.media.id
    use_route53  = local.use_route53
  })

  tags = {
    Name = "dungeonsandapps-prod"
  }

  lifecycle {
    ignore_changes = [user_data, ami]
  }
}
