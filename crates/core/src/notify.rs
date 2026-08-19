//! Completion notifications: webhook, e-mail (SMTP) and Telegram.
//!
//! Channels are configured through `[notifications]` in the config file.
//! `Notifier::notify_completion` fans out to every configured channel; a
//! failing channel does not prevent the others from being delivered, and all
//! failures are aggregated into the returned error.

use serde::{Deserialize, Serialize};

use crate::config::NotificationsConfig;
use crate::session::SessionOutput;

/// A single notification destination.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NotificationChannel {
    /// Generic JSON webhook (POST).
    Webhook { url: String },
    /// Plain or STARTTLS SMTP delivery.
    Email {
        smtp_host: String,
        smtp_port: u16,
        from: String,
        to: String,
        /// Optional SMTP AUTH credentials (empty username = no auth).
        username: String,
        password: String,
    },
    /// Telegram bot message.
    Telegram { bot_token: String, chat_id: String },
}

impl std::fmt::Display for NotificationChannel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Webhook { url } => write!(f, "webhook({url})"),
            Self::Email { to, smtp_host, .. } => write!(f, "email({to}@{smtp_host})"),
            Self::Telegram { chat_id, .. } => write!(f, "telegram(chat {chat_id})"),
        }
    }
}

/// Fan-out notifier for finished research sessions.
#[derive(Debug, Clone)]
pub struct Notifier {
    channels: Vec<NotificationChannel>,
    http: reqwest::Client,
}

impl Default for Notifier {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

impl Notifier {
    pub fn new(channels: Vec<NotificationChannel>) -> Self {
        Self {
            channels,
            http: crate::http_client(),
        }
    }

    /// Build a notifier from the `[notifications]` config section. Empty
    /// values are skipped, so an unconfigured section yields an empty notifier.
    pub fn from_config(config: &NotificationsConfig) -> Self {
        let mut channels = Vec::new();

        if !config.webhook_url.trim().is_empty() {
            channels.push(NotificationChannel::Webhook {
                url: config.webhook_url.trim().to_string(),
            });
        }

        if !config.email_to.trim().is_empty() {
            let smtp_host = if config.smtp_host.trim().is_empty() {
                "localhost".to_string()
            } else {
                config.smtp_host.trim().to_string()
            };
            let from = if config.email_from.trim().is_empty() {
                "parallel-research@localhost".to_string()
            } else {
                config.email_from.trim().to_string()
            };
            channels.push(NotificationChannel::Email {
                smtp_host,
                smtp_port: config.smtp_port,
                from,
                to: config.email_to.trim().to_string(),
                username: config.smtp_username.clone(),
                password: config.smtp_password.clone(),
            });
        }

        if !config.telegram_bot_token.trim().is_empty() && !config.telegram_chat_id.trim().is_empty()
        {
            channels.push(NotificationChannel::Telegram {
                bot_token: config.telegram_bot_token.trim().to_string(),
                chat_id: config.telegram_chat_id.trim().to_string(),
            });
        }

        Self::new(channels)
    }

    pub fn channels(&self) -> &[NotificationChannel] {
        &self.channels
    }

    pub fn is_empty(&self) -> bool {
        self.channels.is_empty()
    }

    /// Notify all configured channels that a session has completed.
    ///
    /// Every channel is attempted; failures are collected and returned as a
    /// single error after all channels have been tried.
    pub async fn notify_completion(&self, session: &SessionOutput) -> anyhow::Result<()> {
        if self.channels.is_empty() {
            return Ok(());
        }

        let mut failures: Vec<String> = Vec::new();
        for channel in &self.channels {
            let result = match channel {
                NotificationChannel::Webhook { url } => self.send_webhook(url, session).await,
                NotificationChannel::Email {
                    smtp_host,
                    smtp_port,
                    from,
                    to,
                    username,
                    password,
                } => {
                    self.send_email(smtp_host, *smtp_port, from, to, username, password, session)
                        .await
                }
                NotificationChannel::Telegram { bot_token, chat_id } => {
                    self.send_telegram(bot_token, chat_id, session).await
                }
            };

            if let Err(e) = result {
                tracing::error!(channel = %channel, error = %e, "notification failed");
                failures.push(format!("{channel}: {e}"));
            } else {
                tracing::info!(channel = %channel, "notification delivered");
            }
        }

        if failures.is_empty() {
            Ok(())
        } else {
            anyhow::bail!("notification failures: {}", failures.join("; "))
        }
    }

