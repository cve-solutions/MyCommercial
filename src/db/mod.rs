use anyhow::Result;
use rusqlite::{Connection, params};
use std::path::Path;
use std::sync::{Arc, Mutex};

use crate::models::*;

pub type DbPool = Arc<Mutex<Connection>>;

pub fn init_db(db_path: &Path) -> Result<DbPool> {
    let conn = Connection::open(db_path)?;
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
    create_tables(&conn)?;
    migrate_entreprises(&conn)?;
    seed_default_settings(&conn)?;
    Ok(Arc::new(Mutex::new(conn)))
}

fn create_tables(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
        -- Settings dynamiques (clé/valeur avec catégorie)
        CREATE TABLE IF NOT EXISTS settings (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            category TEXT NOT NULL,
            key TEXT NOT NULL,
            value TEXT NOT NULL,
            description TEXT,
            value_type TEXT NOT NULL DEFAULT 'string',
            UNIQUE(category, key)
        );

        -- Solutions / Documents
        CREATE TABLE IF NOT EXISTS solutions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            nom TEXT NOT NULL,
            description TEXT NOT NULL DEFAULT '',
            fichier_path TEXT,
            resume_ia TEXT,
            date_creation DATETIME DEFAULT CURRENT_TIMESTAMP
        );

        -- Entreprises (cache local depuis DataGouv)
        CREATE TABLE IF NOT EXISTS entreprises (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            siren TEXT NOT NULL UNIQUE,
            siret TEXT,
            nom TEXT NOT NULL,
            code_ape TEXT NOT NULL,
            libelle_ape TEXT NOT NULL DEFAULT '',
            tranche_effectifs TEXT,
            categorie_entreprise TEXT,
            adresse TEXT,
            code_postal TEXT,
            ville TEXT,
            nature_juridique TEXT,
            date_creation TEXT,
            nombre_etablissements INTEGER,
            dirigeants TEXT,
            chiffre_affaires REAL,
            resultat_net REAL,
            date_maj DATETIME DEFAULT CURRENT_TIMESTAMP
        );

        -- Contacts LinkedIn
        CREATE TABLE IF NOT EXISTS contacts (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            linkedin_id TEXT,
            prenom TEXT NOT NULL,
            nom TEXT NOT NULL,
            poste TEXT NOT NULL,
            entreprise_siren TEXT,
            entreprise_nom TEXT,
            linkedin_url TEXT,
            email TEXT,
            date_ajout DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (entreprise_siren) REFERENCES entreprises(siren)
        );

        -- Messages de prospection
        CREATE TABLE IF NOT EXISTS messages (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            contact_id INTEGER NOT NULL,
            contenu TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'draft',
            date_envoi DATETIME,
            date_reponse DATETIME,
            solution_id INTEGER,
            odoo_lead_id INTEGER,
            FOREIGN KEY (contact_id) REFERENCES contacts(id),
            FOREIGN KEY (solution_id) REFERENCES solutions(id)
        );

        -- Codes APE favoris (pour filtrage rapide)
        CREATE TABLE IF NOT EXISTS codes_ape (
            code TEXT PRIMARY KEY,
            libelle TEXT NOT NULL,
            actif INTEGER NOT NULL DEFAULT 1
        );

        -- Historique des recherches
        CREATE TABLE IF NOT EXISTS search_history (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            criteria_json TEXT NOT NULL,
            results_count INTEGER DEFAULT 0,
            date_recherche DATETIME DEFAULT CURRENT_TIMESTAMP
        );
        "
    )?;
    Ok(())
}

