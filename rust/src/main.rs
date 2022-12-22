use chrono::{NaiveDate, Datelike, Utc};
use date_calculations::*;
use lambda_runtime::{service_fn, LambdaEvent, Error};
use serde_json::{json, Value};
use aws_config::{meta::region::RegionProviderChain, SdkConfig};
use aws_sdk_organizations;
use aws_sdk_costexplorer;
use aws_sdk_sns;
//use log::{info, error};


#[tokio::main]
async fn main() -> Result<(), Error> {
    let func = service_fn(func);
    lambda_runtime::run(func).await?;

    Ok(())
}

async fn func(_event: LambdaEvent<Value>) -> Result<Value, Error> {
    
    //Set up the standard AWS client
    let region_provider = RegionProviderChain::default_provider().or_else("us-east-1");
    let config = aws_config::from_env().region(region_provider).load().await;
    
    //Environment variables
    let threshold_amount = std::env::var("THRESHOLD_AMOUNT").unwrap();
    
    let now = Utc::now();
    let first_of_this_month = NaiveDate::from_ymd_opt(now.year(), now.month(), 1).unwrap();
    let first_of_report_month = previous_month(&first_of_this_month).unwrap();
    let last_of_report_month = end_of_month(&first_of_report_month).unwrap();
    
    //Get the list of accounts that are currently being taxed
    let account_list = list_accounts(&config).await;
    let accounts = account_list.unwrap();
    
    for account in accounts.accounts().unwrap() {
        let account_id = account.id().unwrap().to_string();
        println!("Analyzing account: {:?}", account_id);
        
        let cc_result = calculate_top_cost_categories(&config, &account_id, &first_of_report_month, &last_of_report_month).await.unwrap();
        
        let service_list = cc_result.clone();       //Necessary so we don't get a borrowed after move error
        let mut total_cost : f32 = 0.0;
        
        for (_service, cost) in cc_result.into_iter() {
            total_cost += cost;
        }
        
        if total_cost > threshold_amount.parse::<f32>().unwrap() {
            println!("Send email!");
            let _message = alert_by_account(&config, &account_id, &service_list, &first_of_report_month, &last_of_report_month, &total_cost).await;
            //let result = match _message {
            //    Ok(()) => println!("Everything worked!"),
            //    Err(error) => println!("Error {:?}", error),
            //};
        }
        
    }
    
    Ok(json!({"Result": "Ok"}))
}

async fn alert_by_account(config: &SdkConfig, account_id: &String, services: &Vec<(String, f32)>, first_of_report_month: &NaiveDate, last_of_report_month: &NaiveDate, total_cost: &f32) -> Result<(), Error> {
    let line1 = format!("Below is the spend for account # {} from {} to {}\n", account_id, first_of_report_month, last_of_report_month);
    let line2 = format!("Your account had a total monthly spend of ${:.2}\n", total_cost);
    let line3 = format!("For your information, the following are the top-costing services in this account:\n\n");
    
    let mut message = line1.to_owned();
    message.push_str(&line2);
    message.push_str(&line3);
    
    //Environment variables
    let num_services_to_report = 10;                            //Make this configurable at some point in the future
    let topic_arn = std::env::var("SNS_TOPIC_ARN").unwrap();
    
    let mut service_count = 0;
    for (service, cost) in services.into_iter() {
        if service_count <= num_services_to_report {
            let service_item = format!(" - {} - ${:.2}\n", service, cost);
            message.push_str(&service_item);
            service_count += 1;
        }
    }
    
    let subject = format!("AWS Account #{} spend from {} - {}", account_id, first_of_report_month, last_of_report_month);
    
    let sns_client = aws_sdk_sns::Client::new(&config);
    let _resp = sns_client
        .publish()
        .topic_arn(topic_arn)
        .subject(subject)
        .message(message)
        .send()
        .await?;

    Ok(())    
}

async fn list_accounts(config: &SdkConfig) -> Result<aws_sdk_organizations::output::ListAccountsOutput, Error> {
    let org_client = aws_sdk_organizations::Client::new(&config);
    let resp = org_client.list_accounts().send().await?;
    
    Ok(resp)
}

async fn calculate_top_cost_categories(config: &SdkConfig, account_id: &str, start_date: &NaiveDate, end_date: &NaiveDate) -> Result<Vec<(String, f32)>, Error> {
    let cost_usage_client = aws_sdk_costexplorer::Client::new(&config);
    
    let db = aws_sdk_costexplorer::model::DateInterval::builder()
        .start(start_date.format("%Y-%m-%d").to_string())
        .end(end_date.format("%Y-%m-%d").to_string())
        .build();
    
    let dv = aws_sdk_costexplorer::model::DimensionValues::builder()
        .key(aws_sdk_costexplorer::model::Dimension::LinkedAccount)
        .values(account_id)
        .build();
    
    let exp = aws_sdk_costexplorer::model::Expression::builder()
        .dimensions(dv)
        .build();
        
    let gb = aws_sdk_costexplorer::model::GroupDefinition::builder()
        .r#type(aws_sdk_costexplorer::model::GroupDefinitionType::Dimension)
        .key("SERVICE")
        .build();
    
    let resp = cost_usage_client
        .get_cost_and_usage()
        .time_period(db)
        .granularity(aws_sdk_costexplorer::model::Granularity::Monthly)
        .filter(exp)
        .metrics("UnblendedCost")
        .group_by(gb)
        .send()
        .await?;
        
    let mut sorted: Vec<(String, f32)> = Vec::new();
    
    let cc_result_time = resp.results_by_time().unwrap();
    for item in cc_result_time {
        //Loop over the results and store in a vector
        for sub_item in item.groups().unwrap() {
            
            let keys = sub_item.keys().unwrap();
            let service_name = (keys)[0].to_string();
            let service_cost = sub_item.metrics().unwrap().get("UnblendedCost").unwrap().amount.to_owned().unwrap().to_string().parse::<f32>().unwrap();
            
            sorted.push((service_name, service_cost));
        }
    }
    
    //Sort the vector by the cost in descending order
    sorted.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
    sorted.reverse();
    
    Ok(sorted)
}