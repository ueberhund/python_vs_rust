import boto3
import json
import os
import datetime
import logging
from dateutil.relativedelta import relativedelta

logger = logging.getLogger()
logger.setLevel(logging.INFO)

THRESHOLD_AMOUNT = os.environ['THRESHOLD_AMOUNT']
NUM_SERVICES_TO_REPORT = 10       # make this value configurable at some point in the future

#Does the work to determine if any accounts are listed as not having a tax exempt status
def lambda_handler(event, context):
  
  #Calculate the first and last day of the previous month
  today = datetime.date.today()
  first = today.replace(day=1)
  lastMonth = first - datetime.timedelta(days=1)
  firstMonth = lastMonth.replace(day=1)
  
  #Get the list of accounts that are currently being taxed
  accounts = list_accounts()
  for item in accounts:
    logger.info('Analyzing account ' + item)
    totalCost, spend = calculate_top_cost_categories(item, firstMonth, lastMonth)
    logger.info('Total cost: ' + str(totalCost))
    if totalCost > float(THRESHOLD_AMOUNT):
      alert_by_account(item, spend, firstMonth, lastMonth, totalCost)
  
def alert_by_account(accountId, spend, firstMonth, lastMonth, totalCost):
  #Send alert via SNS
  message = "Below is the spend for account # {} from {} to {}\n".format(accountId, firstMonth, lastMonth)
  message += "Your account had a total monthly spend of ${:,.2f}\n".format(totalCost)
  message += "For your information, the following are the top-costing services in this account:\n\n"

  service_count = 0
  for svc in spend:
      if service_count <= NUM_SERVICES_TO_REPORT:
          message += " - {} : ${:,.2f}\n".format(svc['Service'], float(svc['Usage']))
      service_count += 1

  sns = boto3.client('sns')
  sns_topic_arn = os.environ['SNS_TOPIC_ARN']
  response = sns.publish(TopicArn=sns_topic_arn,
                       Subject="AWS Account #{} spend from {} - {}".format(accountId, firstMonth, lastMonth),
                       Message=message)
                         
def list_accounts():
  client = boto3.client('organizations')
  response = client.list_accounts()
  account_list = []
  for item in response['Accounts']:
    account_list.append(item['Id'])
    
  return account_list
  
  
def calculate_top_cost_categories(accountId, startDate, endDate):
  client = boto3.client('ce')
  response = client.get_cost_and_usage(
      TimePeriod={'Start':startDate.strftime("%Y-%m-%d"), 'End':endDate.strftime("%Y-%m-%d")}, 
      Granularity='MONTHLY',
      Filter={"Dimensions": {"Key":"LINKED_ACCOUNT", "Values":[accountId]}},
      Metrics=["UnblendedCost"],
      GroupBy=[{"Type":"DIMENSION", "Key":"SERVICE"}]
      )
  
  totalCost = 0
  services = []
  for item in response['ResultsByTime'][0]['Groups']:
    json_item = {"Service": item['Keys'][0], "Usage": item['Metrics']['UnblendedCost']['Amount'] }
    services.append(json_item)
    totalCost += float(item['Metrics']['UnblendedCost']['Amount'])
  
  services.sort(reverse=True, key=myFunc)
  return totalCost, services 

def myFunc(e):
  return float(e['Usage'])
