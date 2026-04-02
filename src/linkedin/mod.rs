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

    /// Recherche des profils LinkedIn via Voyager API (nécessite cookie li_at)
    pub async fn search_people(
        &self,
        keywords: &str,
        title: &str,
        company: Option<&str>,
        start: u32,
        count: u32,
    ) -> Result<Vec<Contact>> {
        // Voyager API requires li_at cookie
        let cookie = self.cookie_li_at.as_ref()
            .context("Recherche LinkedIn nécessite le cookie li_at. Configurez-le dans Settings > LinkedIn.")?;

        let url = format!(
            "https://www.linkedin.com/voyager/api/search/dash/clusters\
            ?decorationId=com.linkedin.voyager.dash.deco.search.SearchClusterCollection-165\
            &origin=GLOBAL_SEARCH_HEADER\
            &q=all\
            &query=(keywords:{keywords},flagshipSearchIntent:SEARCH_SRP,\
queryParameters:(resultType:List(PEOPLE)))\
            &start={start}&count={count}",
            keywords = urlencoding::encode(keywords),
            start = start,
            count = count,
        );

        let http_client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        let resp = http_client
            .get(&url)
            .header("Cookie", format!("li_at={}; JSESSIONID=\"ajax:0\"", cookie))
            .header("Csrf-Token", "ajax:0")
            .header("X-Li-Lang", "fr_FR")
            .header("X-Li-Track", "{\"clientVersion\":\"1.13.8622\"}")
            .header("X-Restli-Protocol-Version", "2.0.0")
            .header("Accept", "application/vnd.linkedin.normalized+json+2.1")
            .header("User-Agent", "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/125.0.0.0 Safari/537.36")
            .timeout(std::time::Duration::from_secs(15))
            .send()
            .await
            .context("Erreur de connexion à LinkedIn — vérifiez votre accès réseau et le cookie li_at")?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            if status.as_u16() == 401 || status.as_u16() == 403 {
                anyhow::bail!("Cookie li_at expiré ou invalide (HTTP {}). Reconnectez-vous à LinkedIn et mettez à jour le cookie.", status);
            }
            anyhow::bail!("LinkedIn error {} : {}", status,
                if body.len() > 300 { &body[..300] } else { &body });
        }

        let body_text = resp.text().await
            .context("Erreur lecture réponse LinkedIn")?;

        let data: serde_json::Value = serde_json::from_str(&body_text)
            .context("Erreur parsing JSON LinkedIn")?;

        // Parse Voyager response
        let mut contacts = Vec::new();
        Self::parse_voyager_results(&data, &mut contacts, company);

        Ok(contacts)
    }

    /// Même que search_people mais retourne aussi des infos de debug
    pub async fn search_people_debug(
        &self,
        keywords: &str,
        title: &str,
        company: Option<&str>,
        start: u32,
        count: u32,
    ) -> Result<(Vec<Contact>, String)> {
        let cookie = self.cookie_li_at.as_ref()
            .context("Recherche LinkedIn nécessite le cookie li_at.")?;

        let url = format!(
            "https://www.linkedin.com/voyager/api/search/dash/clusters\
            ?decorationId=com.linkedin.voyager.dash.deco.search.SearchClusterCollection-165\
            &origin=GLOBAL_SEARCH_HEADER\
            &q=all\
            &query=(keywords:{keywords},flagshipSearchIntent:SEARCH_SRP,\
queryParameters:(resultType:List(PEOPLE)))\
            &start={start}&count={count}",
            keywords = urlencoding::encode(keywords),
            start = start,
            count = count,
        );

        let http_client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        let resp = http_client
            .get(&url)
            .header("Cookie", format!("li_at={}; JSESSIONID=\"ajax:0\"", cookie))
            .header("Csrf-Token", "ajax:0")
            .header("X-Li-Lang", "fr_FR")
            .header("X-Li-Track", "{\"clientVersion\":\"1.13.8622\"}")
            .header("X-Restli-Protocol-Version", "2.0.0")
            .header("Accept", "application/vnd.linkedin.normalized+json+2.1")
            .header("User-Agent", "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/125.0.0.0 Safari/537.36")
            .timeout(std::time::Duration::from_secs(15))
            .send()
            .await
            .context("Erreur de connexion à LinkedIn")?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("LinkedIn error {}: {}", status, if body.len() > 300 { &body[..300] } else { &body });
        }

        let body_text = resp.text().await.unwrap_or_default();
        let data: serde_json::Value = serde_json::from_str(&body_text)
            .context("Erreur parsing JSON")?;

        let mut contacts = Vec::new();
        Self::parse_voyager_results(&data, &mut contacts, company);

        // Build debug info
        let keys: Vec<String> = data.as_object()
            .map(|o| o.keys().cloned().collect())
            .unwrap_or_default();
        let el_count = data.get("elements").and_then(|e| e.as_array()).map(|a| a.len()).unwrap_or(0);
        let inc_count = data.get("included").and_then(|i| i.as_array()).map(|a| a.len()).unwrap_or(0);
        let preview = if body_text.len() > 400 { &body_text[..400] } else { &body_text };
        let debug = format!("keys={:?}, elements={}, included={}, body_preview={}", keys, el_count, inc_count, preview);

        Ok((contacts, debug))
    }

    fn parse_voyager_results(data: &serde_json::Value, contacts: &mut Vec<Contact>, company: Option<&str>) {
        // Format 1: data.elements (LinkedIn 2024+ normalized response)
        if let Some(data_obj) = data.get("data") {
            if let Some(elements) = data_obj.get("elements").and_then(|e| e.as_array()) {
                for element in elements {
                    if let Some(items) = element.get("items").and_then(|i| i.as_array()) {
                        for item in items {
                            if let Some(contact) = Self::parse_entity_result(item, company) {
                                contacts.push(contact);
                            }
                        }
                    }
                }
            }
        }

        // Format 2: top-level elements (older format)
        if contacts.is_empty() {
            if let Some(elements) = data.get("elements").and_then(|e| e.as_array()) {
                for element in elements {
                    if let Some(items) = element.get("items").and_then(|i| i.as_array()) {
                        for item in items {
                            if let Some(contact) = Self::parse_entity_result(item, company) {
                                contacts.push(contact);
                            }
                        }
                    }
                }
            }
        }

        // Format 3: "included" array with profile objects
        if contacts.is_empty() {
            if let Some(included) = data.get("included").and_then(|i| i.as_array()) {
                for item in included {
                    let type_name = item.get("$type").and_then(|t| t.as_str()).unwrap_or("");
                    // Match various LinkedIn profile types
                    if type_name.contains("Profile") || type_name.contains("MiniProfile")
                        || type_name.contains("EntityResult") {
                        if let Some(contact) = Self::parse_included_profile(item, company) {
                            contacts.push(contact);
                        }
                    }
                }
            }
        }
    }

    fn parse_included_profile(item: &serde_json::Value, company: Option<&str>) -> Option<Contact> {
        // Try firstName/lastName (MiniProfile / Profile format)
        if let Some(first) = item.get("firstName").and_then(|f| f.as_str()) {
            let last = item.get("lastName").and_then(|l| l.as_str()).unwrap_or("");
            let occupation = item.get("occupation").and_then(|o| o.as_str())
                .or_else(|| item.get("headline").and_then(|h| h.as_str()))
                .unwrap_or("");
            let public_id = item.get("publicIdentifier").and_then(|p| p.as_str());

            if first.is_empty() && last.is_empty() { return None; }

            return Some(Contact {
                id: None,
                linkedin_id: public_id.map(|s| s.to_string()),
                prenom: first.to_string(),
                nom: last.to_string(),
                poste: occupation.to_string(),
                entreprise_siren: None,
                entreprise_nom: company.map(|s| s.to_string()),
                linkedin_url: public_id.map(|id| format!("https://www.linkedin.com/in/{}", id)),
                email: None,
            });
        }

        // Try title.text (EntityResult format in included)
        let title_text = item.get("title")
            .and_then(|t| t.get("text"))
            .and_then(|t| t.as_str())
            .unwrap_or("");
        if title_text.is_empty() { return None; }

        let parts: Vec<&str> = title_text.splitn(2, ' ').collect();
        let prenom = parts.first().unwrap_or(&"").to_string();
        let nom = parts.get(1).unwrap_or(&"").to_string();

        let headline = item.get("primarySubtitle")
            .and_then(|s| s.get("text"))
            .and_then(|t| t.as_str())
            .or_else(|| item.get("headline").and_then(|h| h.get("text")).and_then(|t| t.as_str()))
            .unwrap_or("")
            .to_string();

        let nav_url = item.get("navigationUrl")
            .and_then(|u| u.as_str())
            .map(|u| u.split('?').next().unwrap_or(u).to_string());

        let public_id = nav_url.as_ref()
            .and_then(|u| u.strip_prefix("https://www.linkedin.com/in/"))
            .map(|s| s.trim_end_matches('/').to_string());

        Some(Contact {
            id: None,
            linkedin_id: public_id,
            prenom,
            nom,
            poste: headline,
            entreprise_siren: None,
            entreprise_nom: company.map(|s| s.to_string()),
            linkedin_url: nav_url,
            email: None,
        })
    }

    fn parse_entity_result(item: &serde_json::Value, company: Option<&str>) -> Option<Contact> {
        let entity = item.get("item")
            .and_then(|i| i.get("entityResult"))?;

        let title_text = entity.get("title")
            .and_then(|t| t.get("text"))
            .and_then(|t| t.as_str())
            .unwrap_or("");
        if title_text.is_empty() { return None; }

        let parts: Vec<&str> = title_text.splitn(2, ' ').collect();
        let prenom = parts.first().unwrap_or(&"").to_string();
        let nom = parts.get(1).unwrap_or(&"").to_string();

        let headline = entity.get("primarySubtitle")
            .and_then(|s| s.get("text"))
            .and_then(|t| t.as_str())
            .unwrap_or("")
            .to_string();

        let nav_url = entity.get("navigationUrl")
            .and_then(|u| u.as_str())
            .map(|u| u.split('?').next().unwrap_or(u).to_string());

        let public_id = nav_url.as_ref()
            .and_then(|u| u.strip_prefix("https://www.linkedin.com/in/"))
            .map(|s| s.trim_end_matches('/').to_string());

        Some(Contact {
            id: None,
            linkedin_id: public_id,
            prenom,
            nom,
            poste: headline,
            entreprise_siren: None,
            entreprise_nom: company.map(|s| s.to_string()),
            linkedin_url: nav_url,
            email: None,
        })
    }

    fn parse_mini_profile(item: &serde_json::Value, company: Option<&str>) -> Option<Contact> {
        let first = item.get("firstName").and_then(|f| f.as_str())?;
        let last = item.get("lastName").and_then(|l| l.as_str()).unwrap_or("");
        let occupation = item.get("occupation").and_then(|o| o.as_str()).unwrap_or("");
        let public_id = item.get("publicIdentifier").and_then(|p| p.as_str());

        Some(Contact {
            id: None,
            linkedin_id: public_id.map(|s| s.to_string()),
            prenom: first.to_string(),
            nom: last.to_string(),
            poste: occupation.to_string(),
            entreprise_siren: None,
            entreprise_nom: company.map(|s| s.to_string()),
            linkedin_url: public_id.map(|id| format!("https://www.linkedin.com/in/{}", id)),
            email: None,
        })
    }

    /// Envoie un message LinkedIn à un contact via Voyager API
    pub async fn send_message(&self, recipient_id: &str, body: &str) -> Result<()> {
        let cookie = self.cookie_li_at.as_ref()
            .context("Envoi LinkedIn nécessite le cookie li_at.")?;

        let http_client = reqwest::Client::builder()
            .cookie_store(true)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        let voyager_headers = |req: reqwest::RequestBuilder, cookie: &str| -> reqwest::RequestBuilder {
            req.header("Cookie", format!("li_at={}; JSESSIONID=\"ajax:0\"", cookie))
                .header("Csrf-Token", "ajax:0")
                .header("X-Li-Lang", "fr_FR")
                .header("X-Li-Track", "{\"clientVersion\":\"1.13.8622\"}")
                .header("X-Restli-Protocol-Version", "2.0.0")
                .header("Accept", "application/vnd.linkedin.normalized+json+2.1")
                .header("User-Agent", "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/125.0.0.0 Safari/537.36")
                .timeout(std::time::Duration::from_secs(15))
        };

        // Step 1: Find or create conversation with this person
        // Use the conversations endpoint with participant public ID
        let conv_url = format!(
            "https://www.linkedin.com/voyager/api/messaging/conversations?\
            keyVersion=LEGACY_INBOX&q=participants&recipients=List({})",
            urlencoding::encode(recipient_id)
        );

        let conv_resp = voyager_headers(http_client.get(&conv_url), cookie)
            .send()
            .await
            .context("Erreur recherche conversation LinkedIn")?;

        let conv_status = conv_resp.status();

        if conv_status.as_u16() == 302 || conv_status.as_u16() == 401 || conv_status.as_u16() == 403 {
            anyhow::bail!("Cookie li_at expiré. Reconnectez-vous à LinkedIn et mettez à jour le cookie.");
        }

        // Step 2: Send message via legacy messaging API
        let send_url = "https://www.linkedin.com/voyager/api/messaging/conversations";

        let payload = serde_json::json!({
            "keyVersion": "LEGACY_INBOX",
            "conversationCreate": {
                "eventCreate": {
                    "value": {
                        "com.linkedin.voyager.messaging.create.MessageCreate": {
                            "body": body,
                            "attachments": []
                        }
                    }
                },
                "recipients": [recipient_id],
                "subtype": "MEMBER_TO_MEMBER"
            }
        });

        let resp = voyager_headers(
            http_client.post(send_url).header("Content-Type", "application/json"),
            cookie
        )
            .json(&payload)
            .send()
            .await
            .context("Erreur d'envoi de message LinkedIn")?;

        let status = resp.status();
        let resp_body = resp.text().await.unwrap_or_default();

        if status.as_u16() == 302 || status.as_u16() == 401 || status.as_u16() == 403 {
            anyhow::bail!("Cookie li_at expiré (HTTP {}). Reconnectez-vous à LinkedIn.", status);
        }

        if !status.is_success() && status.as_u16() != 201 {
            anyhow::bail!("Envoi LinkedIn {} : {}", status,
                if resp_body.len() > 400 { &resp_body[..400] } else { &resp_body });
        }

        Ok(())
    }

    #[allow(dead_code)]
    fn extract_member_id(data: &serde_json::Value) -> Option<String> {
        for path in &[
            vec!["entityUrn"],
            vec!["data", "entityUrn"],
            vec!["miniProfile", "entityUrn"],
            vec!["data", "miniProfile", "entityUrn"],
        ] {
            let mut current = data;
            let mut found = true;
            for key in path {
                match current.get(key) {
                    Some(v) => current = v,
                    None => { found = false; break; }
                }
            }
            if found {
                if let Some(urn) = current.as_str() {
                    if let Some(id) = urn.rsplit(':').next() {
                        return Some(id.to_string());
                    }
                }
            }
        }

        if let Some(included) = data.get("included").and_then(|i| i.as_array()) {
            for item in included {
                let type_name = item.get("$type").and_then(|t| t.as_str()).unwrap_or("");
                if type_name.contains("Profile") {
                    if let Some(urn) = item.get("entityUrn").and_then(|u| u.as_str()) {
                        if let Some(id) = urn.rsplit(':').next() {
                            return Some(id.to_string());
                        }
                    }
                }
            }
        }

        None
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
