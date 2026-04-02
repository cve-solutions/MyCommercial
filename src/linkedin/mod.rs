use anyhow::{Result, Context};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::models::{Contact, LinkedInAuthMethod};
use crate::settings::SettingsManager;

/// Client LinkedIn — utilise linkedin-api (Python) pour search/send
pub struct LinkedInClient {
    client: Client,
    auth_method: LinkedInAuthMethod,
    access_token: Option<String>,
    login_email: String,
    login_password: String,
}

impl LinkedInClient {
    pub fn new(settings: &SettingsManager) -> Result<Self> {
        let auth_method = LinkedInAuthMethod::from_db(&settings.linkedin_auth_method());
        let access_token = settings.get("linkedin", "access_token").ok().filter(|s| !s.is_empty());

        Ok(Self {
            client: Client::new(),
            auth_method,
            access_token,
            login_email: settings.get_or_default("linkedin", "login_email", ""),
            login_password: settings.get_or_default("linkedin", "login_password", ""),
        })
    }

    pub fn is_authenticated(&self) -> bool {
        (!self.login_email.is_empty() && !self.login_password.is_empty())
            || self.access_token.is_some()
    }

    /// Find the Python bridge script path
    fn bridge_path() -> Result<String> {
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.to_path_buf()));

        let candidates = vec![
            exe_dir.as_ref().map(|d| d.join("../scripts/linkedin_bridge.py")),
            exe_dir.as_ref().map(|d| d.join("scripts/linkedin_bridge.py")),
            Some(std::path::PathBuf::from("scripts/linkedin_bridge.py")),
            Some(std::path::PathBuf::from("linkedin_bridge.py")),
        ];

        for candidate in candidates.into_iter().flatten() {
            if candidate.exists() {
                return Ok(candidate.to_string_lossy().to_string());
            }
        }

        anyhow::bail!("Script linkedin_bridge.py introuvable. Vérifiez le dossier scripts/.")
    }

    /// Call the Python bridge with a JSON command
    async fn bridge_call(&self, action: &str, params: serde_json::Value) -> Result<serde_json::Value> {
        if self.login_email.is_empty() || self.login_password.is_empty() {
            anyhow::bail!("Configurez login_email et login_password dans Settings > LinkedIn.");
        }

        let bridge = Self::bridge_path()?;

        let input = serde_json::json!({
            "action": action,
            "email": &self.login_email,
            "password": &self.login_password,
            "params": params,
        });

        let input_str = serde_json::to_string(&input)?;

        let output = tokio::process::Command::new("python3")
            .arg(&bridge)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .context("Impossible de lancer python3. Vérifiez que Python 3 est installé.")?
            .wait_with_output()
            .await
            .map_err(|e| {
                // Write stdin before waiting
                anyhow::anyhow!("Erreur exécution bridge: {}", e)
            })?;

        // We need to pipe stdin properly
        drop(output); // discard the above

        let mut child = tokio::process::Command::new("python3")
            .arg(&bridge)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .context("Impossible de lancer python3")?;

        if let Some(ref mut stdin) = child.stdin {
            use tokio::io::AsyncWriteExt;
            stdin.write_all(input_str.as_bytes()).await?;
            stdin.shutdown().await?;
        }

        let output = tokio::time::timeout(
            std::time::Duration::from_secs(60),
            child.wait_with_output(),
        )
        .await
        .context("Timeout (60s) sur le bridge LinkedIn")?
        .context("Erreur exécution bridge LinkedIn")?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        if stdout.trim().is_empty() {
            let err_msg = if stderr.is_empty() {
                "Pas de réponse du bridge LinkedIn".to_string()
            } else {
                format!("Bridge LinkedIn: {}", stderr.lines().last().unwrap_or(&stderr))
            };
            anyhow::bail!("{}", err_msg);
        }

        let result: serde_json::Value = serde_json::from_str(stdout.trim())
            .context(format!("Réponse bridge invalide: {}", &stdout[..stdout.len().min(200)]))?;

        if let Some(err) = result.get("error").and_then(|e| e.as_str()) {
            anyhow::bail!("{}", err);
        }

        Ok(result)
    }

    /// Recherche des profils LinkedIn via linkedin-api (Python)
    pub async fn search_people(
        &self,
        keywords: &str,
        _title: &str,
        _company: Option<&str>,
        start: u32,
        count: u32,
    ) -> Result<Vec<Contact>> {
        let result = self.bridge_call("search", serde_json::json!({
            "keywords": keywords,
            "limit": count,
            "offset": start,
        })).await?;

        let contacts = result.get("results")
            .and_then(|r| r.as_array())
            .map(|arr| {
                arr.iter().filter_map(|item| {
                    let name = item.get("name").and_then(|n| n.as_str()).unwrap_or("");
                    let parts: Vec<&str> = name.splitn(2, ' ').collect();
                    let prenom = parts.first().unwrap_or(&"").to_string();
                    let nom = parts.get(1).unwrap_or(&"").to_string();
                    if prenom.is_empty() && nom.is_empty() { return None; }

                    let urn_id = item.get("urn_id").and_then(|u| u.as_str()).unwrap_or("").to_string();
                    let public_id = item.get("public_id").and_then(|p| p.as_str()).unwrap_or("").to_string();
                    let jobtitle = item.get("jobtitle").and_then(|j| j.as_str()).unwrap_or("").to_string();

                    Some(Contact {
                        id: None,
                        linkedin_id: if !urn_id.is_empty() { Some(urn_id) } else if !public_id.is_empty() { Some(public_id.clone()) } else { None },
                        prenom,
                        nom,
                        poste: jobtitle,
                        entreprise_siren: None,
                        entreprise_nom: None,
                        linkedin_url: if !public_id.is_empty() { Some(format!("https://www.linkedin.com/in/{}", public_id)) } else { None },
                        email: None,
                    })
                }).collect()
            })
            .unwrap_or_default();

        Ok(contacts)
    }

    /// Alias pour compatibilité
    pub async fn search_people_debug(
        &self,
        keywords: &str,
        title: &str,
        company: Option<&str>,
        start: u32,
        count: u32,
    ) -> Result<(Vec<Contact>, String)> {
        let contacts = self.search_people(keywords, title, company, start, count).await?;
        let debug = format!("{} résultats via linkedin-api (Python)", contacts.len());
        Ok((contacts, debug))
    }

    /// Envoie un message LinkedIn via linkedin-api (Python)
    pub async fn send_message(&self, recipient_id: &str, body: &str) -> Result<()> {
        // Always pass both urn and public_id when possible
        let params = if recipient_id.starts_with("ACoA") {
            serde_json::json!({
                "recipients": [recipient_id],
                "message": body,
            })
        } else {
            // publicIdentifier — Python will resolve via get_profile
            serde_json::json!({
                "public_id": recipient_id,
                "message": body,
            })
        };

        self.bridge_call("send", params).await?;
        Ok(())
    }

    /// Login via linkedin-api et retourne le cookie li_at
    pub async fn login_get_cookie(email: &str, password: &str) -> Result<String> {
        let bridge = Self::bridge_path()?;

        let input = serde_json::json!({
            "action": "get_cookie",
            "email": email,
            "password": password,
            "params": {},
        });

        let input_str = serde_json::to_string(&input)?;

        let mut child = tokio::process::Command::new("python3")
            .arg(&bridge)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .context("Impossible de lancer python3")?;

        if let Some(ref mut stdin) = child.stdin {
            use tokio::io::AsyncWriteExt;
            stdin.write_all(input_str.as_bytes()).await?;
            stdin.shutdown().await?;
        }

        let output = tokio::time::timeout(
            std::time::Duration::from_secs(60),
            child.wait_with_output(),
        )
        .await
        .context("Timeout login LinkedIn")?
        .context("Erreur bridge LinkedIn")?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let result: serde_json::Value = serde_json::from_str(stdout.trim())
            .context(format!("Réponse invalide: {}", &stdout[..stdout.len().min(200)]))?;

        if let Some(err) = result.get("error").and_then(|e| e.as_str()) {
            anyhow::bail!("{}", err);
        }

        result.get("li_at")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .context("Cookie li_at non retourné par le bridge")
    }

    /// Flux OAuth2 complet : ouvre le navigateur, écoute le callback, échange le code
    pub async fn oauth2_full_flow(
        client_id: &str,
        client_secret: &str,
        redirect_uri: &str,
    ) -> Result<String> {
        let port = Self::parse_port_from_uri(redirect_uri)?;
        let path = Self::parse_path_from_uri(redirect_uri);

        let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", port))
            .await
            .context(format!("Impossible d'écouter sur le port {}", port))?;

        let auth_url = format!(
            "https://www.linkedin.com/oauth/v2/authorization?response_type=code&client_id={}&redirect_uri={}&scope=openid%20profile%20email%20w_member_social",
            urlencoding::encode(client_id),
            urlencoding::encode(redirect_uri),
        );
        let _ = open::that(&auth_url);

        let code = tokio::time::timeout(
            std::time::Duration::from_secs(120),
            Self::wait_for_callback(&listener, &path),
        )
        .await
        .context("Timeout: pas de réponse LinkedIn en 2 minutes.")?
        .context("Erreur lors de la réception du callback")?;

        let token = Self::oauth2_exchange_token(client_id, client_secret, &code, redirect_uri).await?;
        Ok(token)
    }

    async fn wait_for_callback(
        listener: &tokio::net::TcpListener,
        expected_path: &str,
    ) -> Result<String> {
        loop {
            let (mut stream, _) = listener.accept().await
                .context("Erreur d'acceptation de connexion")?;

            let mut buf = vec![0u8; 4096];
            let n = stream.read(&mut buf).await?;
            let request = String::from_utf8_lossy(&buf[..n]);

            let first_line = request.lines().next().unwrap_or("");
            let parts: Vec<&str> = first_line.split_whitespace().collect();
            if parts.len() < 2 || parts[0] != "GET" {
                Self::send_http_response(&mut stream, 400, "Requête invalide").await;
                continue;
            }

            let request_uri = parts[1];
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

            if let Some((_, code)) = params.iter().find(|(k, _)| *k == "code") {
                let code = urlencoding::decode(code).unwrap_or_default().to_string();
                Self::send_http_response(&mut stream, 200, OAUTH_SUCCESS_HTML).await;
                return Ok(code);
            }

            Self::send_http_response(&mut stream, 400, "Paramètre 'code' manquant").await;
        }
    }

    async fn send_http_response(stream: &mut tokio::net::TcpStream, status: u16, body: &str) {
        let status_text = match status {
            200 => "OK", 400 => "Bad Request", 404 => "Not Found", _ => "Error",
        };
        let html = if body.starts_with('<') { body.to_string() }
        else { format!("<html><body><h2>{}</h2></body></html>", body) };
        let response = format!(
            "HTTP/1.1 {} {}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            status, status_text, html.len(), html
        );
        let _ = stream.write_all(response.as_bytes()).await;
        let _ = stream.flush().await;
    }

    fn parse_port_from_uri(uri: &str) -> Result<u16> {
        let after_scheme = uri.split("://").nth(1).unwrap_or(uri);
        let host_port = after_scheme.split('/').next().unwrap_or("");
        if let Some(port_str) = host_port.split(':').nth(1) {
            port_str.parse::<u16>().context("Port invalide dans redirect_uri")
        } else {
            Ok(8080)
        }
    }

    fn parse_path_from_uri(uri: &str) -> String {
        let after_scheme = uri.split("://").nth(1).unwrap_or(uri);
        match after_scheme.find('/') {
            Some(idx) => after_scheme[idx..].to_string(),
            None => "/callback".to_string(),
        }
    }

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