fn migrate_entreprises(conn: &Connection) -> Result<()> {
    let new_cols = [
        ("nature_juridique", "TEXT"),
        ("date_creation", "TEXT"),
        ("nombre_etablissements", "INTEGER"),
        ("dirigeants", "TEXT"),
        ("chiffre_affaires", "REAL"),
        ("resultat_net", "REAL"),
    ];
    for (col, typ) in &new_cols {
        let _ = conn.execute(&format!("ALTER TABLE entreprises ADD COLUMN {} {}", col, typ), []);
    }

    // Ensure new settings exist in existing databases
    let new_settings = [
        ("linkedin", "login_email", "", "Email de connexion LinkedIn (pour auto-login)", "string"),
        ("linkedin", "login_password", "", "Mot de passe LinkedIn (pour auto-login)", "password"),
        ("app", "solutions_url", "", "URL du site web pour importer les solutions", "string"),
        ("prospection", "signature", "Seriez-vous disponible pour un échange rapide ?\n\nCordialement,\nCyrille VERGER\n0787080801\nGoverbyte", "Signature obligatoire en fin de message", "text"),
    ];
    for (cat, key, val, desc, vtype) in &new_settings {
        conn.execute(
            "INSERT OR IGNORE INTO settings (category, key, value, description, value_type)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![cat, key, val, desc, vtype],
        )?;
    }

    Ok(())
}

fn seed_default_settings(conn: &Connection) -> Result<()> {
    let defaults = vec![
        // LinkedIn
        ("linkedin", "auth_method", "oauth2", "Méthode d'authentification LinkedIn", "select"),
        ("linkedin", "client_id", "", "LinkedIn OAuth2 Client ID", "string"),
        ("linkedin", "client_secret", "", "LinkedIn OAuth2 Client Secret", "password"),
        ("linkedin", "redirect_uri", "http://localhost:8080/callback", "LinkedIn OAuth2 Redirect URI", "string"),
        ("linkedin", "access_token", "", "Token d'accès LinkedIn", "password"),
        ("linkedin", "cookie_li_at", "", "Cookie li_at pour auth par cookie", "password"),
        ("linkedin", "login_email", "", "Email de connexion LinkedIn (pour auto-login)", "string"),
        ("linkedin", "login_password", "", "Mot de passe LinkedIn (pour auto-login)", "password"),
        ("linkedin", "api_key", "", "API Key LinkedIn", "password"),
        ("linkedin", "daily_limit", "50", "Limite quotidienne de messages", "number"),
        ("linkedin", "delay_between_messages_sec", "30", "Délai entre messages (secondes)", "number"),

        // Ollama / IA
        ("ollama", "base_url", "http://localhost:11434", "URL du serveur Ollama", "string"),
        ("ollama", "model", "", "Modèle Ollama sélectionné", "string"),
        ("ollama", "auto_select", "true", "Auto-sélection du meilleur modèle", "bool"),
        ("ollama", "temperature", "0.7", "Température de génération", "number"),
        ("ollama", "max_tokens", "2048", "Nombre max de tokens", "number"),
        ("ollama", "system_prompt", "Tu es un assistant commercial expert. Résume les solutions de manière concise et percutante pour un public de décideurs (CEO, CTO, RSSI).", "Prompt système pour l'IA", "text"),

        // Odoo CRM
        ("odoo", "enabled", "false", "Activer l'intégration Odoo", "bool"),
        ("odoo", "url", "https://mycompany.odoo.com", "URL de l'instance Odoo", "string"),
        ("odoo", "database", "", "Nom de la base Odoo", "string"),
        ("odoo", "username", "", "Nom d'utilisateur Odoo", "string"),
        ("odoo", "password", "", "Mot de passe Odoo", "password"),
        ("odoo", "api_key", "", "Clé API Odoo", "password"),
        ("odoo", "pipeline_id", "", "ID du pipeline CRM", "string"),

        // DataGouv / API Entreprise
        ("datagouv", "api_token", "", "Token API Entreprise (entreprise.api.gouv.fr)", "password"),
        ("datagouv", "sirene_api_url", "https://api.insee.fr/entreprises/sirene/V3.11", "URL API Sirene INSEE", "string"),
        ("datagouv", "sirene_api_token", "", "Token API Sirene INSEE", "password"),
        ("datagouv", "cache_duration_hours", "168", "Durée du cache entreprises (heures)", "number"),

        // Prospection
        ("prospection", "postes_cibles", "CEO,CTO,RSSI,DSI,DG,PDG,Directeur Technique,Directeur Informatique,CISO", "Postes ciblés (séparés par virgule)", "text"),
        ("prospection", "tranches_effectifs", "12,21,22,31,32,41", "Tranches d'effectifs ciblées", "text"),
        ("prospection", "message_template", "Bonjour {prenom},\n\nJe me permets de vous contacter car {solution_resume}\n\nSeriez-vous disponible pour un échange rapide ?\n\nCordialement,\nCyrille VERGER\n0787080801\nGoverbyte", "Template de message par défaut", "text"),
        ("prospection", "signature", "Seriez-vous disponible pour un échange rapide ?\n\nCordialement,\nCyrille VERGER\n0787080801\nGoverbyte", "Signature obligatoire en fin de message", "text"),

        // Application
        ("app", "font_size", "14", "Taille des caractères (10-30)", "number"),
        ("app", "theme", "dark", "Thème de l'interface (dark/light)", "select"),
        ("app", "language", "fr", "Langue de l'application", "select"),
        ("app", "log_level", "info", "Niveau de log", "select"),
        ("app", "db_path", "", "Chemin de la base de données (vide = défaut)", "string"),
        ("app", "solutions_url", "", "URL du site web pour importer les solutions", "string"),
    ];

    for (cat, key, val, desc, vtype) in defaults {
        conn.execute(
            "INSERT OR IGNORE INTO settings (category, key, value, description, value_type)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![cat, key, val, desc, vtype],
        )?;
    }
    Ok(())
}

