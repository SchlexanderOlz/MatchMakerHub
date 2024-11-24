use axum::http::{HeaderMap, HeaderValue};
use lazy_static::lazy_static;
use reqwest::{self, header::COOKIE};

use crate::models::EZAUTHValidationResponse;

pub async fn validate_user(session_token: &str, ezauth_url: &str) -> Result<EZAUTHValidationResponse, Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();

    let response = client
        .get(&format!("{ezauth_url}/profile"))
        .header(
            COOKIE,
            HeaderValue::from_str(format!("session={session_token}").as_str()).unwrap(),
        )
        .send()
        .await?;

    if response.status().is_client_error() {
        return Err(Box::new(std::io::Error::new(
            std::io::ErrorKind::Other,
            response.text().await?,
        )));
    }

    let parsed: EZAUTHValidationResponse = response.json().await?;

    Ok(parsed)
}