    /// Generic alert fan-out (watch diffs, failures, ...). Every channel is
    /// attempted; delivery errors are logged, never propagated — alerts are
    /// best-effort side effects.
    pub async fn notify_alert(&self, event: &str, subject: &str, text: &str) {
        for channel in &self.channels {
            let result = match channel {
                NotificationChannel::Webhook { url } => {
                    self.alert_webhook(url, event, subject, text).await
                }
                NotificationChannel::Telegram { bot_token, chat_id } => {
                    self.alert_telegram(bot_token, chat_id, subject, text).await
                }
                NotificationChannel::Email {
                    smtp_host,
                    smtp_port,
                    from,
                    to,
                    username,
                    password,
                } => {
                    self.alert_email(
                        smtp_host, *smtp_port, from, to, username, password, subject, text,
                    )
                    .await
                }
            };
            if let Err(e) = result {
                tracing::warn!(channel = %channel, error = %e, "alert notification failed");
            }
        }
    }

    async fn alert_webhook(
        &self,
        url: &str,
        event: &str,
        subject: &str,
        text: &str,
    ) -> anyhow::Result<()> {
        let payload = serde_json::json!({
            "event": event,
            "subject": subject,
            "text": text,
        });
        let response = self.http.post(url).json(&payload).send().await?;
        let status = response.status();
        if !status.is_success() {
            anyhow::bail!("webhook returned HTTP {status}");
        }
        Ok(())
    }

    async fn alert_telegram(
        &self,
        bot_token: &str,
        chat_id: &str,
        subject: &str,
        text: &str,
    ) -> anyhow::Result<()> {
        let url = format!("https://api.telegram.org/bot{bot_token}/sendMessage");
        let payload = serde_json::json!({
            "chat_id": chat_id,
            "text": format!("{subject}\n\n{text}"),
        });
        let response = self.http.post(&url).json(&payload).send().await?;
        let status = response.status();
        if !status.is_success() {
            anyhow::bail!("telegram returned HTTP {status}");
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    async fn alert_email(
        &self,
        smtp_host: &str,
        smtp_port: u16,
        from: &str,
        to: &str,
        username: &str,
        password: &str,
        subject: &str,
        text: &str,
    ) -> anyhow::Result<()> {
        use lettre::message::{header::ContentType, Mailbox};
        use lettre::transport::smtp::authentication::Credentials;
        use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};

        let email = Message::builder()
            .from(from.parse::<Mailbox>()?)
            .to(to.parse::<Mailbox>()?)
            .subject(subject.to_string())
            .header(ContentType::TEXT_PLAIN)
            .body(text.to_string())?;
        let transport = if !username.is_empty() {
            AsyncSmtpTransport::<Tokio1Executor>::relay(smtp_host)?
                .port(smtp_port)
                .credentials(Credentials::new(username.to_string(), password.to_string()))
                .build()
        } else if smtp_port == 25 || smtp_port == 1025 {
            AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(smtp_host)
                .port(smtp_port)
                .build()
        } else {
            AsyncSmtpTransport::<Tokio1Executor>::relay(smtp_host)?
                .port(smtp_port)
                .build()
        };
        transport.send(email).await?;
        Ok(())
    }

    // ── Webhook ─────────────────────────────────────────────────────────

    async fn send_webhook(&self, url: &str, session: &SessionOutput) -> anyhow::Result<()> {
        let payload = serde_json::json!({
            "event": "session.completed",
            "session_id": session.session_id.0,
            "output_dir": session.output_dir.display().to_string(),
            "total_tokens": session.total_tokens,
            "total_agents": session.total_agents,
            "summary": session.summary_line(),
            "synthesis_preview": session.synthesis_preview(500),
        });
        let response = self.http.post(url).json(&payload).send().await?;
        let status = response.status();
        if !status.is_success() {
            anyhow::bail!("webhook returned HTTP {status}");
        }
        Ok(())
    }

    // ── Email ───────────────────────────────────────────────────────────

    #[allow(clippy::too_many_arguments)]
    async fn send_email(
        &self,
        smtp_host: &str,
        smtp_port: u16,
        from: &str,
        to: &str,
        username: &str,
        password: &str,
        session: &SessionOutput,
    ) -> anyhow::Result<()> {
        use lettre::message::{header::ContentType, Mailbox};
        use lettre::transport::smtp::authentication::Credentials;
        use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};

        let email = Message::builder()
            .from(from.parse::<Mailbox>()?)
            .to(to.parse::<Mailbox>()?)
            .subject(format!("Research session completed: {}", session.session_id))
            .header(ContentType::TEXT_PLAIN)
            .body(email_body(session))?;

        let transport = if !username.is_empty() {
            AsyncSmtpTransport::<Tokio1Executor>::relay(smtp_host)?
                .port(smtp_port)
                .credentials(Credentials::new(username.to_string(), password.to_string()))
                .build()
        } else if smtp_port == 25 || smtp_port == 1025 {
            // Common plaintext ports (local relays, MailHog, ...).
            AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(smtp_host)
                .port(smtp_port)
                .build()
        } else {
            AsyncSmtpTransport::<Tokio1Executor>::relay(smtp_host)?
                .port(smtp_port)
                .build()
        };

        transport.send(email).await?;
        Ok(())
    }

