use anyhow::{Context as _, Result};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use reqwest::Client;
use serde::Deserialize;
use std::time::Duration;

const METADATA_TOKEN_URL: &str =
    "http://metadata.google.internal/computeMetadata/v1/instance/service-accounts/default/token";

#[derive(Clone)]
pub struct SecretManagerClient {
    http: Client,
    project: String,
    secret: String,
}

#[derive(Deserialize)]
struct AccessToken {
    access_token: String,
}

#[derive(Deserialize)]
struct SecretAccessResponse {
    payload: SecretPayload,
}

#[derive(Deserialize)]
struct SecretPayload {
    data: String,
}

impl SecretManagerClient {
    pub fn new(project: String, secret: String) -> Result<Self> {
        anyhow::ensure!(valid_project_id(&project), "invalid GCP project id");
        anyhow::ensure!(valid_secret_id(&secret), "invalid Secret Manager secret id");
        Ok(Self {
            http: Client::builder().timeout(Duration::from_secs(15)).build()?,
            project,
            secret,
        })
    }

    pub async fn latest(&self) -> Result<String> {
        let response = self
            .http
            .get(format!(
                "https://secretmanager.googleapis.com/v1/projects/{}/secrets/{}/versions/latest:access",
                self.project, self.secret
            ))
            .bearer_auth(self.token().await?)
            .send()
            .await?
            .error_for_status()
            .context("7DTD server password lookup failed")?
            .json::<SecretAccessResponse>()
            .await?;
        decode_server_password(&response)
    }

    async fn token(&self) -> Result<String> {
        Ok(self
            .http
            .get(METADATA_TOKEN_URL)
            .header("Metadata-Flavor", "Google")
            .send()
            .await?
            .error_for_status()?
            .json::<AccessToken>()
            .await?
            .access_token)
    }
}

fn decode_server_password(response: &SecretAccessResponse) -> Result<String> {
    let decoded = STANDARD
        .decode(&response.payload.data)
        .context("7DTD server password was not valid base64")?;
    let password = String::from_utf8(decoded)
        .context("7DTD server password was not UTF-8")?
        .trim()
        .to_owned();
    anyhow::ensure!(
        !password.is_empty()
            && password.len() <= 128
            && password.chars().all(|character| character.is_ascii()
                && !character.is_control()
                && character != '`'),
        "7DTD server password contains unsupported characters"
    );
    Ok(password)
}

fn valid_project_id(value: &str) -> bool {
    !value.is_empty()
        && value.chars().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-'
        })
}

fn valid_secret_id(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_generated_server_password() {
        let response: SecretAccessResponse =
            serde_json::from_str(r#"{"payload":{"data":"MDEyMzQ1Njc4OWFiY2RlZg=="}}"#)
                .expect("response should parse");
        assert_eq!(
            decode_server_password(&response).expect("password should decode"),
            "0123456789abcdef"
        );
    }

    #[test]
    fn rejects_password_that_could_change_discord_markup() {
        let response: SecretAccessResponse =
            serde_json::from_str(r#"{"payload":{"data":"cGFzc3dvcmRg"}}"#)
                .expect("response should parse");
        assert!(decode_server_password(&response).is_err());
    }
}
