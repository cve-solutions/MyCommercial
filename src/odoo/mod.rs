use anyhow::{Result, Context};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::models::{Contact, MessageStatus};
use crate::settings::SettingsManager;

/// Client Odoo CRM via JSON-RPC
pub struct OdooClient {
    client: Client,
    url: String,
    database: String,
    uid: Option<i64>,
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
            client: Client::new(),
            url: settings.odoo_url(),
            database: settings.get_or_default("odoo", "database", ""),
            uid: None,
            password: settings.get_or_default("odoo", "password", ""),
            enabled: settings.odoo_enabled(),
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Authentification via JSON-RPC
    pub async fn authenticate(&mut self) -> Result<()> {
        if !self.enabled {
            anyhow::bail!("Intégration Odoo désactivée");
        }

        let username = self.password.clone(); // Will be fixed below
        let url = format!("{}/jsonrpc", self.url);

        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "call".to_string(),
            id: 1,
            params: serde_json::json!({
                "service": "common",
                "method": "authenticate",
                "args": [&self.database, &username, &self.password, {}]
            }),
        };

        let resp = self.client
            .post(&url)
            .json(&request)
            .send()
            .await
            .context("Impossible de se connecter à Odoo")?;

        let data: JsonRpcResponse = resp.json().await
            .context("Erreur parsing réponse Odoo")?;

        if let Some(error) = data.error {
            anyhow::bail!("Erreur Odoo: {}", error.message.unwrap_or_else(|| "Inconnue".into()));
        }

        self.uid = data.result.and_then(|v| v.as_i64());
        if self.uid.is_none() {
            anyhow::bail!("Authentification Odoo échouée: UID non reçu");
        }

        Ok(())
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

        let uid = self.uid.context("Non authentifié. Appelez authenticate() d'abord.")?;
        let url = format!("{}/jsonrpc", self.url);

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

        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "call".to_string(),
            id: 2,
            params: serde_json::json!({
                "service": "object",
                "method": "execute_kw",
                "args": [
                    &self.database,
                    uid,
                    &self.password,
                    "crm.lead",
                    "create",
                    [lead_data]
                ]
            }),
        };

        let resp = self.client
            .post(&url)
            .json(&request)
            .send()
            .await
            .context("Erreur création lead Odoo")?;

        let data: JsonRpcResponse = resp.json().await?;

        if let Some(error) = data.error {
            anyhow::bail!("Erreur Odoo: {}", error.message.unwrap_or_else(|| "Inconnue".into()));
        }

        data.result
            .and_then(|v| v.as_i64())
            .context("ID du lead non reçu")
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

        let uid = self.uid.context("Non authentifié")?;
        let url = format!("{}/jsonrpc", self.url);

        let probability = match status {
            MessageStatus::Interested => 70.0,
            MessageStatus::Replied => 30.0,
            MessageStatus::NotInterested => 0.0,
            MessageStatus::NoResponse => 5.0,
            _ => 10.0,
        };

        let active = !matches!(status, MessageStatus::NotInterested);

        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "call".to_string(),
            id: 3,
            params: serde_json::json!({
                "service": "object",
                "method": "execute_kw",
                "args": [
                    &self.database,
                    uid,
                    &self.password,
                    "crm.lead",
                    "write",
                    [[lead_id], {
                        "probability": probability,
                        "active": active,
                    }]
                ]
            }),
        };

        let resp = self.client
            .post(&url)
            .json(&request)
            .send()
            .await
            .context("Erreur mise à jour lead Odoo")?;

        let data: JsonRpcResponse = resp.json().await?;
        if let Some(error) = data.error {
            anyhow::bail!("Erreur Odoo: {}", error.message.unwrap_or_else(|| "Inconnue".into()));
        }

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

        let uid = self.uid.context("Non authentifié")?;
        let url = format!("{}/jsonrpc", self.url);

        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "call".to_string(),
            id: 4,
            params: serde_json::json!({
                "service": "object",
                "method": "execute_kw",
                "args": [
                    &self.database,
                    uid,
                    &self.password,
                    "mail.message",
                    "create",
                    [{
                        "model": "crm.lead",
                        "res_id": lead_id,
                        "body": note,
                        "message_type": "comment",
                        "subtype_xmlid": "mail.mt_note",
                    }]
                ]
            }),
        };

        self.client.post(&url).json(&request).send().await?;
        Ok(())
    }
}
