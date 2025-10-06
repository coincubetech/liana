use super::ServiceProvider;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt;

const MELD_API_BASE_URL: &str = "https://api-sb.meld.io/crypto/session/widget";
const MELD_AUTH_HEADER: &str = "BASIC WePYLDhjQ9xBCsedwgRGm5:3Jg4JnemxqoBPHTbHtcMuszbhkGHQmh";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MeldSessionRequest<'a> {
    pub session_data: SessionData<'a>,
    pub session_type: &'a str,
    pub external_customer_id: &'a str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionData<'a> {
    pub wallet_address: &'a str,
    pub country_code: &'a str,
    pub source_currency_code: &'a str,
    pub source_amount: &'a str,
    pub destination_currency_code: &'a str,
    pub service_provider: &'a str,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MeldSessionResponse {
    pub customer_id: Option<String>,
    pub external_customer_id: Option<String>,
    pub external_session_id: Option<String>,
    pub id: String,
    pub token: Option<String>,
    pub widget_url: String,
}

#[derive(Debug)]
pub enum MeldError {
    Network(reqwest::Error),
    Serialization(serde_json::Error),
    Api(String),
}

impl fmt::Display for MeldError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MeldError::Network(e) => write!(f, "Network error: {}", e),
            MeldError::Serialization(e) => write!(f, "Serialization error: {}", e),
            MeldError::Api(msg) => fmt::Display::fmt(msg, f),
        }
    }
}

impl Error for MeldError {}

impl From<reqwest::Error> for MeldError {
    fn from(error: reqwest::Error) -> Self {
        MeldError::Network(error)
    }
}

impl From<serde_json::Error> for MeldError {
    fn from(error: serde_json::Error) -> Self {
        MeldError::Serialization(error)
    }
}

#[derive(Clone)]
pub struct MeldClient {
    client: reqwest::Client,
}

impl MeldClient {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }

    pub async fn create_widget_session(
        &self,
        wallet_address: &str,
        country_code: &str,
        source_amount: &str,
        service_provider: ServiceProvider,
        network: liana::miniscript::bitcoin::Network,
    ) -> Result<String, MeldError> {
        tracing::info!("Creating Meld session with network: {:?}", network,);

        // Generate unique customer ID for each request to ensure fresh sessions
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| MeldError::Api(format!("System time error: {}", e)))?
            .as_secs();
        let unique_customer_id = format!("liana_user_{}", timestamp);

        let request = MeldSessionRequest {
            session_data: SessionData {
                wallet_address,
                country_code,
                source_currency_code: "USD",
                source_amount,
                destination_currency_code: "BTC",
                service_provider: service_provider.as_str(),
            },
            session_type: "BUY",
            external_customer_id: &unique_customer_id,
        };

        // Debug logging
        tracing::info!("Sending request to: {}", MELD_API_BASE_URL);
        tracing::info!("Request body: {:?}", &request);

        let response = self
            .client
            .post(MELD_API_BASE_URL)
            .header("Authorization", MELD_AUTH_HEADER)
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;

        if response.status().is_success() {
            let session_response: MeldSessionResponse = response.json().await?;
            tracing::info!("Meld API response: {:?}", session_response);

            Ok(session_response.widget_url)
        } else {
            #[derive(Deserialize, Debug)]
            struct MeldErrorMessageExtract {
                message: String,
            }

            let status = response.status();
            let error_text = response.json::<MeldErrorMessageExtract>().await.ok();

            tracing::error!("Meld API error: HTTP {}: {:?}", status, error_text);
            Err(MeldError::Api(
                error_text
                    .map(|e| e.message)
                    .unwrap_or("Unknown Meld API Error".to_string()),
            ))
        }
    }
}

impl Default for MeldClient {
    fn default() -> Self {
        Self::new()
    }
}
