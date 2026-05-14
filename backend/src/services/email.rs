//! Email service via SendGrid Web API.
//!
//! Uses the SendGrid `/v3/mail/send` REST endpoint directly with `reqwest`
//! — no extra SDK crate needed. Supports HTML emails with plain-text fallback.

use anyhow::Result;
use reqwest::Client;
use serde_json::json;

use crate::utils::config::Config;

pub struct EmailService {
    client: Client,
    api_key: String,
    from_email: String,
    from_name: String,
    app_base_url: String,
}

impl EmailService {
    pub fn new(config: &Config) -> Result<Self> {
        Ok(Self {
            client: Client::new(),
            api_key: config.sendgrid_api_key.clone(),
            from_email: config.email_from.clone(),
            from_name: config.email_from_name.clone(),
            app_base_url: config.app_base_url.clone(),
        })
    }

    /// Send an expert invitation email with a direct portal link.
    pub async fn send_expert_invitation_email(
        &self,
        to_name: &str,
        to_email: &str,
        paper_title: &str,
        paper_id: &str,
    ) -> Result<()> {
        let subject = format!("Expert Opinion Requested: {}", paper_title);

        let html = format!(
            r#"<!DOCTYPE html>
<html lang="en">
<head><meta charset="UTF-8"><meta name="viewport" content="width=device-width,initial-scale=1"></head>
<body style="margin:0;padding:0;background:#f5f0eb;font-family:Georgia,'Times New Roman',serif;">
  <table width="100%" cellpadding="0" cellspacing="0" style="background:#f5f0eb;padding:40px 0;">
    <tr><td align="center">
      <table width="600" cellpadding="0" cellspacing="0" style="background:#ffffff;border-radius:16px;border:1px solid #e0d8cf;overflow:hidden;max-width:600px;width:100%;">

        <tr>
          <td style="background:#6b1f2a;padding:24px 32px;">
            <p style="margin:0;color:#ffffff;font-size:18px;font-weight:bold;">📄 LivePaper</p>
          </td>
        </tr>

        <tr>
          <td style="padding:32px;">
            <p style="margin:0 0 16px;font-size:15px;color:#2c2217;line-height:1.6;">
              Dear <strong>{to_name}</strong>,
            </p>
            <p style="margin:0 0 24px;font-size:15px;color:#5a4535;line-height:1.6;">
              You have been added as an expert for a paper on LivePaper. Unanswered questions from researchers on related topic will
              be shared with you. We invite you to share your expertise by answering these questions — your insights will help future researchers get accurate answers instantly.   
            </p>

            <table width="100%" cellpadding="0" cellspacing="0" style="margin-bottom:24px;">
              <tr>
                <td style="background:#faf7f4;border:1px solid #e0d8cf;border-radius:12px;padding:20px;">
                  <p style="margin:0 0 6px;font-size:11px;font-weight:bold;color:#8a7060;text-transform:uppercase;letter-spacing:0.08em;font-family:system-ui,sans-serif;">Paper</p>
                  <p style="margin:0;font-size:15px;font-weight:bold;color:#2c2217;line-height:1.4;">{paper_title}</p>
                </td>
              </tr>
            </table>


            <p style="margin:0;font-size:13px;color:#8a7060;line-height:1.6;font-family:system-ui,sans-serif;">
              Your responses will be attributed to you and added to our knowledge base,
              helping future researchers get accurate answers instantly.
            </p>
          </td>
        </tr>

        <tr>
          <td style="background:#faf7f4;border-top:1px solid #e0d8cf;padding:20px 32px;">
            <p style="margin:0;font-size:12px;color:#b0a090;font-family:system-ui,sans-serif;">
              You received this because you are listed as an expert for the paper above.
              If this was sent in error, you can safely ignore it.
            </p>
          </td>
        </tr>

      </table>
    </td></tr>
  </table>
</body>
</html>"#,
            to_name = escape_html(to_name),
            paper_title = escape_html(paper_title),
        );

        let plain = format!(
            "Dear {to_name},\n\nYou have been added as an expert for a paper on LivePaper: {paper_title}.\n Unanswered questions from researchers on related topic will be shared with you. We invite you to share your expertise by answering these questions — your insights will help future researchers get accurate answers instantly.\n\nBest regards,\nThe LivePaper Team",
            to_name = to_name,
            paper_title = paper_title,
        );

        self.send_html(to_email, to_name, &subject, &html, &plain).await
    }


