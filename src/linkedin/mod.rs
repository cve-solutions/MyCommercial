use anyhow::{Result, Context};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::models::{Contact, LinkedInAuthMethod};
use crate::settings::SettingsManager;

/// Client LinkedIn supportant plusieurs méthodes d'authentification
pub struct LinkedInClient {
    client: Client,
    auth_method: LinkedInAuthMethod,
    access_token: Option<String>,
    cookie_li_at: Option<String>,
    api_key: Option<String>,
}

#[derive(Debug, Deserialize)]
struct LinkedInSearchResponse {
    elements: Option<Vec<LinkedInProfile>>,
}

#[derive(Debug, Deserialize)]
struct LinkedInProfile {
    #[serde(rename = "publicIdentifier")]
    public_identifier: Option<String>,
    #[serde(rename = "firstName")]
    first_name: Option<String>,
    #[serde(rename = "lastName")]
    last_name: Option<String>,
    headline: Option<String>,
    #[serde(rename = "profileUrl")]
    profile_url: Option<String>,
}

#[derive(Debug, Serialize)]
struct LinkedInMessage {
    recipients: Vec<String>,
    subject: Option<String>,
    body: String,
}

impl LinkedInClient {
    pub fn new(settings: &SettingsManager) -> Result<Self> {
        let auth_method = LinkedInAuthMethod::from_db(&settings.linkedin_auth_method());
        let access_token = settings.get("linkedin", "access_token").ok().filter(|s| !s.is_empty());
        let cookie_li_at = settings.get("linkedin", "cookie_li_at").ok().filter(|s| !s.is_empty());
        let api_key = settings.get("linkedin", "api_key").ok().filter(|s| !s.is_empty());

        Ok(Self {
            client: Client::new(),
            auth_method,
            access_token,
            cookie_li_at,
            api_key,
        })
    }

    pub fn is_authenticated(&self) -> bool {
        match self.auth_method {
            LinkedInAuthMethod::OAuth2 => self.access_token.is_some(),
            LinkedInAuthMethod::Cookie => self.cookie_li_at.is_some(),
            LinkedInAuthMethod::ApiKey => self.api_key.is_some(),
        }
    }

    fn auth_header(&self) -> Result<(String, String)> {
        match self.auth_method {
            LinkedInAuthMethod::OAuth2 => {
                let token = self.access_token.as_ref()
                    .context("Token OAuth2 non configuré")?;
                Ok(("Authorization".into(), format!("Bearer {}", token)))
            }
            LinkedInAuthMethod::Cookie => {
                let cookie = self.cookie_li_at.as_ref()
                    .context("Cookie li_at non configuré")?;
                Ok(("Cookie".into(), format!("li_at={}", cookie)))
            }
            LinkedInAuthMethod::ApiKey => {
                let key = self.api_key.as_ref()
                    .context("API Key non configurée")?;
                Ok(("Authorization".into(), format!("Bearer {}", key)))
            }
        }
    }

    /// Recherche des profils LinkedIn par poste et entreprise
    pub async fn search_people(
        &self,
        keywords: &str,
        title: &str,
        company: Option<&str>,
        start: u32,
        count: u32,
    ) -> Result<Vec<Contact>> {
        let (header_name, header_value) = self.auth_header()?;

        let mut url = format!(
            "https://api.linkedin.com/v2/search/blended?q=people&keywords={}&title={}&start={}&count={}",
            urlencoding::encode(keywords),
            urlencoding::encode(title),
            start,
            count,
        );

        if let Some(comp) = company {
            url.push_str(&format!("&company={}", urlencoding::encode(comp)));
        }

        let resp = self.client
            .get(&url)
            .header(&header_name, &header_value)
            .header("X-Restli-Protocol-Version", "2.0.0")
            .send()
            .await
            .context("Erreur de connexion à l'API LinkedIn")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("LinkedIn API error {}: {}", status, body);
        }

        let data: LinkedInSearchResponse = resp.json().await
            .context("Erreur de parsing de la réponse LinkedIn")?;

