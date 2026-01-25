//! HTTP client for the Poloniex Spot API.

use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use base64::Engine;
use hmac::{Hmac, Mac};
use reqwest::header::{HeaderMap, HeaderValue};
use reqwest::{Client as HttpClient, Method, StatusCode};
use serde::Deserialize;
use sha2::Sha256;
use thiserror::Error;
use tracing::{debug, warn};
use crate::config::ExchangeConfig;

/// Default receive window for signed requests in milliseconds.
const DEFAULT_RECEIVE_WINDOW: i64 = 5000;

/// Production Poloniex HTTP API endpoint.
const BASE_HTTP_API_URL: &str = "https://api.poloniex.com";

/// Default rate limit (requests per minute).
const DEFAULT_RATE_LIMIT: i64 = 200;

/// HTTP request timeout.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

/// Poloniex API error.
#[derive(Debug, Error)]
#[error("poloniex api error {code}: {message}")]
pub struct ApiError {
    pub code: i32,
    pub message: String,
}

/// Client errors.
#[derive(Debug, Error)]
pub enum ClientError {
    #[error("rate limit exceeded: {current}/{limit} per minute")]
    RateLimitExceeded { current: i64, limit: i64 },

    #[error("request error: {0}")]
    Request(#[from] reqwest::Error),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error(transparent)]
    Api(#[from] ApiError),
}

/// Result type for client operations.
pub type Result<T> = std::result::Result<T, ClientError>;

/// Configuration for creating a new Client.
pub struct ClientConfig {
    pub base_url: String,
    pub api_key: String,
    pub api_secret: String,
    pub rate_limit: i64,
    pub receive_window: i64,
}

impl ClientConfig {
    pub fn new(api_key: String, api_secret: String, rate_limit: i64) -> Self {
        Self {
            base_url: BASE_HTTP_API_URL.to_string(),
            api_key,
            api_secret,
            rate_limit: if rate_limit > 0 {
                rate_limit
            } else {
                DEFAULT_RATE_LIMIT
            },
            receive_window: DEFAULT_RECEIVE_WINDOW,
        }
    }
}

struct RateLimitState {
    window_start: Instant,
}

/// HTTP client for the Poloniex Spot API.
/// Handles request signing, rate limiting, and error handling.
pub struct Client {
    config: ClientConfig,
    http_client: HttpClient,
    request_count: AtomicI64,
    rate_limit_state: Mutex<RateLimitState>,
}

impl Client {
    /// Creates a new Poloniex API client.
    pub fn new(config: ClientConfig) -> Self {
        let http_client = HttpClient::builder()
            .timeout(REQUEST_TIMEOUT)
            .build()
            .expect("failed to build http client");

        Self {
            config,
            http_client,
            request_count: AtomicI64::new(0),
            rate_limit_state: Mutex::new(RateLimitState {
                window_start: Instant::now(),
            }),
        }
    }

    /// Creates a new Poloniex API client from exchange config.
    pub fn from_config(exchange_config: &ExchangeConfig) -> Self {
        let config = ClientConfig::new(
            exchange_config.api_key.clone(),
            exchange_config.api_secret.clone(),
            exchange_config.rate_limit.map(i64::from).unwrap_or(DEFAULT_RATE_LIMIT),
        );
        Self::new(config)
    }

    /// Creates an HMAC-SHA256 signature for Poloniex API.
    ///
    /// Signature format:
    /// - GET: METHOD\n/endpoint\nsignTimestamp=xxx&param1=val1&param2=val2 (sorted by ASCII)
    /// - POST/DELETE: METHOD\n/endpoint\nrequestBody=xxx&signTimestamp=xxx
    fn sign(&self, method: &Method, endpoint: &str, timestamp: i64, payload: &str) -> String {
        let sign_payload = if *method == Method::GET {
            // For GET: a payload already contains signTimestamp=xxx&params (sorted)
            if payload.is_empty() {
                format!("{}\n{}\nsignTimestamp={}", method.as_str(), endpoint, timestamp)
            } else {
                format!("{}\n{}\n{}", method.as_str(), endpoint, payload)
            }
        } else {
            // For POST/DELETE: METHOD\n/endpoint\nrequestBody=xxx&signTimestamp=xxx
            if payload.is_empty() {
                format!("{}\n{}\nsignTimestamp={}", method.as_str(), endpoint, timestamp)
            } else {
                format!(
                    "{}\n{}\nrequestBody={}&signTimestamp={}",
                    method.as_str(),
                    endpoint,
                    payload,
                    timestamp
                )
            }
        };

        let mut mac = Hmac::<Sha256>::new_from_slice(self.config.api_secret.as_bytes())
            .expect("HMAC can take key of any size");
        mac.update(sign_payload.as_bytes());
        let result = mac.finalize();

        base64::engine::general_purpose::STANDARD.encode(result.into_bytes())
    }

    /// Sends an HTTP request to the Poloniex API.
    /// If signed is true, the request will include authentication headers.
    pub async fn request(
        &self,
        method: Method,
        endpoint: &str,
        params: Option<HashMap<String, String>>,
        signed: bool,
    ) -> Result<Vec<u8>> {
        self.check_rate_limit()?;

        let mut params = params.unwrap_or_default();
        let timestamp = chrono::Utc::now().timestamp_millis();

        let (url, body, payload) = if method == Method::GET || method == Method::DELETE {
            // For GET/DELETE: add signTimestamp and sort params
            if signed {
                params.insert("signTimestamp".to_string(), timestamp.to_string());
            }

            // Sort parameters by key for consistent signing
            let mut sorted_params: Vec<_> = params.iter().collect();
            sorted_params.sort_by(|a, b| a.0.cmp(b.0));

            let payload: String = sorted_params
                .iter()
                .map(|(k, v)| format!("{}={}", k, urlencoding::encode(v)))
                .collect::<Vec<_>>()
                .join("&");

            let url = if payload.is_empty() {
                format!("{}{}", self.config.base_url, endpoint)
            } else {
                format!("{}{}?{}", self.config.base_url, endpoint, payload)
            };

            (url, None, payload)
        } else {
            // For POST: use JSON body
            let json_body = serde_json::to_string(&params)?;
            let url = format!("{}{}", self.config.base_url, endpoint);
            (url, Some(json_body.clone()), json_body)
        };

        let mut request = self.http_client.request(method.clone(), &url);

        if body.is_some() {
            request = request.header("Content-Type", "application/json");
            request = request.body(body.unwrap());
        }

        if signed {
            let signature = self.sign(&method, endpoint, timestamp, &payload);
            let mut headers = HeaderMap::new();
            headers.insert("key", HeaderValue::from_str(&self.config.api_key).unwrap());
            headers.insert(
                "signTimestamp",
                HeaderValue::from_str(&timestamp.to_string()).unwrap(),
            );
            headers.insert("signature", HeaderValue::from_str(&signature).unwrap());
            headers.insert("signatureMethod", HeaderValue::from_static("hmacSHA256"));
            headers.insert(
                "recvWindow",
                HeaderValue::from_str(&self.config.receive_window.to_string()).unwrap(),
            );
            request = request.headers(headers);
        }

        debug!(
            method = %method,
            endpoint = %endpoint,
            signed = signed,
            "sending request"
        );

        let response = request.send().await?;
        self.increment_request_count();

        let status = response.status();
        let body = response.bytes().await?;

        if status.is_client_error() || status.is_server_error() {
            return Err(self.parse_error_response(status, &body));
        }

        Ok(body.to_vec())
    }

    /// Verifies we haven't exceeded the rate limit.
    fn check_rate_limit(&self) -> Result<()> {
        let mut state = self.rate_limit_state.lock().unwrap();

        // Reset counter every minute
        // if state.window_start.elapsed() > Duration::from_secs(60) {
        if state.window_start.elapsed() > Duration::from_secs(5) {
            self.request_count.store(0, Ordering::SeqCst);
            state.window_start = Instant::now();
        }

        let current = self.request_count.load(Ordering::SeqCst);
        if current >= self.config.rate_limit {
            return Err(ClientError::RateLimitExceeded {
                current,
                limit: self.config.rate_limit,
            });
        }

        Ok(())
    }

    /// Increments the request counter.
    fn increment_request_count(&self) {
        self.request_count.fetch_add(1, Ordering::SeqCst);
    }

    /// Creates a ClientError from an error response.
    fn parse_error_response(&self, status: StatusCode, body: &[u8]) -> ClientError {
        #[derive(Deserialize)]
        struct ErrorResponse {
            code: Option<i32>,
            message: Option<String>,
        }

        let api_err = match serde_json::from_slice::<ErrorResponse>(body) {
            Ok(resp) => ApiError {
                code: resp.code.unwrap_or(status.as_u16() as i32),
                message: resp
                    .message
                    .unwrap_or_else(|| String::from_utf8_lossy(body).to_string()),
            },
            Err(_) => ApiError {
                code: status.as_u16() as i32,
                message: String::from_utf8_lossy(body).to_string(),
            },
        };

        warn!(code = api_err.code, message = %api_err.message, "api error");

        ClientError::Api(api_err)
    }

    /// Fetches the current server time from Poloniex.
    pub async fn get_server_time(&self) -> Result<chrono::DateTime<chrono::Utc>> {
        let body = self
            .request(Method::GET, "/timestamp", None, false)
            .await?;

        // Try parsing as a struct first
        #[derive(Deserialize)]
        struct ServerTimeResponse {
            #[serde(rename = "serverTime")]
            server_time: i64,
        }

        if let Ok(resp) = serde_json::from_slice::<ServerTimeResponse>(&body) {
            return Ok(chrono::DateTime::from_timestamp_millis(resp.server_time)
                .unwrap_or_default());
        }

        // Try parsing as a plain timestamp
        let timestamp: i64 = serde_json::from_slice(&body)?;
        Ok(chrono::DateTime::from_timestamp_millis(timestamp).unwrap_or_default())
    }

    /// Checks connectivity to Poloniex API by fetching server time.
    pub async fn ping(&self) -> Result<()> {
        self.get_server_time().await?;
        Ok(())
    }

    /// Returns the current request count in the window.
    pub fn request_count(&self) -> i64 {
        self.request_count.load(Ordering::SeqCst)
    }

    /// Returns the maximum requests per minute.
    pub fn rate_limit(&self) -> i64 {
        self.config.rate_limit
    }
}