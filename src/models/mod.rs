use serde::{Deserialize, Serialize};

// ── Entreprise (from DataGouv / API Entreprise) ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entreprise {
    pub siren: String,
    pub siret: Option<String>,
    pub nom: String,
    pub code_ape: String,
    pub libelle_ape: String,
    pub tranche_effectifs: Option<String>,
    pub categorie_entreprise: Option<String>, // PME, ETI, GE
    pub adresse: Option<String>,
    pub code_postal: Option<String>,
    pub ville: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrancheEffectifs {
    pub code: String,
    pub libelle: String,
    pub min: u32,
    pub max: Option<u32>,
}

impl TrancheEffectifs {
    pub fn all() -> Vec<Self> {
        vec![
            Self { code: "00".into(), libelle: "0 salarié".into(), min: 0, max: Some(0) },
            Self { code: "01".into(), libelle: "1 ou 2 salariés".into(), min: 1, max: Some(2) },
            Self { code: "02".into(), libelle: "3 à 5 salariés".into(), min: 3, max: Some(5) },
            Self { code: "03".into(), libelle: "6 à 9 salariés".into(), min: 6, max: Some(9) },
            Self { code: "11".into(), libelle: "10 à 19 salariés".into(), min: 10, max: Some(19) },
            Self { code: "12".into(), libelle: "20 à 49 salariés".into(), min: 20, max: Some(49) },
            Self { code: "21".into(), libelle: "50 à 99 salariés".into(), min: 50, max: Some(99) },
            Self { code: "22".into(), libelle: "100 à 199 salariés".into(), min: 100, max: Some(199) },
            Self { code: "31".into(), libelle: "200 à 249 salariés".into(), min: 200, max: Some(249) },
            Self { code: "32".into(), libelle: "250 à 499 salariés".into(), min: 250, max: Some(499) },
            Self { code: "41".into(), libelle: "500 à 999 salariés".into(), min: 500, max: Some(999) },
            Self { code: "42".into(), libelle: "1000 à 1999 salariés".into(), min: 1000, max: Some(1999) },
            Self { code: "51".into(), libelle: "2000 à 4999 salariés".into(), min: 2000, max: Some(4999) },
            Self { code: "52".into(), libelle: "5000 à 9999 salariés".into(), min: 5000, max: Some(9999) },
            Self { code: "53".into(), libelle: "10000 salariés et plus".into(), min: 10000, max: None },
        ]
    }
}

// ── LinkedIn Contact ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Contact {
    pub id: Option<i64>,
    pub linkedin_id: Option<String>,
    pub prenom: String,
    pub nom: String,
    pub poste: String,        // CEO, CTO, RSSI, etc.
    pub entreprise_siren: Option<String>,
    pub entreprise_nom: Option<String>,
    pub linkedin_url: Option<String>,
    pub email: Option<String>,
}

// ── Prospection Message ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageStatus {
    Draft,
    Sent,
    Delivered,
    Read,
    Replied,
    Interested,
    NotInterested,
    NoResponse,
}

impl MessageStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Draft => "Brouillon",
            Self::Sent => "Envoyé",
            Self::Delivered => "Délivré",
            Self::Read => "Lu",
            Self::Replied => "Répondu",
            Self::Interested => "Intéressé",
            Self::NotInterested => "Pas intéressé",
            Self::NoResponse => "Sans réponse",
        }
    }

    pub fn from_db(s: &str) -> Self {
        match s {
            "draft" => Self::Draft,
            "sent" => Self::Sent,
            "delivered" => Self::Delivered,
            "read" => Self::Read,
            "replied" => Self::Replied,
            "interested" => Self::Interested,
            "not_interested" => Self::NotInterested,
            "no_response" => Self::NoResponse,
            _ => Self::Draft,
        }
    }

    pub fn to_db(&self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Sent => "sent",
            Self::Delivered => "delivered",
            Self::Read => "read",
            Self::Replied => "replied",
            Self::Interested => "interested",
            Self::NotInterested => "not_interested",
            Self::NoResponse => "no_response",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProspectionMessage {
    pub id: Option<i64>,
    pub contact_id: i64,
    pub contenu: String,
    pub status: MessageStatus,
    pub date_envoi: Option<String>,
    pub date_reponse: Option<String>,
    pub solution_id: Option<i64>,
    pub odoo_lead_id: Option<i64>,
}

// ── Solution Document ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Solution {
    pub id: Option<i64>,
    pub nom: String,
    pub description: String,
    pub fichier_path: Option<String>,
    pub resume_ia: Option<String>,
    pub date_creation: Option<String>,
}

// ── Search Criteria ──

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SearchCriteria {
    pub postes: Vec<String>,           // CEO, CTO, RSSI...
    pub codes_ape: Vec<String>,        // Filtrer par code APE
    pub tranches_effectifs: Vec<String>, // Filtrer par taille
    pub regions: Vec<String>,          // Filtrer par région
    pub mots_cles: Option<String>,     // Mots-clés supplémentaires
}

// ── Rapport Stats ──

#[derive(Debug, Clone, Default)]
pub struct RapportStats {
    pub total_contacts: u32,
    pub messages_envoyes: u32,
    pub messages_lus: u32,
    pub reponses: u32,
    pub interesses: u32,
    pub pas_interesses: u32,
    pub sans_reponse: u32,
    pub taux_reponse: f64,
    pub taux_interet: f64,
}

// ── Ollama Model Info ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaModel {
    pub name: String,
    pub size: u64,
    pub parameter_size: Option<String>,
    pub family: Option<String>,
}

// ── LinkedIn Auth Method ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LinkedInAuthMethod {
    OAuth2,
    Cookie,
    ApiKey,
}

impl LinkedInAuthMethod {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::OAuth2 => "OAuth2",
            Self::Cookie => "Cookie (li_at)",
            Self::ApiKey => "API Key",
        }
    }

    pub fn from_db(s: &str) -> Self {
        match s {
            "oauth2" => Self::OAuth2,
            "cookie" => Self::Cookie,
            "api_key" => Self::ApiKey,
            _ => Self::OAuth2,
        }
    }

    pub fn to_db(&self) -> &'static str {
        match self {
            Self::OAuth2 => "oauth2",
            Self::Cookie => "cookie",
            Self::ApiKey => "api_key",
        }
    }
}
