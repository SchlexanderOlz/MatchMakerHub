use serde::Deserialize;

#[derive(Deserialize, Clone, Debug)]
pub struct EZAUTHValidationResponse {
    pub _id: String,
    pub username: String,
    pub email: String,

    #[serde(rename = "createdAt")]
    pub created_at: String
}

