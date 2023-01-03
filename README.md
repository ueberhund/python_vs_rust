# python_vs_rust
Provides some simple comparisons on how a python-based Lambda function looks in Rust

Recommend you use the directions [here](https://github.com/awslabs/aws-lambda-rust-runtime) to build the solution

Both functions are expecting 2 environment variables:
- `THRESHOLD_AMOUNT` - the function alerts when the total AWS bill is above this amount. This variable should be a value, like `100`.
- `SNS_TOPIC_ARN` - the ARN of the SNS topic to send to when the total bill exceeds the value in `THRESHOLD_AMOUNT`. This variable should be an SNS ARN, like `arn:aws:sns:us-east-1:1234567890:billing-alert`.