        let contacts = data.elements.unwrap_or_default().into_iter().map(|p| {
            Contact {
                id: None,
                linkedin_id: p.public_identifier.clone(),
                prenom: p.first_name.unwrap_or_default(),
                nom: p.last_name.unwrap_or_default(),
                poste: p.headline.unwrap_or_default(),
                entreprise_siren: None,
                entreprise_nom: company.map(|s| s.to_string()),
                linkedin_url: p.profile_url.or_else(|| {
                    p.public_identifier.map(|id| format!("https://www.linkedin.com/in/{}", id))
                }),
                email: None,
            }
        }).collect();

        Ok(contacts)
    }

    /// Envoie un message LinkedIn à un contact
    pub async fn send_message(&self, recipient_id: &str, body: &str) -> Result<()> {
        let (header_name, header_value) = self.auth_header()?;

        let payload = serde_json::json!({
            "recipients": [format!("urn:li:person:{}", recipient_id)],
            "body": body,
        });

        let resp = self.client
            .post("https://api.linkedin.com/v2/messages")
            .header(&header_name, &header_value)
            .header("Content-Type", "application/json")
            .header("X-Restli-Protocol-Version", "2.0.0")
            .json(&payload)
            .send()
            .await
            .context("Erreur d'envoi de message LinkedIn")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("Erreur envoi message LinkedIn {}: {}", status, body);
        }

        Ok(())
    }

    /// Flux OAuth2 complet : ouvre le navigateur, écoute le callback, échange le code
    pub async fn oauth2_full_flow(
        client_id: &str,
        client_secret: &str,
        redirect_uri: &str,
    ) -> Result<String> {
        // Parse port from redirect_uri
        let port = Self::parse_port_from_uri(redirect_uri)?;
        let path = Self::parse_path_from_uri(redirect_uri);

        // Bind the TCP listener BEFORE opening the browser
        let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", port))
            .await
            .context(format!("Impossible d'écouter sur le port {}. Vérifiez qu'il n'est pas utilisé.", port))?;

        // Build authorization URL and open browser
        let auth_url = format!(
            "https://www.linkedin.com/oauth/v2/authorization?response_type=code&client_id={}&redirect_uri={}&scope=openid%20profile%20email%20w_member_social",
            urlencoding::encode(client_id),
            urlencoding::encode(redirect_uri),
        );
        let _ = open::that(&auth_url);

        // Wait for the callback (timeout 120s)
        let code = tokio::time::timeout(
            std::time::Duration::from_secs(120),
            Self::wait_for_callback(&listener, &path),
        )
        .await
        .context("Timeout: pas de réponse LinkedIn en 2 minutes. Réessayez.")?
        .context("Erreur lors de la réception du callback")?;

        // Exchange code for token
        let token = Self::oauth2_exchange_token(client_id, client_secret, &code, redirect_uri).await?;

        Ok(token)
    }

    /// Wait for OAuth2 callback on local TCP listener
    async fn wait_for_callback(
        listener: &tokio::net::TcpListener,
        expected_path: &str,
    ) -> Result<String> {
        loop {
            let (mut stream, _) = listener.accept().await
                .context("Erreur d'acceptation de connexion")?;

            let mut buf = vec![0u8; 4096];
            let n = stream.read(&mut buf).await
                .context("Erreur de lecture HTTP")?;
            let request = String::from_utf8_lossy(&buf[..n]);

            // Parse GET line: "GET /callback?code=XXXX&state=... HTTP/1.1"
            let first_line = request.lines().next().unwrap_or("");
            let parts: Vec<&str> = first_line.split_whitespace().collect();
            if parts.len() < 2 || parts[0] != "GET" {
                Self::send_http_response(&mut stream, 400, "Requête invalide").await;
                continue;
            }

            let request_uri = parts[1];

            // Check path matches
            let (req_path, query) = match request_uri.split_once('?') {
                Some((p, q)) => (p, q),
                None => {
                    Self::send_http_response(&mut stream, 400, "Pas de paramètres").await;
                    continue;
                }
            };

            if req_path != expected_path {
                Self::send_http_response(&mut stream, 404, "Not found").await;
                continue;
            }

            // Check for error
            let params: Vec<(&str, &str)> = query
                .split('&')
                .filter_map(|p| p.split_once('='))
                .collect();

            if let Some((_, err)) = params.iter().find(|(k, _)| *k == "error") {
                let desc = params.iter()
                    .find(|(k, _)| *k == "error_description")
                    .map(|(_, v)| urlencoding::decode(v).unwrap_or_default().to_string())
                    .unwrap_or_default();
                Self::send_http_response(&mut stream, 200,
                    &format!("<h2 style='color:red'>Erreur LinkedIn</h2><p>{}: {}</p>", err, desc)
                ).await;
                anyhow::bail!("LinkedIn OAuth2 erreur: {} - {}", err, desc);
            }

            // Extract code
            if let Some((_, code)) = params.iter().find(|(k, _)| *k == "code") {
                let code = urlencoding::decode(code)
                    .unwrap_or_default()
                    .to_string();
                Self::send_http_response(&mut stream, 200, OAUTH_SUCCESS_HTML).await;
                return Ok(code);
            }

            Self::send_http_response(&mut stream, 400, "Paramètre 'code' manquant").await;
        }
    }

    async fn send_http_response(stream: &mut tokio::net::TcpStream, status: u16, body: &str) {
        let status_text = match status {
            200 => "OK",
            400 => "Bad Request",
            404 => "Not Found",
            _ => "Error",
        };
        let html = if body.starts_with('<') {
            body.to_string()
        } else {
            format!("<html><body><h2>{}</h2></body></html>", body)
        };
        let response = format!(
            "HTTP/1.1 {} {}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            status, status_text, html.len(), html
        );
        let _ = stream.write_all(response.as_bytes()).await;
        let _ = stream.flush().await;
    }

    fn parse_port_from_uri(uri: &str) -> Result<u16> {
        // http://localhost:8080/callback -> 8080
        let after_scheme = uri.split("://").nth(1).unwrap_or(uri);
        let host_port = after_scheme.split('/').next().unwrap_or("");
        if let Some(port_str) = host_port.split(':').nth(1) {
            port_str.parse::<u16>().context("Port invalide dans redirect_uri")
        } else {
            Ok(8080)
        }
    }

    fn parse_path_from_uri(uri: &str) -> String {
        // http://localhost:8080/callback -> /callback
        let after_scheme = uri.split("://").nth(1).unwrap_or(uri);
        match after_scheme.find('/') {
            Some(idx) => after_scheme[idx..].to_string(),
            None => "/callback".to_string(),
        }
    }

    /// Échange un code OAuth2 contre un token d'accès
    pub async fn oauth2_exchange_token(
        client_id: &str,
        client_secret: &str,
        code: &str,
        redirect_uri: &str,
    ) -> Result<String> {
        let client = Client::new();
        let resp = client
            .post("https://www.linkedin.com/oauth/v2/accessToken")
            .form(&[
                ("grant_type", "authorization_code"),
                ("code", code),
                ("client_id", client_id),
                ("client_secret", client_secret),
                ("redirect_uri", redirect_uri),
            ])
            .send()
            .await
            .context("Erreur échange token OAuth2")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("LinkedIn token exchange error {}: {}", status, body);
        }

        let data: serde_json::Value = resp.json().await?;

        if let Some(err) = data.get("error") {
            let desc = data.get("error_description")
                .and_then(|v| v.as_str())
                .unwrap_or("Inconnue");
            anyhow::bail!("LinkedIn token error: {} - {}", err, desc);
        }

        data["access_token"]
            .as_str()
            .map(|s| s.to_string())
            .context("Token non trouvé dans la réponse OAuth2")
    }
}

const OAUTH_SUCCESS_HTML: &str = r#"<!DOCTYPE html>
<html lang="fr">
<head><meta charset="utf-8"><title>MyCommercial - LinkedIn</title>
<style>
body { font-family: -apple-system, sans-serif; background: #0f172a; color: #e2e8f0;
       display: flex; justify-content: center; align-items: center; height: 100vh; margin: 0; }
.box { text-align: center; background: #1e293b; padding: 40px 60px; border-radius: 12px;
       box-shadow: 0 4px 24px rgba(0,0,0,0.5); }
h1 { color: #22c55e; margin-bottom: 10px; }
p { color: #94a3b8; font-size: 18px; }
.icon { font-size: 64px; margin-bottom: 16px; }
</style></head>
<body><div class="box">
<div class="icon">&#x2714;</div>
<h1>Connexion LinkedIn réussie !</h1>
<p>Vous pouvez fermer cet onglet et retourner à MyCommercial.</p>
</div></body></html>"#;
