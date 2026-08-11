//! Outbound transactional email, abstracted behind [`Mailer`].

use serde::Serialize;

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone)]
pub struct Message {
    pub to: String,
    pub subject: String,
    pub text: String,
    pub html: String,
}

/// Configured outbound mail backend.
#[derive(Clone)]
pub enum Mailer {
    /// Dev/default: log the message (including reset links) instead of sending.
    Log { from: MailAddress },
    /// Cloudflare Email Sending REST API.
    Cloudflare(CloudflareMailer),
}

#[derive(Debug, Clone)]
pub struct MailAddress {
    pub address: String,
    pub name: Option<String>,
}

#[derive(Clone)]
pub struct CloudflareMailer {
    client: reqwest::Client,
    account_id: String,
    api_token: String,
    from: MailAddress,
}

impl Mailer {
    pub fn from_env() -> Result<Self, Box<dyn std::error::Error>> {
        let from = MailAddress {
            address: required_env("MAIL_FROM").unwrap_or_else(|_| "noreply@localhost".into()),
            name: optional_env("MAIL_FROM_NAME"),
        };
        let provider = optional_env("MAIL_PROVIDER")
            .unwrap_or_else(|| "log".into())
            .to_ascii_lowercase();

        match provider.as_str() {
            "log" => Ok(Self::Log { from }),
            "cloudflare" | "cf" => {
                let account_id = required_env("CF_ACCOUNT_ID")
                    .or_else(|_| required_env("CLOUDFLARE_ACCOUNT_ID"))
                    .map_err(|_| {
                        "CF_ACCOUNT_ID (or CLOUDFLARE_ACCOUNT_ID) is required when MAIL_PROVIDER=cloudflare"
                    })?;
                let api_token = required_env("CF_API_TOKEN")
                    .or_else(|_| required_env("CLOUDFLARE_API_TOKEN"))
                    .map_err(|_| {
                        "CF_API_TOKEN (or CLOUDFLARE_API_TOKEN) is required when MAIL_PROVIDER=cloudflare"
                    })?;
                if from.address == "noreply@localhost" || from.address.is_empty() {
                    return Err(
                        "MAIL_FROM must be set to an onboarded Cloudflare Email Sending address"
                            .into(),
                    );
                }
                Ok(Self::Cloudflare(CloudflareMailer {
                    client: reqwest::Client::new(),
                    account_id,
                    api_token,
                    from,
                }))
            }
            other => Err(format!("unknown MAIL_PROVIDER={other:?}; use log or cloudflare").into()),
        }
    }

    pub async fn send(&self, message: Message) -> AppResult<()> {
        match self {
            Self::Log { from } => {
                tracing::info!(
                    to = %message.to,
                    from = %from.address,
                    subject = %message.subject,
                    body = %message.text,
                    "mail (log provider): message not sent"
                );
                Ok(())
            }
            Self::Cloudflare(cf) => cf.send(message).await,
        }
    }
}

impl CloudflareMailer {
    async fn send(&self, message: Message) -> AppResult<()> {
        let url = format!(
            "https://api.cloudflare.com/client/v4/accounts/{}/email/sending/send",
            self.account_id
        );

        let from = CfFrom {
            address: self.from.address.as_str(),
            name: self.from.name.as_deref(),
        };
        let body = CfSendRequest {
            to: &message.to,
            from,
            subject: &message.subject,
            text: &message.text,
            html: &message.html,
        };

        let response = self
            .client
            .post(&url)
            .bearer_auth(&self.api_token)
            .json(&body)
            .send()
            .await
            .map_err(|err| {
                tracing::error!(error = %err, "cloudflare email request failed");
                AppError::Internal("Failed to send email".into())
            })?;

        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        if !status.is_success() {
            tracing::error!(%status, body = %text, "cloudflare email send rejected");
            return Err(AppError::Internal("Failed to send email".into()));
        }

        tracing::debug!(to = %message.to, "cloudflare email accepted");
        Ok(())
    }
}

#[derive(Serialize)]
struct CfSendRequest<'a> {
    to: &'a str,
    from: CfFrom<'a>,
    subject: &'a str,
    text: &'a str,
    html: &'a str,
}

#[derive(Serialize)]
struct CfFrom<'a> {
    address: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<&'a str>,
}

fn optional_env(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

fn required_env(key: &str) -> Result<String, std::env::VarError> {
    optional_env(key).ok_or(std::env::VarError::NotPresent)
}

/// Public site origin used in emailed links (`https://example.com`, no trailing slash).
pub fn app_base_url_from_env() -> String {
    optional_env("APP_BASE_URL")
        .unwrap_or_else(|| "http://127.0.0.1:3000".into())
        .trim_end_matches('/')
        .to_string()
}
