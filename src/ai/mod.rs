use anyhow::{Result, Context};
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::models::OllamaModel;
use crate::settings::SettingsManager;

/// Client Ollama pour résumer les solutions et générer des messages
pub struct OllamaClient {
    client: Client,
    base_url: String,
    model: String,
    temperature: f64,
    max_tokens: u32,
    system_prompt: String,
}

#[derive(Debug, Deserialize)]
struct OllamaTagsResponse {
    models: Option<Vec<OllamaModelInfo>>,
}

#[derive(Debug, Deserialize)]
struct OllamaModelInfo {
    name: Option<String>,
    size: Option<u64>,
    details: Option<OllamaModelDetails>,
}

#[derive(Debug, Deserialize)]
struct OllamaModelDetails {
    parameter_size: Option<String>,
    family: Option<String>,
}

#[derive(Debug, Serialize)]
struct OllamaGenerateRequest {
    model: String,
    prompt: String,
    system: Option<String>,
    stream: bool,
    options: OllamaOptions,
}

#[derive(Debug, Serialize)]
struct OllamaOptions {
    temperature: f64,
    num_predict: u32,
}

#[derive(Debug, Deserialize)]
struct OllamaGenerateResponse {
    response: Option<String>,
    done: Option<bool>,
}

impl OllamaClient {
    pub fn new(settings: &SettingsManager) -> Self {
        Self {
            client: Client::new(),
            base_url: settings.ollama_url(),
            model: settings.ollama_model(),
            temperature: settings.ollama_temperature(),
            max_tokens: settings.get_u32("ollama", "max_tokens", 2048),
            system_prompt: settings.ollama_system_prompt(),
        }
    }

    /// Liste les modèles installés localement
    pub async fn list_models(&self) -> Result<Vec<OllamaModel>> {
        let url = format!("{}/api/tags", self.base_url);
        let resp = self.client
            .get(&url)
            .send()
            .await
            .context("Impossible de se connecter à Ollama. Vérifiez qu'Ollama est démarré.")?;

        let data: OllamaTagsResponse = resp.json().await
            .context("Erreur parsing réponse Ollama")?;

        let models = data.models.unwrap_or_default().into_iter().map(|m| {
            OllamaModel {
                name: m.name.unwrap_or_default(),
                size: m.size.unwrap_or(0),
                parameter_size: m.details.as_ref().and_then(|d| d.parameter_size.clone()),
                family: m.details.and_then(|d| d.family),
            }
        }).collect();

        Ok(models)
    }

    /// Auto-sélectionne le meilleur modèle pour la tâche commerciale
    /// Préfère: mistral, llama, gemma (modèles conversationnels)
    pub async fn auto_select_model(&mut self) -> Result<String> {
        let models = self.list_models().await?;
        if models.is_empty() {
            anyhow::bail!("Aucun modèle Ollama installé. Installez un modèle avec: ollama pull mistral");
        }

        // Priorité: modèles orientés chat/instruction
        let preferred_families = ["mistral", "llama", "gemma", "phi", "qwen", "deepseek"];
        let preferred_sizes = ["7b", "8b", "13b", "14b"]; // Tailles raisonnables

        let mut best: Option<&OllamaModel> = None;
        let mut best_score = 0i32;

        for model in &models {
            let name_lower = model.name.to_lowercase();
            let mut score = 0i32;

            // Bonus pour famille préférée
            for (i, family) in preferred_families.iter().enumerate() {
                if name_lower.contains(family) {
                    score += (preferred_families.len() - i) as i32 * 10;
                    break;
                }
            }

            // Bonus pour taille raisonnable
            if let Some(ref param_size) = model.parameter_size {
                let ps = param_size.to_lowercase();
                for (i, size) in preferred_sizes.iter().enumerate() {
                    if ps.contains(size) {
                        score += (preferred_sizes.len() - i) as i32 * 5;
                        break;
                    }
                }
            }

            // Bonus pour modèles "instruct" ou "chat"
            if name_lower.contains("instruct") || name_lower.contains("chat") {
                score += 15;
            }

            if score > best_score {
                best_score = score;
                best = Some(model);
            }
        }

        let selected = best.unwrap_or(&models[0]);
        self.model = selected.name.clone();
        Ok(selected.name.clone())
    }

    /// Génère un résumé d'une solution pour la prospection
    pub async fn summarize_solution(&self, document_content: &str) -> Result<String> {
        let prompt = format!(
            "Résume la solution suivante en 2-3 phrases percutantes pour un décideur (CEO/CTO/RSSI). \
             Mets en avant les bénéfices business et la valeur ajoutée.\n\n\
             Document:\n{}\n\nRésumé:",
            document_content
        );

        self.generate(&prompt).await
    }

    /// Génère un message de prospection personnalisé
    pub async fn generate_prospection_message(
        &self,
        contact_prenom: &str,
        contact_poste: &str,
        entreprise_nom: &str,
        solution_resume: &str,
        template: &str,
    ) -> Result<String> {
        let prompt = format!(
            "Personnalise le message de prospection suivant pour :\n\
             - Prénom: {}\n- Poste: {}\n- Entreprise: {}\n- Solution: {}\n\n\
             Template:\n{}\n\n\
             Génère un message professionnel, personnalisé et engageant. \
             Garde un ton courtois et direct. Maximum 500 caractères.",
            contact_prenom, contact_poste, entreprise_nom, solution_resume, template
        );

        self.generate(&prompt).await
    }

    /// Appel générique à Ollama
    pub async fn generate(&self, prompt: &str) -> Result<String> {
        if self.model.is_empty() {
            anyhow::bail!("Aucun modèle Ollama sélectionné. Configurez-le dans Settings > Ollama.");
        }

        let url = format!("{}/api/generate", self.base_url);
        let request = OllamaGenerateRequest {
            model: self.model.clone(),
            prompt: prompt.to_string(),
            system: Some(self.system_prompt.clone()),
            stream: false,
            options: OllamaOptions {
                temperature: self.temperature,
                num_predict: self.max_tokens,
            },
        };

        let resp = self.client
            .post(&url)
            .json(&request)
            .send()
            .await
            .context("Impossible de se connecter à Ollama")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("Ollama error {}: {}", status, body);
        }

        let data: OllamaGenerateResponse = resp.json().await
            .context("Erreur parsing réponse Ollama")?;

        data.response.context("Pas de réponse d'Ollama")
    }

    pub fn current_model(&self) -> &str {
        &self.model
    }

    pub fn set_model(&mut self, model: String) {
        self.model = model;
    }
}
