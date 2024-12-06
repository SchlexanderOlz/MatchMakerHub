use axum::http::{HeaderMap, HeaderValue};
use lazy_static::lazy_static;
use reqwest::{self, header::COOKIE};
use uuid::Uuid;
use rand::{distributions::Alphanumeric, Rng};


use crate::models::EZAUTHValidationResponse;

pub async fn validate_user(session_token: &str, ezauth_url: &str) -> Result<EZAUTHValidationResponse, Box<dyn std::error::Error>> {
    #[cfg(disable_auth)]
    {
        let random_id = Uuid::new_v4().to_string();
        let random_username: String = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(10)
            .map(char::from)
            .collect();
        let random_email = format!("{}@example.com", random_username);

        return Ok(EZAUTHValidationResponse {
            _id: random_id,
            username: random_username,
            email: random_email,
            created_at: "2021-01-01T00:00:00.000Z".to_string(),
        });
    }

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