    // ── Telegram ────────────────────────────────────────────────────────

    async fn send_telegram(
        &self,
        bot_token: &str,
        chat_id: &str,
        session: &SessionOutput,
    ) -> anyhow::Result<()> {
        let url = format!("https://api.telegram.org/bot{bot_token}/sendMessage");
        let payload = serde_json::json!({
            "chat_id": chat_id,
            "text": telegram_text(session),
        });
        let response = self.http.post(&url).json(&payload).send().await?;
        let status = response.status();
        let body: serde_json::Value = response.json().await.unwrap_or_default();
        if !status.is_success() || body.get("ok").and_then(|v| v.as_bool()) != Some(true) {
            let description = body
                .get("description")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown error");
            anyhow::bail!("telegram API error (HTTP {status}): {description}");
        }
        Ok(())
    }
}

/// Plain-text email body for a completed session.
pub fn email_body(session: &SessionOutput) -> String {
    format!(
        "{}\n\nSynthesis preview:\n{}\n",
        session.summary_line(),
        session.synthesis_preview(2000)
    )
}

/// Telegram message text for a completed session.
pub fn telegram_text(session: &SessionOutput) -> String {
    format!(
        "✅ {}\n\n{}\n\nFull report: {}",
        session.summary_line(),
        session.synthesis_preview(800),
        session.output_dir.display()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::SessionId;
    use std::path::PathBuf;

    fn sample_session() -> SessionOutput {
        SessionOutput {
            session_id: SessionId("sess-notify-test".to_string()),
            output_dir: PathBuf::from("/tmp/out"),
            synthesis: "# Report\n\nFindings body text.".to_string(),
            total_tokens: 42,
            total_agents: 1,
        }
    }

    fn config_from_toml(toml_str: &str) -> NotificationsConfig {
        #[derive(serde::Deserialize)]
        struct Wrapper {
            #[serde(default)]
            notifications: NotificationsConfig,
        }
        toml::from_str::<Wrapper>(toml_str).unwrap().notifications
    }

    #[test]
    fn test_from_config_empty() {
        let notifier = Notifier::from_config(&NotificationsConfig::default());
        assert!(notifier.is_empty());
        assert!(notifier.channels().is_empty());
    }

    #[test]
    fn test_from_config_webhook_only() {
        let cfg = config_from_toml(
            r#"
[notifications]
webhook_url = "https://hooks.example.com/abc"
"#,
        );
        let notifier = Notifier::from_config(&cfg);
        assert_eq!(notifier.channels().len(), 1);
        assert!(matches!(
            &notifier.channels()[0],
            NotificationChannel::Webhook { url } if url == "https://hooks.example.com/abc"
        ));
    }

    #[test]
    fn test_from_config_email_defaults() {
        let cfg = config_from_toml(
            r#"
[notifications]
email_to = "me@example.com"
"#,
        );
        let notifier = Notifier::from_config(&cfg);
        assert_eq!(notifier.channels().len(), 1);
        match &notifier.channels()[0] {
            NotificationChannel::Email { smtp_host, to, from, .. } => {
                assert_eq!(smtp_host, "localhost");
                assert_eq!(to, "me@example.com");
                assert_eq!(from, "parallel-research@localhost");
            }
            other => panic!("expected Email channel, got {other:?}"),
        }
    }

    #[test]
    fn test_from_config_telegram_requires_token_and_chat() {
        let cfg = config_from_toml(
            r#"
[notifications]
telegram_bot_token = "123:abc"
"#,
        );
        assert!(Notifier::from_config(&cfg).is_empty());

        let cfg = config_from_toml(
            r#"
[notifications]
telegram_bot_token = "123:abc"
telegram_chat_id = "42"
"#,
        );
        let notifier = Notifier::from_config(&cfg);
        assert_eq!(notifier.channels().len(), 1);
        assert!(matches!(
            &notifier.channels()[0],
            NotificationChannel::Telegram { chat_id, .. } if chat_id == "42"
        ));
    }

    #[test]
    fn test_message_bodies_contain_session_info() {
        let session = sample_session();
        assert!(email_body(&session).contains("sess-notify-test"));
        assert!(telegram_text(&session).contains("sess-notify-test"));
        assert!(telegram_text(&session).contains("/tmp/out"));
    }

    #[tokio::test]
    async fn test_notify_completion_no_channels_is_ok() {
        let notifier = Notifier::new(vec![]);
        notifier.notify_completion(&sample_session()).await.unwrap();
    }

    /// End-to-end webhook delivery against a local one-shot HTTP server.
    #[tokio::test]
    async fn test_send_webhook_to_local_server() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buf = Vec::new();
            let mut chunk = [0u8; 4096];
            // Read until we have the full headers, then the body per Content-Length.
            let mut header_end = None;
            let mut expected = None;
            loop {
                let n = socket.read(&mut chunk).await.unwrap();
                if n == 0 {
                    break;
                }
                buf.extend_from_slice(&chunk[..n]);
                if header_end.is_none() {
                    if let Some(pos) = find_subslice(&buf, b"\r\n\r\n") {
                        header_end = Some(pos);
                        let headers = String::from_utf8_lossy(&buf[..pos]).to_lowercase();
                        expected = headers
                            .lines()
                            .find_map(|line| line.strip_prefix("content-length:"))
                            .and_then(|v| v.trim().parse::<usize>().ok());
                    }
                }
                if let (Some(pos), Some(len)) = (header_end, expected) {
                    if buf.len() >= pos + 4 + len {
                        break;
                    }
                }
            }
            socket
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n")
                .await
                .unwrap();
            String::from_utf8_lossy(&buf).to_string()
        });

        let notifier = Notifier::new(vec![NotificationChannel::Webhook {
            url: format!("http://{addr}/hook"),
        }]);
        notifier
            .notify_completion(&sample_session())
            .await
            .expect("webhook delivery should succeed");

        let request = server.await.unwrap();
        assert!(request.starts_with("POST /hook"));
        assert!(request.contains("session.completed"));
        assert!(request.contains("sess-notify-test"));
    }

    /// A failing webhook must surface an error mentioning the channel.
    #[tokio::test]
    async fn test_webhook_failure_is_reported() {
        use tokio::net::TcpListener;
        // Bind and immediately drop → connection refused.
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);

        let notifier = Notifier::new(vec![NotificationChannel::Webhook {
            url: format!("http://{addr}/hook"),
        }]);
        let err = notifier
            .notify_completion(&sample_session())
            .await
            .expect_err("should fail");
        assert!(err.to_string().contains("webhook"));
    }

    fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
        haystack
            .windows(needle.len())
            .position(|window| window == needle)
    }
}
