#![allow(unused)]

use std::{env, str};
use anyhow::Context;
use rand::prelude::*;
use serde_json::{json, Value};
use aws_sdk_s3::{Client, Credentials, Region, config, types::ByteStream};
use lambda_runtime::{service_fn, LambdaEvent, Error};

const ENV_CRED_KEY_ID: &str = "S3_KEY_ID";
const ENV_CRED_KEY_SECRET: &str = "S3_KEY_SECRET";
const BUCKET: &str = "BUCKET";
const REGION: &str = "REGION";
const FILE_PATH: &str = "FILE_PATH";

async fn download_object(client: &Client, bucket_name: &str, key: &str) -> Result<ByteStream, Error> {
    let resp = client
        .get_object()
        .bucket(bucket_name)
        .key(key)
        .send()
        .await?;
    
    Ok(resp.body)
}

fn get_aws_client(region: &str) -> Result<Client, Error> {
    let key_id = env::var(ENV_CRED_KEY_ID).context("Missing S3_KEY_ID")?;
    let key_secret = env::var(ENV_CRED_KEY_SECRET).context("Missing S3_KEY_SECRET")?;

    let cred = Credentials::new(
        key_id, 
        key_secret, 
        None, 
        None, 
        "loaded-from-custom-env");

    let region = Region::new(region.to_string());

    let conf_builder = config::Builder::new()
        .region(region)
        .credentials_provider(cred);

    let conf = conf_builder.build();

    let client = Client::from_conf(conf);

    Ok(client)
}

async fn convert_bytes_to_string(stream: ByteStream) -> Result<Vec<String>, Error> {
    let b = stream
        .collect()
        .await?
        .into_bytes();

    let mut s = str::from_utf8(&b)?
        .split("\n")
        .map(|x| x.to_string())
        .collect::<Vec<String>>();

    Ok(s)
}

fn get_random_line(text: Vec<String>) -> Result<String, String> {
    let line = text
        .choose(&mut thread_rng());
        
    let retline = match line {
        Some(x) => x.to_owned(),
        None => return Err("Failed to get random line.".to_string())
    };

    Ok(retline)
}

async fn func(event: LambdaEvent<Value>) -> Result<Value, Error> {
    let (event, _context) = event.into_parts();
    let region = env::var(REGION).context("Missing REGION")?;
    let bucket = env::var(BUCKET).context("Missing BUCKET")?;
    let file_path = env::var(FILE_PATH).context("Missing FILE_PATH")?;

    let client = get_aws_client(&region)?;
    let stream = download_object(&client, &bucket, &file_path).await?;

    let text = convert_bytes_to_string(stream).await?;
    let line = get_random_line(text)?;

    let out  = json!({
        "statusCode": 200,
        "headers": {
            "Content-Type": "application/json",
            "Access-Control-Allow-Methods": "'GET,OPTIONS'",
            "Access-Control-Allow-Headers": "'Content-Type,X-Amz-Date,Authorization,X-Api-Key,X-Amz-Security-Token'",
            "Access-Control-Allow-Origin": "https://example.com"
        },
        "body": format!("{{\"message\": \"{}\"}}",line.trim()),
        "isBase64Encoded": false
    });

    Ok(out)
}

#[tokio::main]
async fn main() -> Result<(), lambda_runtime::Error> {
    let func = service_fn(func);
    lambda_runtime::run(func).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use aws_sdk_s3::{model::ObjectAttributes::*, client::fluent_builders::GetObjectAttributes, output::GetObjectAttributesOutput, types::ByteStream};

    #[test]
    fn validate_env_vars() -> Result<(), Error> {
        let region = env::var(REGION).context("Missing REGION")?;
        let bucket = env::var(BUCKET).context("Missing BUCKET")?;
        let file_path = env::var(FILE_PATH).context("Missing FILE_PATH")?;

        assert_ne!(region,"");
        assert_ne!(bucket,"");
        assert_ne!(file_path,"");
        Ok(())
    }    

    #[tokio::test]
    async fn valid_aws_client() -> Result<(), Error> {
        let region = env::var(REGION).context("Missing REGION")?;
        let client = get_aws_client(&region)?;
        Ok(())
    }

    #[tokio::test]
    async fn file_size_not_zero() -> Result<(), Error> {
        let region = env::var(REGION).context("Missing REGION")?;
        let bucket = env::var(BUCKET).context("Missing BUCKET")?;
        let file_path = env::var(FILE_PATH).context("Missing FILE_PATH")?;
        
        let client = get_aws_client(&region)?;
        let file_attributes = client
        .get_object_attributes()
        .bucket(bucket)
        .key(file_path)
        .object_attributes(ObjectSize)
        .send()
        .await;
        assert!(file_attributes.unwrap().object_size()>0);
        Ok(())
    }

    
    #[tokio::test]
    async fn downloaded_file_size_check() -> Result<(), Error> {
        let region = env::var(REGION).context("Missing REGION")?;
        let bucket = env::var(BUCKET).context("Missing BUCKET")?;
        let file_path = env::var(FILE_PATH).context("Missing FILE_PATH")?;

        let client = get_aws_client(&region)?;
        let file_attributes = client
            .get_object_attributes()
            .bucket(&bucket)
            .key(&file_path)
            .object_attributes(ObjectSize)
            .send()
            .await;

        let size = file_attributes.unwrap().object_size();
        let stream = download_object(&client, &bucket, &file_path).await?;
        let bytes = stream.collect().await.expect("Failed to get bytes from ByteStream").into_bytes();

        assert_eq!(bytes.len() as i64,size);
        Ok(())
    }

    #[tokio::test]
    async fn deserialize_stream() {
        let string = "This is a\ntest string.\nIt will get\nturned into\nan array.";
        let bytes = ByteStream::from_static(string.as_bytes());
        
        let deserialized = convert_bytes_to_string(bytes).await;
        assert_eq!(string
            .split("\n")
            .map(|x| x.to_string())
            .collect::<Vec<String>>()
            ,deserialized.expect("Error"));
    }

    #[test]
    fn random_line() {
        let string = "This is a\ntest string.\nIt will get\nturned into\nan array.";

        let str_array = string
            .split("\n")
            .map(|x| x.to_string())
            .collect::<Vec<String>>();

        //get line
        let s1 = get_random_line(str_array.clone()).expect("Error");
        assert!(str_array.contains(&s1));
        
        //random?
        let mut i = 10;
        let mut s2 = "".to_string();
        while i > 0 {
            s2 = get_random_line(str_array.clone()).expect("Error");
            if (s1==s2) {
                i = i-1;
            }
            else {
                assert!(true);
                break;
            }
        }
    }    
    
}