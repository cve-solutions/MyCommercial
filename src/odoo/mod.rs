use anyhow::{Result, Context};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::models::{Contact, MessageStatus};
use crate::settings::SettingsManager;

/// Client Odoo CRM via JSON-RPC (compatible Odoo 14-19)
pub struct OdooClient {
    client: Client,
    url: String,
    database: String,
    uid: Option<i64>,
    username: String,
    password: String,
    enabled: bool,
}

#[derive(Debug, Serialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    method: String,
    id: u32,
    params: Value,
}

#[derive(Debug, Deserialize)]
struct JsonRpcResponse {
    result: Option<Value>,
    error: Option<JsonRpcError>,
}

#[derive(Debug, Deserialize)]
struct JsonRpcError {
    message: Option<String>,
    data: Option<Value>,
}

impl OdooClient {
    pub fn new(settings: &SettingsManager) -> Self {
        Self {
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(15))
                .build()
                .unwrap_or_else(|_| Client::new()),
            url: settings.odoo_url().trim().trim_end_matches('/').to_string(),
            database: settings.get_or_default("odoo", "database", "").trim().to_string(),
            uid: None,
            username: settings.get_or_default("odoo", "username", "").trim().to_string(),
            password: settings.get_or_default("odoo", "password", ""),
            enabled: settings.odoo_enabled(),
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Envoi d'une requête JSON-RPC à Odoo
    async fn rpc(&self, endpoint: &str, params: Value) -> Result<JsonRpcResponse> {
        let url = format!("{}{}", self.url, endpoint);
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "call".to_string(),
            id: 1,
            params,
        };

        let resp = self.client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .context(format!("Connexion impossible à {} — vérifiez URL et réseau", url))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("Odoo HTTP {} sur {}: {}", status, url,
                if body.len() > 200 { &body[..200] } else { &body });
        }

        resp.json().await
            .context("Réponse Odoo invalide (pas du JSON-RPC)")
    }

    /// Extraction d'erreur JSON-RPC
    fn check_error(data: &JsonRpcResponse) -> Result<()> {
        if let Some(ref error) = data.error {
            let msg = error.message.as_deref().unwrap_or("Inconnue");
            let detail = error.data.as_ref()
                .and_then(|d| d.get("message").and_then(|m| m.as_str()))
                .map(|d| format!(" — {}", d))
                .unwrap_or_default();
            anyhow::bail!("Erreur Odoo: {}{}", msg, detail);
        }
        Ok(())
    }

    /// Authentification (compatible Odoo 14-19)
    pub async fn authenticate(&mut self) -> Result<()> {
        if !self.enabled {
            anyhow::bail!("Intégration Odoo désactivée");
        }
        if self.url.is_empty() || self.database.is_empty() || self.username.is_empty() {
            anyhow::bail!("Configuration Odoo incomplète (url, database et username requis)");
        }

        // Odoo 17+/19: /web/session/authenticate
        let data = self.rpc("/web/session/authenticate", serde_json::json!({
            "db": &self.database,
            "login": &self.username,
            "password": &self.password,
        })).await?;

        Self::check_error(&data)?;

        self.uid = data.result.as_ref()
            .and_then(|v| v.get("uid").and_then(|u| u.as_i64()));

        if self.uid.is_none() {
            anyhow::bail!(
                "Authentification échouée (database='{}', user='{}') — vérifiez vos identifiants",
                self.database, self.username
            );
        }

        Ok(())
    }

    /// Appel ORM via /web/dataset/call_kw (Odoo 17+/19)
    async fn call_kw(&self, model: &str, method: &str, args: Value, kwargs: Value) -> Result<Value> {
        let uid = self.uid.context("Non authentifié. Appelez authenticate() d'abord.")?;
        let _ = uid; // uid is in the session cookie after authenticate

        let data = self.rpc("/web/dataset/call_kw", serde_json::json!({
            "model": model,
            "method": method,
            "args": args,
            "kwargs": kwargs,
        })).await?;

        Self::check_error(&data)?;
        data.result.context("Pas de résultat dans la réponse Odoo")
    }

    /// Crée un lead/opportunité dans le CRM Odoo
    pub async fn create_lead(
        &self,
        contact: &Contact,
        solution_name: &str,
        message: &str,
    ) -> Result<i64> {
        if !self.enabled {
            anyhow::bail!("Intégration Odoo désactivée");
        }

        let lead_data = serde_json::json!({
            "name": format!("Prospection {} {} - {}", contact.prenom, contact.nom, solution_name),
            "contact_name": format!("{} {}", contact.prenom, contact.nom),
            "function": contact.poste,
            "partner_name": contact.entreprise_nom.as_deref().unwrap_or(""),
            "description": message,
            "type": "opportunity",
            "website": contact.linkedin_url.as_deref().unwrap_or(""),
            "email_from": contact.email.as_deref().unwrap_or(""),
        });

        let result = self.call_kw(
            "crm.lead", "create",
            serde_json::json!([lead_data]),
            serde_json::json!({}),
        ).await?;

        result.as_i64().context("ID du lead non reçu")
    }

    /// Met à jour le stage d'un lead (Intéressé/KO)
    pub async fn update_lead_status(
        &self,
        lead_id: i64,
        status: &MessageStatus,
    ) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        let probability = match status {
            MessageStatus::Interested => 70.0,
            MessageStatus::Replied => 30.0,
            MessageStatus::NotInterested => 0.0,
            MessageStatus::NoResponse => 5.0,
            _ => 10.0,
        };

        let active = !matches!(status, MessageStatus::NotInterested);

        self.call_kw(
            "crm.lead", "write",
            serde_json::json!([[lead_id], {
                "probability": probability,
                "active": active,
            }]),
            serde_json::json!({}),
        ).await?;

        Ok(())
    }

    /// Ajoute une note/log à un lead existant
    pub async fn add_lead_note(
        &self,
        lead_id: i64,
        note: &str,
    ) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        self.call_kw(
            "mail.message", "create",
            serde_json::json!([{
                "model": "crm.lead",
                "res_id": lead_id,
                "body": note,
                "message_type": "comment",
                "subtype_xmlid": "mail.mt_note",
            }]),
            serde_json::json!({}),
        ).await?;

        Ok(())
    }
}