    /// Send an expert invitation email with a direct portal link.
    pub async fn send_expert_invitation(
        &self,
        to_name: &str,
        to_email: &str,
        paper_title: &str,
        question: &str,
        paper_id: &str,
    ) -> Result<()> {
        let portal_link = format!(
            "{}/expert-response?paper_id={}&expert_email={}&question={}",
            self.app_base_url.trim_end_matches('/'),
            url_encode(paper_id),
            url_encode(to_email),
            url_encode(question),
        );

        let subject = format!("Expert Opinion Requested: {}", paper_title);

        let html = format!(
            r#"<!DOCTYPE html>
<html lang="en">
<head><meta charset="UTF-8"><meta name="viewport" content="width=device-width,initial-scale=1"></head>
<body style="margin:0;padding:0;background:#f5f0eb;font-family:Georgia,'Times New Roman',serif;">
  <table width="100%" cellpadding="0" cellspacing="0" style="background:#f5f0eb;padding:40px 0;">
    <tr><td align="center">
      <table width="600" cellpadding="0" cellspacing="0" style="background:#ffffff;border-radius:16px;border:1px solid #e0d8cf;overflow:hidden;max-width:600px;width:100%;">

        <tr>
          <td style="background:#6b1f2a;padding:24px 32px;">
            <p style="margin:0;color:#ffffff;font-size:18px;font-weight:bold;">📄 LivePaper</p>
          </td>
        </tr>

        <tr>
          <td style="padding:32px;">
            <p style="margin:0 0 16px;font-size:15px;color:#2c2217;line-height:1.6;">
              Dear <strong>{to_name}</strong>,
            </p>
            <p style="margin:0 0 24px;font-size:15px;color:#5a4535;line-height:1.6;">
              A researcher using LivePaper has a question about a paper you are associated with
              and we believe you are best placed to answer it.
            </p>

            <table width="100%" cellpadding="0" cellspacing="0" style="margin-bottom:24px;">
              <tr>
                <td style="background:#faf7f4;border:1px solid #e0d8cf;border-radius:12px;padding:20px;">
                  <p style="margin:0 0 6px;font-size:11px;font-weight:bold;color:#8a7060;text-transform:uppercase;letter-spacing:0.08em;font-family:system-ui,sans-serif;">Paper</p>
                  <p style="margin:0;font-size:15px;font-weight:bold;color:#2c2217;line-height:1.4;">{paper_title}</p>
                </td>
              </tr>
            </table>

            <table width="100%" cellpadding="0" cellspacing="0" style="margin-bottom:32px;">
              <tr>
                <td style="background:#fdf6f0;border:1px solid #f0e4d8;border-left:3px solid #6b1f2a;border-radius:0 12px 12px 0;padding:20px;">
                  <p style="margin:0 0 6px;font-size:11px;font-weight:bold;color:#8a7060;text-transform:uppercase;letter-spacing:0.08em;font-family:system-ui,sans-serif;">Question from researcher</p>
                  <p style="margin:0;font-size:15px;color:#2c2217;line-height:1.6;">{question}</p>
                </td>
              </tr>
            </table>

            <table width="100%" cellpadding="0" cellspacing="0" style="margin-bottom:24px;">
              <tr>
                <td align="center">
                  <a href="{portal_link}" style="display:inline-block;background:#6b1f2a;color:#ffffff;text-decoration:none;font-family:system-ui,sans-serif;font-size:15px;font-weight:600;padding:14px 32px;border-radius:10px;">
                    Submit Your Expert Response
                  </a>
                </td>
              </tr>
            </table>

            <p style="margin:0 0 8px;font-size:13px;color:#8a7060;font-family:system-ui,sans-serif;">Or paste this link into your browser:</p>
            <p style="margin:0 0 24px;font-size:12px;color:#6b1f2a;word-break:break-all;font-family:system-ui,sans-serif;">{portal_link}</p>

            <p style="margin:0;font-size:13px;color:#8a7060;line-height:1.6;font-family:system-ui,sans-serif;">
              Your response will be attributed to you and added to our knowledge base,
              helping future researchers get accurate answers instantly.
            </p>
          </td>
        </tr>

        <tr>
          <td style="background:#faf7f4;border-top:1px solid #e0d8cf;padding:20px 32px;">
            <p style="margin:0;font-size:12px;color:#b0a090;font-family:system-ui,sans-serif;">
              You received this because you are listed as an author or expert for the paper above.
              If this was sent in error, you can safely ignore it.
            </p>
          </td>
        </tr>

      </table>
    </td></tr>
  </table>
</body>
</html>"#,
            to_name = escape_html(to_name),
            paper_title = escape_html(paper_title),
            question = escape_html(question),
            portal_link = portal_link,
        );

        let plain = format!(
            "Dear {to_name},\n\nA researcher has a question about the paper: {paper_title}\n\nQuestion:\n{question}\n\nSubmit your response here:\n{portal_link}\n\nBest regards,\nThe LivePaper Team",
            to_name = to_name,
            paper_title = paper_title,
            question = question,
            portal_link = portal_link,
        );

        self.send_html(to_email, to_name, &subject, &html, &plain).await
    }

    /// Send an HTML email via the SendGrid v3 API.
    ///
    /// `plain_text` is the fallback shown by email clients that don't render HTML.
    pub async fn send_html(
        &self,
        to_email: &str,
        to_name: &str,
        subject: &str,
        html_body: &str,
        plain_text: &str,
    ) -> Result<()> {
        let body = json!({
            "personalizations": [{
                "to": [{ "email": to_email, "name": to_name }]
            }],
            "from": {
                "email": self.from_email,
                "name": self.from_name
            },
            "subject": subject,
            "content": [
                { "type": "text/plain", "value": plain_text },
                { "type": "text/html",  "value": html_body  }
            ]
        });

        let resp = self.client
            .post("https://api.sendgrid.com/v3/mail/send")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("SendGrid request failed: {e}"))?;

        // SendGrid returns 202 Accepted on success — no body
        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("SendGrid error {status}: {text}"));
        }

        tracing::info!("Email sent to {to_email} via SendGrid");
        Ok(())
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Percent-encode a string for safe inclusion in a URL query parameter.
fn url_encode(s: &str) -> String {
    s.bytes()
        .flat_map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9'
            | b'-' | b'_' | b'.' | b'~' => vec![b as char],
            _ => format!("%{:02X}", b).chars().collect(),
        })
        .collect()
}

/// Escape HTML special characters to prevent injection in email bodies.
fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
     .replace('<', "&lt;")
     .replace('>', "&gt;")
     .replace('"', "&quot;")
     .replace('\'', "&#39;")
}