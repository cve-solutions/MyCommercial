use anyhow::{Result, Context};
use reqwest::Client;
use serde::{Deserialize, Serialize};

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

    /// Génère l'URL d'autorisation OAuth2 et ouvre le navigateur
    pub fn oauth2_auth_url(client_id: &str, redirect_uri: &str) -> String {
        let url = format!(
            "https://www.linkedin.com/oauth/v2/authorization?response_type=code&client_id={}&redirect_uri={}&scope=r_liteprofile%20r_emailaddress%20w_member_social",
            urlencoding::encode(client_id),
            urlencoding::encode(redirect_uri),
        );
        // Try to open in browser
        let _ = open::that(&url);
        url
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

        let data: serde_json::Value = resp.json().await?;
        data["access_token"]
            .as_str()
            .map(|s| s.to_string())
            .context("Token non trouvé dans la réponse OAuth2")
    }
}