// ── CRUD Operations ──

pub fn get_setting(db: &DbPool, category: &str, key: &str) -> Result<String> {
    let conn = db.lock().unwrap();
    let value = conn.query_row(
        "SELECT value FROM settings WHERE category = ?1 AND key = ?2",
        params![category, key],
        |row| row.get::<_, String>(0),
    )?;
    Ok(value)
}

pub fn set_setting(db: &DbPool, category: &str, key: &str, value: &str) -> Result<()> {
    let conn = db.lock().unwrap();
    conn.execute(
        "UPDATE settings SET value = ?3 WHERE category = ?1 AND key = ?2",
        params![category, key, value],
    )?;
    Ok(())
}

pub fn get_settings_by_category(db: &DbPool, category: &str) -> Result<Vec<(String, String, String, String)>> {
    let conn = db.lock().unwrap();
    let mut stmt = conn.prepare(
        "SELECT key, value, description, value_type FROM settings WHERE category = ?1 ORDER BY id"
    )?;
    let rows = stmt.query_map(params![category], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
        ))
    })?;
    let mut results = Vec::new();
    for row in rows {
        results.push(row?);
    }
    Ok(results)
}

pub fn get_all_categories(db: &DbPool) -> Result<Vec<String>> {
    let conn = db.lock().unwrap();
    // Ordre logique plutôt qu'alphabétique
    let mut stmt = conn.prepare(
        "SELECT DISTINCT category FROM settings ORDER BY
         CASE category
            WHEN 'linkedin' THEN 1
            WHEN 'ollama' THEN 2
            WHEN 'odoo' THEN 3
            WHEN 'datagouv' THEN 4
            WHEN 'prospection' THEN 5
            WHEN 'app' THEN 6
            ELSE 7
         END"
    )?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
    let mut results = Vec::new();
    for row in rows {
        results.push(row?);
    }
    Ok(results)
}

// ── Contacts ──

