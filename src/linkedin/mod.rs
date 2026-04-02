use anyhow::{Result, Context};
use reqwest::Client;
use reqwest::cookie::CookieStore;
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

        Ok(Self {
            client: Client::new(),
            auth_method,
            access_token,
            cookie_li_at,
        })
    }

    pub fn is_authenticated(&self) -> bool {
        self.cookie_li_at.is_some() || self.access_token.is_some()
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
            // Store entityUrn as linkedin_id for messaging (urn:li:fs_miniProfile:ABC)
            let entity_urn = item.get("entityUrn").and_then(|u| u.as_str());
            let linkedin_id = entity_urn
                .map(|s| s.to_string())
                .or_else(|| public_id.map(|s| s.to_string()));

            if first.is_empty() && last.is_empty() { return None; }

            return Some(Contact {
                id: None,
                linkedin_id,
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

    /// Login LinkedIn avec email/password et retourne le cookie li_at
    pub async fn login_get_cookie(email: &str, password: &str) -> Result<String> {
        let jar = std::sync::Arc::new(reqwest::cookie::Jar::default());
        let http_client = reqwest::Client::builder()
            .cookie_provider(jar.clone())
            .redirect(reqwest::redirect::Policy::limited(10))
            .user_agent("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/125.0.0.0 Safari/537.36")
            .build()
            .context("Erreur création client HTTP")?;

        // Step 1: GET login page to extract CSRF token
        let login_page = http_client
            .get("https://www.linkedin.com/login")
            .timeout(std::time::Duration::from_secs(15))
            .send()
            .await
            .context("Impossible d'accéder à la page de login LinkedIn")?;

        let login_html = login_page.text().await.unwrap_or_default();

        // Extract loginCsrfParam from HTML
        let csrf_token = login_html
            .split("loginCsrfParam")
            .nth(1)
            .and_then(|s| s.split("value=\"").nth(1))
            .and_then(|s| s.split('"').next())
            .context("CSRF token introuvable sur la page de login LinkedIn")?
            .to_string();

        // Step 2: POST login form
        let _login_resp = http_client
            .post("https://www.linkedin.com/checkpoint/lg/login-submit")
            .header("Content-Type", "application/x-www-form-urlencoded")
            .header("Referer", "https://www.linkedin.com/login")
            .header("Origin", "https://www.linkedin.com")
            .form(&[
                ("session_key", email),
                ("session_password", password),
                ("loginCsrfParam", csrf_token.as_str()),
            ])
            .timeout(std::time::Duration::from_secs(20))
            .send()
            .await
            .context("Erreur lors du login LinkedIn")?;

        // Step 3: Check cookie jar for li_at
        let linkedin_url = reqwest::Url::parse("https://www.linkedin.com").unwrap();
        let cookies_str = jar.cookies(&linkedin_url)
            .map(|h| h.to_str().unwrap_or("").to_string())
            .unwrap_or_default();

        // Parse li_at from cookie string "li_at=ABC; other=DEF"
        if let Some(li_at) = Self::extract_cookie_value(&cookies_str, "li_at") {
            return Ok(li_at);
        }

        // Step 4: Try accessing feed to trigger more cookie setting
        let _feed_resp = http_client
            .get("https://www.linkedin.com/feed/")
            .timeout(std::time::Duration::from_secs(15))
            .send()
            .await
            .context("Erreur accès feed LinkedIn après login")?;

        let cookies_str = jar.cookies(&linkedin_url)
            .map(|h| h.to_str().unwrap_or("").to_string())
            .unwrap_or_default();

        if let Some(li_at) = Self::extract_cookie_value(&cookies_str, "li_at") {
            return Ok(li_at);
        }

        // Check if login failed
        let resp_url = _feed_resp.url().to_string();
        if resp_url.contains("checkpoint") || resp_url.contains("challenge") {
            anyhow::bail!("LinkedIn demande une vérification (2FA/captcha). Connectez-vous dans un navigateur puis copiez le cookie li_at manuellement.");
        }
        if resp_url.contains("login") {
            anyhow::bail!("Identifiants LinkedIn incorrects (redirigé vers login).");
        }

        anyhow::bail!("Cookie li_at non obtenu. Cookies reçus: {} — URL finale: {}",
            if cookies_str.len() > 100 { &cookies_str[..100] } else { &cookies_str },
            resp_url);
    }

    fn extract_cookie_value(cookies_str: &str, name: &str) -> Option<String> {
        let prefix = format!("{}=", name);
        cookies_str.split("; ")
            .find(|c| c.starts_with(&prefix))
            .map(|c| c[prefix.len()..].to_string())
            .filter(|v| !v.is_empty() && v != "\"\"")
    }

    fn voyager_request(&self, client: &reqwest::Client, method: reqwest::Method, url: &str) -> Result<reqwest::RequestBuilder> {
        let cookie = self.cookie_li_at.as_ref()
            .context("LinkedIn nécessite le cookie li_at.")?;
        Ok(client.request(method, url)
            .header("Cookie", format!("li_at={}; JSESSIONID=\"ajax:0\"", cookie))
            .header("Csrf-Token", "ajax:0")
            .header("X-Li-Lang", "fr_FR")
            .header("X-Li-Track", "{\"clientVersion\":\"1.13.8622\"}")
            .header("X-Restli-Protocol-Version", "2.0.0")
            .header("Accept", "application/vnd.linkedin.normalized+json+2.1")
            .header("User-Agent", "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/125.0.0.0 Safari/537.36")
            .timeout(std::time::Duration::from_secs(15)))
    }

    /// Envoie un message LinkedIn à un contact via Voyager API
    /// recipient_id peut être un URN (urn:li:fs_miniProfile:ABC) ou un publicIdentifier (cyrille-verger)
    pub async fn send_message(&self, recipient_id: &str, body: &str) -> Result<()> {
        let http_client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        // Resolve recipient to proper URN
        let recipient_urn = if recipient_id.starts_with("urn:li:") {
            recipient_id.to_string()
        } else {
            // Resolve publicIdentifier to URN via search
            let search_url = format!(
                "https://www.linkedin.com/voyager/api/search/dash/clusters\
                ?decorationId=com.linkedin.voyager.dash.deco.search.SearchClusterCollection-165\
                &origin=GLOBAL_SEARCH_HEADER&q=all\
                &query=(keywords:{},flagshipSearchIntent:SEARCH_SRP,\
queryParameters:(resultType:List(PEOPLE)))&count=1",
                urlencoding::encode(recipient_id)
            );
            let search_resp = self.voyager_request(&http_client, reqwest::Method::GET, &search_url)?
                .send()
                .await
                .context("Erreur recherche profil LinkedIn")?;

            if !search_resp.status().is_success() {
                anyhow::bail!("Impossible de résoudre le profil '{}' (HTTP {}). Resauvez le contact depuis une nouvelle recherche LinkedIn.",
                    recipient_id, search_resp.status());
            }

            let data: serde_json::Value = search_resp.json().await.unwrap_or_default();

            // Look for entityUrn in included profiles
            let urn = data.get("included").and_then(|inc| inc.as_array())
                .and_then(|items| {
                    items.iter().find_map(|item| {
                        let t = item.get("$type").and_then(|t| t.as_str()).unwrap_or("");
                        if !t.contains("Profile") { return None; }
                        let pub_id = item.get("publicIdentifier").and_then(|p| p.as_str())?;
                        if pub_id == recipient_id {
                            item.get("entityUrn").and_then(|u| u.as_str()).map(|s| s.to_string())
                        } else {
                            None
                        }
                    })
                });

            match urn {
                Some(u) => u,
                None => anyhow::bail!(
                    "Profil '{}' non trouvé dans les résultats LinkedIn. Resauvez le contact depuis une recherche.",
                    recipient_id),
            }
        };

        // Send message
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
                "recipients": [&recipient_urn],
                "subtype": "MEMBER_TO_MEMBER"
            }
        });

        let resp = self.voyager_request(&http_client, reqwest::Method::POST,
            "https://www.linkedin.com/voyager/api/messaging/conversations")?
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await
            .context("Erreur d'envoi de message LinkedIn")?;

        let status = resp.status();
        let resp_body = resp.text().await.unwrap_or_default();

        if status.as_u16() == 302 || status.as_u16() == 401 || status.as_u16() == 403 {
            anyhow::bail!("LinkedIn auth échouée (HTTP {}). Vérifiez le cookie li_at.", status);
        }

        if !status.is_success() && status.as_u16() != 201 {
            anyhow::bail!("Envoi LinkedIn {} (recipient='{}') : {}", status, recipient_urn,
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
