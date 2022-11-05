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

async fn download_object(client: &Client, bucket_name: &str, key: &str) -> ByteStream {
    let resp = client
        .get_object()
        .bucket(bucket_name)
        .key(key)
        .send()
        .await;
    resp.unwrap().body
}

fn get_aws_client(region: &str) -> Result<Client, Error> {
    let key_id = env::var(ENV_CRED_KEY_ID)
        .context("Missing S3_KEY_ID")?;
    let key_secret = env::var(ENV_CRED_KEY_SECRET)
        .context("Missing S3_KEY_SECRET")?;

    let cred = Credentials::new(key_id, key_secret, None, None, "loaded-from-custom-env");

    let region = Region::new(region.to_string());
    let conf_builder = config::Builder::new()
        .region(region)
        .credentials_provider(cred);
    let conf = conf_builder
        .build();

    let client = Client::from_conf(conf);

    Ok(client)
}

async fn func(event: LambdaEvent<Value>) -> Result<Value, Error> {
    let (event, _context) = event.into_parts();
    let region = env::var(REGION).context("Missing REGION")?;
    let bucket = env::var(BUCKET).context("Missing BUCKET")?;
    let file_path = env::var(FILE_PATH).context("Missing FILE_PATH")?;

    let client = get_aws_client(&region)?;
    let stream = download_object(&client, &bucket, &file_path).await;

    let b = stream
        .collect()
        .await
        .expect("error")
        .into_bytes();

    let s = str::from_utf8(&b)
        .unwrap().split("\n")
        .collect::<Vec<&str>>();
    
    let a = s
        .choose(&mut thread_rng())
        .copied()
        .unwrap_or_default();

    let out  = json!({
        "statusCode": 200,
        "headers": {
            "Content-Type": "application/json",
            "Access-Control-Allow-Methods": "'GET,OPTIONS'",
            "Access-Control-Allow-Headers": "'Content-Type,X-Amz-Date,Authorization,X-Api-Key,X-Amz-Security-Token'",
            "Access-Control-Allow-Origin": "https://example.com"
        },
        "body": format!("{{\"message\": \"{}\"}}",a.trim()),
        "isBase64Encoded": false
    });

    Ok(out)
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let func = service_fn(func);
    lambda_runtime::run(func).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use aws_sdk_s3::model::ObjectAttributes::*;

    #[test]
    fn validate_env_vars() -> Result<(), Error> {
        let region = env::var(REGION).context("Missing REGION")?;
        let bucket = env::var(BUCKET).context("Missing BUCKET")?;
        let file_path = env::var(FILE_PATH).context("Missing FILE_PATH")?;

        assert!(region!="");
        assert!(bucket!="");
        assert!(file_path!="");
        Ok(())
    }    

    #[test]
    fn valid_aws_client() -> Result<(), Error> {
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
    async fn downloaded_file_has_size() -> Result<(), Error> {
        let region = env::var(REGION).context("Missing REGION")?;
        let bucket = env::var(BUCKET).context("Missing BUCKET")?;
        let file_path = env::var(FILE_PATH).context("Missing FILE_PATH")?;

        let client = get_aws_client(&region)?;
        let stream = download_object(&client, &bucket, &file_path).await;
        assert!(stream.collect().await.expect("error").into_bytes().len()>0);
        Ok(())
    }
    
}