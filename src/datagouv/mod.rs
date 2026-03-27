use anyhow::{Result, Context};
use reqwest::Client;
use serde::Deserialize;

use crate::db::{self, DbPool};
use crate::models::Entreprise;
use crate::settings::SettingsManager;

/// Client pour les API DataGouv, Recherche-Entreprises et Entreprise.api.gouv.fr
pub struct DataGouvClient {
    client: Client,
    api_entreprise_token: Option<String>,
    sirene_api_url: String,
    sirene_api_token: Option<String>,
    db: DbPool,
}

// ── API Recherche-Entreprises (OPEN - no token needed) ──

#[derive(Debug, Deserialize)]
struct RechercheEntreprisesResponse {
    results: Option<Vec<RechercheEntreprise>>,
    total_results: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct RechercheEntreprise {
    siren: Option<String>,
    nom_complet: Option<String>,
    nom_raison_sociale: Option<String>,
    siege: Option<RechercheEtablissement>,
    activite_principale: Option<String>,
    tranche_effectif_salarie: Option<String>,
    categorie_entreprise: Option<String>,
    nature_juridique: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RechercheEtablissement {
    siret: Option<String>,
    adresse: Option<String>,
    code_postal: Option<String>,
    libelle_commune: Option<String>,
    activite_principale: Option<String>,
    libelle_activite_principale: Option<String>,
}

// ── API Sirene (INSEE) structures ──

#[derive(Debug, Deserialize)]
struct SireneResponse {
    #[serde(rename = "unitesLegales")]
    unites_legales: Option<Vec<SireneUniteLegale>>,
    #[allow(dead_code)]
    header: Option<SireneHeader>,
}

#[derive(Debug, Deserialize)]
struct SireneHeader {
    #[allow(dead_code)]
    total: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct SireneUniteLegale {
    siren: Option<String>,
    #[serde(rename = "denominationUniteLegale")]
    denomination: Option<String>,
    #[serde(rename = "nomUniteLegale")]
    nom: Option<String>,
    #[serde(rename = "prenomUsuelUniteLegale")]
    prenom: Option<String>,
    #[serde(rename = "activitePrincipaleUniteLegale")]
    activite_principale: Option<String>,
    #[serde(rename = "trancheEffectifsUniteLegale")]
    tranche_effectifs: Option<String>,
    #[serde(rename = "categorieEntreprise")]
    categorie_entreprise: Option<String>,
}

// ── API Entreprise (entreprise.api.gouv.fr) structures ──

#[derive(Debug, Deserialize)]
struct ApiEntrepriseResponse {
    data: Option<ApiEntrepriseData>,
}

#[derive(Debug, Deserialize)]
struct ApiEntrepriseData {
    siren: Option<String>,
    siret_siege_social: Option<String>,
    #[serde(rename = "personne_morale_attributs")]
    personne_morale: Option<PersonneMorale>,
    #[allow(dead_code)]
    forme_juridique: Option<FormeJuridique>,
    activite_principale: Option<ActivitePrincipale>,
    tranche_effectif_salarie: Option<TrancheEffectifSalarie>,
    adresse: Option<Adresse>,
}

#[derive(Debug, Deserialize)]
struct PersonneMorale {
    raison_sociale: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FormeJuridique {
    #[allow(dead_code)]
    code: Option<String>,
    #[allow(dead_code)]
    libelle: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ActivitePrincipale {
    code: Option<String>,
    libelle: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TrancheEffectifSalarie {
    code: Option<String>,
    #[allow(dead_code)]
    intitule: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Adresse {
    acheminement_postal: Option<AcheminementPostal>,
}

#[derive(Debug, Deserialize)]
struct AcheminementPostal {
    #[serde(rename = "l4")]
    ligne4: Option<String>,
    #[serde(rename = "l6")]
    ligne6: Option<String>,
}

impl DataGouvClient {
    pub fn new(settings: &SettingsManager, db: DbPool) -> Self {
        let api_entreprise_token = settings.get("datagouv", "api_token").ok().filter(|s| !s.is_empty());
        let sirene_api_url = settings.get_or_default(
            "datagouv", "sirene_api_url",
            "https://api.insee.fr/entreprises/sirene/V3.11"
        );
        let sirene_api_token = settings.get("datagouv", "sirene_api_token").ok().filter(|s| !s.is_empty());

        Self {
            client: Client::new(),
            api_entreprise_token,
            sirene_api_url,
            sirene_api_token,
            db,
        }
    }

    /// Recherche d'entreprises via l'API ouverte recherche-entreprises.api.gouv.fr
    /// Pas besoin de token ! Rate limit: 7 req/sec
    pub async fn search_open(
        &self,
        query: &str,
        code_ape: Option<&str>,
        tranche_effectifs: Option<&str>,
        departement: Option<&str>,
        page: u32,
        per_page: u32,
    ) -> Result<(Vec<Entreprise>, u32)> {
        let mut url = format!(
            "https://recherche-entreprises.api.gouv.fr/search?q={}&page={}&per_page={}&etat_administratif=A",
            urlencoding::encode(query),
            page,
            per_page,
        );

        if let Some(ape) = code_ape {
            url.push_str(&format!("&activite_principale={}", urlencoding::encode(ape)));
        }
        if let Some(tranche) = tranche_effectifs {
            url.push_str(&format!("&tranche_effectif_salarie={}", urlencoding::encode(tranche)));
        }
        if let Some(dep) = departement {
            url.push_str(&format!("&departement={}", urlencoding::encode(dep)));
        }

        let resp = self.client
            .get(&url)
            .header("Accept", "application/json")
            .send()
            .await
            .context("Erreur de connexion à recherche-entreprises.api.gouv.fr")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("API Recherche Entreprises error {}: {}", status, body);
        }

        let data: RechercheEntreprisesResponse = resp.json().await
            .context("Erreur de parsing recherche-entreprises")?;

        let total = data.total_results.unwrap_or(0);

        let entreprises: Vec<Entreprise> = data.results.unwrap_or_default().into_iter().map(|r| {
            let siege = r.siege.as_ref();
            let nom = r.nom_complet
                .or(r.nom_raison_sociale)
                .unwrap_or_else(|| "Inconnu".to_string());

            let code_ape = siege
                .and_then(|s| s.activite_principale.clone())
                .or(r.activite_principale)
                .unwrap_or_default();

            let libelle_ape = siege
                .and_then(|s| s.libelle_activite_principale.clone())
                .unwrap_or_default();

            Entreprise {
                siren: r.siren.unwrap_or_default(),
                siret: siege.and_then(|s| s.siret.clone()),
                nom,
                code_ape,
                libelle_ape,
                tranche_effectifs: r.tranche_effectif_salarie,
                categorie_entreprise: r.categorie_entreprise,
                adresse: siege.and_then(|s| s.adresse.clone()),
                code_postal: siege.and_then(|s| s.code_postal.clone()),
                ville: siege.and_then(|s| s.libelle_commune.clone()),
            }
        }).collect();

        // Cache en BDD
        for e in &entreprises {
            let _ = db::upsert_entreprise(&self.db, e);
        }

        Ok((entreprises, total))
    }

    /// Recherche d'entreprises via l'API Sirene INSEE par code APE et tranche d'effectifs
    pub async fn search_sirene(
        &self,
        codes_ape: &[String],
        tranches_effectifs: &[String],
        nombre: u32,
    ) -> Result<Vec<Entreprise>> {
        let token = self.sirene_api_token.as_ref()
            .context("Token API Sirene non configuré. Configurez-le dans Settings > DataGouv.")?;

        let mut filters = Vec::new();
        if !codes_ape.is_empty() {
            let ape_filter: Vec<String> = codes_ape.iter()
                .map(|c| format!("activitePrincipaleUniteLegale:\"{}\"", c))
                .collect();
            filters.push(format!("({})", ape_filter.join(" OR ")));
        }
        if !tranches_effectifs.is_empty() {
            let tranche_filter: Vec<String> = tranches_effectifs.iter()
                .map(|t| format!("trancheEffectifsUniteLegale:\"{}\"", t))
                .collect();
            filters.push(format!("({})", tranche_filter.join(" OR ")));
        }
        filters.push("etatAdministratifUniteLegale:\"A\"".to_string());
        let q = filters.join(" AND ");

        let url = format!(
            "{}/unites_legales?q={}&nombre={}",
            self.sirene_api_url,
            urlencoding::encode(&q),
            nombre
        );

        let resp = self.client
            .get(&url)
            .header("Authorization", format!("Bearer {}", token))
            .header("Accept", "application/json")
            .send()
            .await
            .context("Erreur de connexion à l'API Sirene")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("API Sirene error {}: {}", status, body);
        }

        let data: SireneResponse = resp.json().await
            .context("Erreur de parsing de la réponse Sirene")?;

        let entreprises: Vec<Entreprise> = data.unites_legales.unwrap_or_default().into_iter().map(|ul| {
            let nom = ul.denomination.clone()
                .or_else(|| {
                    match (&ul.prenom, &ul.nom) {
                        (Some(p), Some(n)) => Some(format!("{} {}", p, n)),
                        (None, Some(n)) => Some(n.clone()),
                        _ => ul.denomination.clone(),
                    }
                })
                .unwrap_or_else(|| "Inconnu".to_string());

            Entreprise {
                siren: ul.siren.unwrap_or_default(),
                siret: None,
                nom,
                code_ape: ul.activite_principale.unwrap_or_default(),
                libelle_ape: String::new(),
                tranche_effectifs: ul.tranche_effectifs,
                categorie_entreprise: ul.categorie_entreprise,
                adresse: None,
                code_postal: None,
                ville: None,
            }
        }).collect();

        for e in &entreprises {
            let _ = db::upsert_entreprise(&self.db, e);
        }

        Ok(entreprises)
    }

    /// Récupère les détails d'une entreprise via entreprise.api.gouv.fr
    pub async fn get_entreprise_details(&self, siren: &str) -> Result<Entreprise> {
        let token = self.api_entreprise_token.as_ref()
            .context("Token API Entreprise non configuré. Configurez-le dans Settings > DataGouv.")?;

        let url = format!(
            "https://entreprise.api.gouv.fr/v3/insee/sirene/unites_legales/{}",
            siren
        );

        let resp = self.client
            .get(&url)
            .header("Authorization", format!("Bearer {}", token))
            .header("Accept", "application/json")
            .send()
            .await
            .context("Erreur de connexion à API Entreprise")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("API Entreprise error {}: {}", status, body);
        }

        let data: ApiEntrepriseResponse = resp.json().await
            .context("Erreur de parsing réponse API Entreprise")?;

        let d = data.data.context("Pas de données dans la réponse")?;
        let nom = d.personne_morale
            .and_then(|pm| pm.raison_sociale)
            .unwrap_or_else(|| "Inconnu".to_string());

        let (code_ape, libelle_ape) = d.activite_principale
            .map(|a| (a.code.unwrap_or_default(), a.libelle.unwrap_or_default()))
            .unwrap_or_default();

        let tranche = d.tranche_effectif_salarie.and_then(|t| t.code);

        let (adresse, code_postal, ville) = d.adresse
            .and_then(|a| a.acheminement_postal)
            .map(|ap| {
                let cp_ville = ap.ligne6.unwrap_or_default();
                let parts: Vec<&str> = cp_ville.splitn(2, ' ').collect();
                let cp = parts.first().map(|s| s.to_string());
                let v = parts.get(1).map(|s| s.to_string());
                (ap.ligne4, cp, v)
            })
            .unwrap_or((None, None, None));

        let entreprise = Entreprise {
            siren: d.siren.unwrap_or_else(|| siren.to_string()),
            siret: d.siret_siege_social,
            nom,
            code_ape,
            libelle_ape,
            tranche_effectifs: tranche,
            categorie_entreprise: None,
            adresse,
            code_postal,
            ville,
        };

        db::upsert_entreprise(&self.db, &entreprise)?;
        Ok(entreprise)
    }

    /// Recherche dans le cache local (BDD)
    pub fn search_local(
        &self,
        codes_ape: &[String],
        tranches: &[String],
    ) -> Result<Vec<Entreprise>> {
        db::search_entreprises(&self.db, codes_ape, tranches)
    }
}