pub fn insert_contact(db: &DbPool, contact: &Contact) -> Result<i64> {
    let conn = db.lock().unwrap();
    conn.execute(
        "INSERT INTO contacts (linkedin_id, prenom, nom, poste, entreprise_siren, entreprise_nom, linkedin_url, email)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            contact.linkedin_id, contact.prenom, contact.nom, contact.poste,
            contact.entreprise_siren, contact.entreprise_nom, contact.linkedin_url, contact.email,
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn get_contacts(db: &DbPool, limit: u32, offset: u32) -> Result<Vec<Contact>> {
    let conn = db.lock().unwrap();
    let mut stmt = conn.prepare(
        "SELECT id, linkedin_id, prenom, nom, poste, entreprise_siren, entreprise_nom, linkedin_url, email
         FROM contacts ORDER BY date_ajout DESC LIMIT ?1 OFFSET ?2"
    )?;
    let rows = stmt.query_map(params![limit, offset], |row| {
        Ok(Contact {
            id: Some(row.get(0)?),
            linkedin_id: row.get(1)?,
            prenom: row.get(2)?,
            nom: row.get(3)?,
            poste: row.get(4)?,
            entreprise_siren: row.get(5)?,
            entreprise_nom: row.get(6)?,
            linkedin_url: row.get(7)?,
            email: row.get(8)?,
        })
    })?;
    let mut results = Vec::new();
    for row in rows {
        results.push(row?);
    }
    Ok(results)
}

// ── Messages ──

pub fn insert_message(db: &DbPool, msg: &ProspectionMessage) -> Result<i64> {
    let conn = db.lock().unwrap();
    conn.execute(
        "INSERT INTO messages (contact_id, contenu, status, date_envoi, solution_id)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            msg.contact_id, msg.contenu, msg.status.to_db(),
            msg.date_envoi, msg.solution_id,
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn update_message_status(db: &DbPool, msg_id: i64, status: &MessageStatus) -> Result<()> {
    let conn = db.lock().unwrap();
    conn.execute(
        "UPDATE messages SET status = ?2 WHERE id = ?1",
        params![msg_id, status.to_db()],
    )?;
    Ok(())
}

pub fn get_messages(db: &DbPool, limit: u32, offset: u32) -> Result<Vec<(ProspectionMessage, Contact)>> {
    let conn = db.lock().unwrap();
    let mut stmt = conn.prepare(
        "SELECT m.id, m.contact_id, m.contenu, m.status, m.date_envoi, m.date_reponse,
                m.solution_id, m.odoo_lead_id,
                c.id, c.linkedin_id, c.prenom, c.nom, c.poste, c.entreprise_siren,
                c.entreprise_nom, c.linkedin_url, c.email
         FROM messages m
         JOIN contacts c ON c.id = m.contact_id
         ORDER BY m.date_envoi DESC LIMIT ?1 OFFSET ?2"
    )?;
    let rows = stmt.query_map(params![limit, offset], |row| {
        Ok((
            ProspectionMessage {
                id: Some(row.get(0)?),
                contact_id: row.get(1)?,
                contenu: row.get(2)?,
                status: MessageStatus::from_db(&row.get::<_, String>(3)?),
                date_envoi: row.get(4)?,
                date_reponse: row.get(5)?,
                solution_id: row.get(6)?,
                odoo_lead_id: row.get(7)?,
            },
            Contact {
                id: Some(row.get(8)?),
                linkedin_id: row.get(9)?,
                prenom: row.get(10)?,
                nom: row.get(11)?,
                poste: row.get(12)?,
                entreprise_siren: row.get(13)?,
                entreprise_nom: row.get(14)?,
                linkedin_url: row.get(15)?,
                email: row.get(16)?,
            },
        ))
    })?;
    let mut results = Vec::new();
    for row in rows {
        results.push(row?);
    }
    Ok(results)
}

// ── Entreprises ──

pub fn upsert_entreprise(db: &DbPool, e: &Entreprise) -> Result<()> {
    let conn = db.lock().unwrap();
    conn.execute(
        "INSERT INTO entreprises (siren, siret, nom, code_ape, libelle_ape, tranche_effectifs,
         categorie_entreprise, adresse, code_postal, ville,
         nature_juridique, date_creation, nombre_etablissements, dirigeants, chiffre_affaires, resultat_net)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)
         ON CONFLICT(siren) DO UPDATE SET
            siret=excluded.siret, nom=excluded.nom, code_ape=excluded.code_ape,
            libelle_ape=excluded.libelle_ape, tranche_effectifs=excluded.tranche_effectifs,
            categorie_entreprise=excluded.categorie_entreprise, adresse=excluded.adresse,
            code_postal=excluded.code_postal, ville=excluded.ville,
            nature_juridique=COALESCE(excluded.nature_juridique, entreprises.nature_juridique),
            date_creation=COALESCE(excluded.date_creation, entreprises.date_creation),
            nombre_etablissements=COALESCE(excluded.nombre_etablissements, entreprises.nombre_etablissements),
            dirigeants=COALESCE(excluded.dirigeants, entreprises.dirigeants),
            chiffre_affaires=COALESCE(excluded.chiffre_affaires, entreprises.chiffre_affaires),
            resultat_net=COALESCE(excluded.resultat_net, entreprises.resultat_net),
            date_maj=CURRENT_TIMESTAMP",
        params![
            e.siren, e.siret, e.nom, e.code_ape, e.libelle_ape,
            e.tranche_effectifs, e.categorie_entreprise, e.adresse, e.code_postal, e.ville,
            e.nature_juridique, e.date_creation, e.nombre_etablissements, e.dirigeants,
            e.chiffre_affaires, e.resultat_net,
        ],
    )?;
    Ok(())
}

pub fn search_entreprises(db: &DbPool, codes_ape: &[String], tranches: &[String]) -> Result<Vec<Entreprise>> {
    let conn = db.lock().unwrap();
    let ape_filter = if codes_ape.is_empty() {
        "1=1".to_string()
    } else {
        let placeholders: Vec<String> = codes_ape.iter().map(|c| format!("'{}'", c.replace('\'', "''"))).collect();
        format!("code_ape IN ({})", placeholders.join(","))
    };
    let tranche_filter = if tranches.is_empty() {
        "1=1".to_string()
    } else {
        let placeholders: Vec<String> = tranches.iter().map(|t| format!("'{}'", t.replace('\'', "''"))).collect();
        format!("tranche_effectifs IN ({})", placeholders.join(","))
    };

    let sql = format!(
        "SELECT siren, siret, nom, code_ape, libelle_ape, tranche_effectifs,
                categorie_entreprise, adresse, code_postal, ville,
                nature_juridique, date_creation, nombre_etablissements, dirigeants,
                chiffre_affaires, resultat_net
         FROM entreprises WHERE {} AND {} ORDER BY nom LIMIT 500",
        ape_filter, tranche_filter
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], |row| {
        Ok(Entreprise {
            siren: row.get(0)?,
            siret: row.get(1)?,
            nom: row.get(2)?,
            code_ape: row.get(3)?,
            libelle_ape: row.get(4)?,
            tranche_effectifs: row.get(5)?,
            categorie_entreprise: row.get(6)?,
            adresse: row.get(7)?,
            code_postal: row.get(8)?,
            ville: row.get(9)?,
            nature_juridique: row.get(10)?,
            date_creation: row.get(11)?,
            nombre_etablissements: row.get(12)?,
            dirigeants: row.get(13)?,
            chiffre_affaires: row.get(14)?,
            resultat_net: row.get(15)?,
        })
    })?;
    let mut results = Vec::new();
    for row in rows {
        results.push(row?);
    }
    Ok(results)
}

// ── Solutions ──

pub fn insert_solution(db: &DbPool, sol: &Solution) -> Result<i64> {
    let conn = db.lock().unwrap();
    conn.execute(
        "INSERT INTO solutions (nom, description, fichier_path, resume_ia)
         VALUES (?1, ?2, ?3, ?4)",
        params![sol.nom, sol.description, sol.fichier_path, sol.resume_ia],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn get_solutions(db: &DbPool) -> Result<Vec<Solution>> {
    let conn = db.lock().unwrap();
    let mut stmt = conn.prepare(
        "SELECT id, nom, description, fichier_path, resume_ia, date_creation
         FROM solutions ORDER BY nom"
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(Solution {
            id: Some(row.get(0)?),
            nom: row.get(1)?,
            description: row.get(2)?,
            fichier_path: row.get(3)?,
            resume_ia: row.get(4)?,
            date_creation: row.get(5)?,
        })
    })?;
    let mut results = Vec::new();
    for row in rows {
        results.push(row?);
    }
    Ok(results)
}

pub fn seed_solutions(db: &DbPool) -> Result<()> {
    let existing = get_solutions(db)?;
    if !existing.is_empty() {
        return Ok(());
    }
    let seeds = vec![
        Solution {
            id: None,
            nom: "TakeOver".to_string(),
            description: "Prise en main à distance sécurisée. Transport vidéo optimisé via QUIC/UDP à ~300 Kbps, analyse IA intégrée et sécurité de niveau ANSSI.".to_string(),
            fichier_path: None,
            resume_ia: None,
            date_creation: None,
        },
        Solution {
            id: None,
            nom: "GoverDirectory".to_string(),
            description: "Annuaire d'entreprise souverain et conforme. Gestion centralisée des identités, SSO SAML/OIDC, compatibilité LDAP et Active Directory.".to_string(),
            fichier_path: None,
            resume_ia: None,
            date_creation: None,
        },
        Solution {
            id: None,
            nom: "GoverDirectory-PKI".to_string(),
            description: "Infrastructure à clés publiques souveraine. Émission et gestion des certificats X.509, signature électronique eIDAS et chiffrement S/MIME, intégrés à votre annuaire.".to_string(),
            fichier_path: None,
            resume_ia: None,
            date_creation: None,
        },
        Solution {
            id: None,
            nom: "ProxOps".to_string(),
            description: "Gestion intelligente pour Proxmox VE. Monitoring temps réel, capacity planning prédictif et équilibrage de charge automatique.".to_string(),
            fichier_path: None,
            resume_ia: None,
            date_creation: None,
        },
        Solution {
            id: None,
            nom: "AutoCompose".to_string(),
            description: "Reverse-engineering de conteneurs Docker et Podman. Filtrage des secrets, validation Compose et export YAML/JSON/TOML prêts à versionner.".to_string(),
            fichier_path: None,
            resume_ia: None,
            date_creation: None,
        },
        Solution {
            id: None,
            nom: "OlympusChain".to_string(),
            description: "Blockchain d'entreprise souveraine pour la gestion documentaire sécurisée. Stockage immutable, consensus PoA, chiffrement AES-256 et conformité RGPD/NIS2 native.".to_string(),
            fichier_path: None,
            resume_ia: None,
            date_creation: None,
        },
    ];
    for sol in &seeds {
        insert_solution(db, sol)?;
    }
    Ok(())
}

// ── Solutions update ──

pub fn update_solution_summary(db: &DbPool, sol_id: i64, summary: &str) -> Result<()> {
    let conn = db.lock().unwrap();
    conn.execute(
        "UPDATE solutions SET resume_ia = ?2 WHERE id = ?1",
        params![sol_id, summary],
    )?;
    Ok(())
}

pub fn delete_contact(db: &DbPool, contact_id: i64) -> Result<()> {
    let conn = db.lock().unwrap();
    conn.execute("DELETE FROM messages WHERE contact_id = ?1", params![contact_id])?;
    conn.execute("DELETE FROM contacts WHERE id = ?1", params![contact_id])?;
    Ok(())
}

pub fn update_message_odoo_lead(db: &DbPool, msg_id: i64, lead_id: i64) -> Result<()> {
    let conn = db.lock().unwrap();
    conn.execute(
        "UPDATE messages SET odoo_lead_id = ?2 WHERE id = ?1",
        params![msg_id, lead_id],
    )?;
    Ok(())
}

#[allow(dead_code)]
pub fn get_entreprises(db: &DbPool, limit: u32, offset: u32) -> Result<Vec<Entreprise>> {
    let conn = db.lock().unwrap();
    let mut stmt = conn.prepare(
        "SELECT siren, siret, nom, code_ape, libelle_ape, tranche_effectifs,
                categorie_entreprise, adresse, code_postal, ville,
                nature_juridique, date_creation, nombre_etablissements, dirigeants,
                chiffre_affaires, resultat_net
         FROM entreprises ORDER BY nom LIMIT ?1 OFFSET ?2"
    )?;
    let rows = stmt.query_map(params![limit, offset], |row| {
        Ok(Entreprise {
            siren: row.get(0)?,
            siret: row.get(1)?,
            nom: row.get(2)?,
            code_ape: row.get(3)?,
            libelle_ape: row.get(4)?,
            tranche_effectifs: row.get(5)?,
            categorie_entreprise: row.get(6)?,
            adresse: row.get(7)?,
            code_postal: row.get(8)?,
            ville: row.get(9)?,
            nature_juridique: row.get(10)?,
            date_creation: row.get(11)?,
            nombre_etablissements: row.get(12)?,
            dirigeants: row.get(13)?,
            chiffre_affaires: row.get(14)?,
            resultat_net: row.get(15)?,
        })
    })?;
    let mut results = Vec::new();
    for row in rows {
        results.push(row?);
    }
    Ok(results)
}

pub fn count_messages_today(db: &DbPool) -> Result<u32> {
    let conn = db.lock().unwrap();
    let count: u32 = conn.query_row(
        "SELECT COUNT(*) FROM messages WHERE status != 'draft' AND date_envoi >= date('now')",
        [], |row| row.get(0)
    )?;
    Ok(count)
}

// ── Rapport / Stats ──

pub fn get_rapport_stats(db: &DbPool) -> Result<RapportStats> {
    let conn = db.lock().unwrap();
    let total_contacts: u32 = conn.query_row(
        "SELECT COUNT(*) FROM contacts", [], |row| row.get(0)
    )?;
    let messages_envoyes: u32 = conn.query_row(
        "SELECT COUNT(*) FROM messages WHERE status != 'draft'", [], |row| row.get(0)
    )?;
    let messages_lus: u32 = conn.query_row(
        "SELECT COUNT(*) FROM messages WHERE status IN ('read','replied','interested','not_interested')",
        [], |row| row.get(0)
    )?;
    let reponses: u32 = conn.query_row(
        "SELECT COUNT(*) FROM messages WHERE status IN ('replied','interested','not_interested')",
        [], |row| row.get(0)
    )?;
    let interesses: u32 = conn.query_row(
        "SELECT COUNT(*) FROM messages WHERE status = 'interested'", [], |row| row.get(0)
    )?;
    let pas_interesses: u32 = conn.query_row(
        "SELECT COUNT(*) FROM messages WHERE status = 'not_interested'", [], |row| row.get(0)
    )?;
    let sans_reponse: u32 = conn.query_row(
        "SELECT COUNT(*) FROM messages WHERE status IN ('sent','delivered') AND date_envoi < datetime('now', '-7 days')",
        [], |row| row.get(0)
    )?;

    let taux_reponse = if messages_envoyes > 0 { reponses as f64 / messages_envoyes as f64 * 100.0 } else { 0.0 };
    let taux_interet = if reponses > 0 { interesses as f64 / reponses as f64 * 100.0 } else { 0.0 };

    Ok(RapportStats {
        total_contacts,
        messages_envoyes,
        messages_lus,
        reponses,
        interesses,
        pas_interesses,
        sans_reponse,
        taux_reponse,
        taux_interet,
    })
}
