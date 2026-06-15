terraform {
  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = "~> 5.0"
    }
  }
}

provider "aws" {
  region = var.aws_region
}

# IAM role for Lambda
resource "aws_iam_role" "ec2_scheduler" {
  name = "ec2-scheduler-lambda-role"

  assume_role_policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Action = "sts:AssumeRole"
      Effect = "Allow"
      Principal = {
        Service = "lambda.amazonaws.com"
      }
    }]
  })
}

resource "aws_iam_role_policy" "ec2_scheduler" {
  name = "ec2-scheduler-policy"
  role = aws_iam_role.ec2_scheduler.id

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Effect = "Allow"
        Action = [
          "logs:CreateLogGroup",
          "logs:CreateLogStream",
          "logs:PutLogEvents"
        ]
        Resource = "arn:aws:logs:${var.aws_region}:*:log-group:/aws/lambda/ec2-scheduler:*"
      },
      {
        Effect = "Allow"
        Action = [
          "ec2:StartInstances",
          "ec2:StopInstances",
          "ec2:DescribeInstances"
        ]
        Resource = "arn:aws:ec2:${var.aws_region}:*:instance/*"
        Condition = {
          StringEquals = {
            "aws:ResourceTag/Schedule": "true"
          }
        }
      }
    ]
  })
}

# Lambda function - auto-discovers instances by tag
resource "aws_lambda_function" "ec2_scheduler" {
  filename         = data.archive_file.lambda_zip.output_path
  source_code_hash = data.archive_file.lambda_zip.output_base64sha256
  function_name    = "ec2-scheduler"
  role             = aws_iam_role.ec2_scheduler.arn
  handler          = "index.handler"
  runtime          = "python3.11"
  timeout          = 30

  environment {
    variables = {
      SCHEDULE_TAG_KEY   = "Schedule"
      SCHEDULE_TAG_VALUE = "true"
    }
  }
}

# Lambda code archive
data "archive_file" "lambda_zip" {
  type        = "zip"
  output_path = "${path.module}/lambda.zip"

  source {
    filename = "index.py"
    content  = <<-EOF
import json
import os
import boto3

ec2 = boto3.client('ec2')

def handler(event, context):
    tag_key = os.environ.get('SCHEDULE_TAG_KEY', 'Schedule')
    tag_value = os.environ.get('SCHEDULE_TAG_VALUE', 'true')
    action = event.get('action', 'stop')
    
    # Find instances by tag
    response = ec2.describe_instances(
        Filters=[
            {'Name': f'tag:{tag_key}', 'Values': [tag_value]},
            {'Name': 'instance-state-name', 'Values': ['running', 'stopped', 'stopping', 'pending']}
        ]
    )
    
    instance_ids = []
    for reservation in response['Reservations']:
        for instance in reservation['Instances']:
            instance_ids.append(instance['InstanceId'])
    
    if not instance_ids:
        return {'statusCode': 200, 'body': 'No instances found with tag Schedule=true'}
    
    if action == 'stop':
        # Only stop running instances
        running = ec2.describe_instances(
            InstanceIds=instance_ids,
            Filters=[{'Name': 'instance-state-name', 'Values': ['running']}]
        )
        to_stop = []
        for r in running['Reservations']:
            for i in r['Instances']:
                to_stop.append(i['InstanceId'])
        
        if to_stop:
            ec2.stop_instances(InstanceIds=to_stop)
            return {'statusCode': 200, 'body': f'Stopped: {to_stop}'}
        return {'statusCode': 200, 'body': 'No running instances to stop'}
        
    elif action == 'start':
        # Only start stopped instances
        stopped = ec2.describe_instances(
            InstanceIds=instance_ids,
            Filters=[{'Name': 'instance-state-name', 'Values': ['stopped']}]
        )
        to_start = []
        for r in stopped['Reservations']:
            for i in r['Instances']:
                to_start.append(i['InstanceId'])
        
        if to_start:
            ec2.start_instances(InstanceIds=to_start)
            return {'statusCode': 200, 'body': f'Started: {to_start}'}
        return {'statusCode': 200, 'body': 'No stopped instances to start'}
    else:
        return {'statusCode': 400, 'body': f'Unknown action: {action}'}
EOF
  }
}

# CloudWatch Event Rules (EventBridge) — Rome TZ (CEST/CET)
# Stop at 22:00 UTC = 00:00 CEST (23:00 CET winter, close enough)
resource "aws_cloudwatch_event_rule" "stop_ec2" {
  name                = "stop-ec2-midnight"
  description         = "Stop EC2 instances at 22:00 UTC (00:00 CEST)"
  schedule_expression = "cron(0 22 * * ? *)"
}

resource "aws_cloudwatch_event_target" "stop_ec2" {
  rule      = aws_cloudwatch_event_rule.stop_ec2.name
  target_id = "StopEC2Lambda"
  arn       = aws_lambda_function.ec2_scheduler.arn

  input = jsonencode({ action = "stop" })
}

# Start at 14:00 UTC = 16:00 Rome (CEST)
resource "aws_cloudwatch_event_rule" "start_ec2" {
  name                = "start-ec2-afternoon"
  description         = "Start EC2 instances at 14:00 UTC (16:00 CEST)"
  schedule_expression = "cron(0 14 * * ? *)"
}

resource "aws_cloudwatch_event_target" "start_ec2" {
  rule      = aws_cloudwatch_event_rule.start_ec2.name
  target_id = "StartEC2Lambda"
  arn       = aws_lambda_function.ec2_scheduler.arn

  input = jsonencode({ action = "start" })
}

# Lambda permissions for EventBridge
resource "aws_lambda_permission" "allow_stop" {
  statement_id  = "AllowExecutionFromEventBridgeStop"
  action        = "lambda:InvokeFunction"
  function_name = aws_lambda_function.ec2_scheduler.function_name
  principal     = "events.amazonaws.com"
  source_arn    = aws_cloudwatch_event_rule.stop_ec2.arn
}

resource "aws_lambda_permission" "allow_start" {
  statement_id  = "AllowExecutionFromEventBridgeStart"
  action        = "lambda:InvokeFunction"
  function_name = aws_lambda_function.ec2_scheduler.function_name
  principal     = "events.amazonaws.com"
  source_arn    = aws_cloudwatch_event_rule.start_ec2.arn
}
