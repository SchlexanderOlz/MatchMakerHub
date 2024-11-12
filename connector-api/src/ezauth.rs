use axum::http::{HeaderMap, HeaderValue};
use lazy_static::lazy_static;
use reqwest::{self, header::COOKIE};

use crate::models::EZAUTHValidationResponse;

pub fn validate_user(session_token: &str, ezauth_url: &str) -> Option<EZAUTHValidationResponse> {
    let client = reqwest::blocking::Client::new();

    let response = client
        .get(&format!("{ezauth_url}/profile"))
        .header(
            COOKIE,
            HeaderValue::from_str(format!("session={session_token}").as_str()).unwrap(),
        )
        .send()
        .unwrap();

    if response.status().is_client_error() {
        return None;
    }


    let parsed: EZAUTHValidationResponse = response.json().unwrap();

    Some(parsed)
}
