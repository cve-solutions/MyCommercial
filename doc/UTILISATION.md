# MyCommercial - Guide d'Utilisation

## Table des matieres

- [Demarrage](#demarrage)
- [Interface generale](#interface-generale)
- [Dashboard](#1-dashboard)
- [Recherche](#2-recherche)
- [Contacts](#3-contacts)
- [Messages](#4-messages)
- [Solutions](#5-solutions)
- [Rapports](#6-rapports)
- [Settings](#7-settings)
- [Workflow complet](#workflow-complet)

---

## Demarrage

```bash
mycommercial
```

Une fenetre native s'ouvre (1280x800, theme sombre) avec 7 onglets.

---

## Interface generale

```mermaid
graph LR
    subgraph TOPBAR["Barre de navigation"]
        D["Dashboard"]
        S["Recherche"]
        C["Contacts"]
        M["Messages"]
        SOL["Solutions"]
        R["Rapports"]
        SET["Settings"]
    end

    subgraph CONTENT["Zone principale"]
        PANEL["Contenu de l'onglet actif"]
    end

    subgraph BOTTOM["Barre de notifications"]
        TOAST["Toasts: succes, erreurs, infos"]
    end

    D --> PANEL
    S --> PANEL
    C --> PANEL
    M --> PANEL
    SOL --> PANEL
    R --> PANEL
    SET --> PANEL

    style TOPBAR fill:#1e293b,stroke:#3b82f6,color:#e2e8f0
    style CONTENT fill:#1e293b,stroke:#64748b,color:#e2e8f0
    style BOTTOM fill:#1e293b,stroke:#94a3b8,color:#e2e8f0
```

### Elements de l'interface

| Element | Description |
|---------|-------------|
| **Barre de navigation** | Onglets cliquables en haut, spinner si chargement async |
| **Zone principale** | Contenu adaptatif selon l'onglet (tableaux, formulaires, graphiques) |
| **Toasts** | Notifications temporaires (4s) en bas : succes (vert), erreur (rouge), info (cyan) |
| **Modales** | Fenetres d'erreur et d'edition centrees avec fond assombri |

### Theme couleurs

| Couleur | Usage |
|---------|-------|
| Bleu (#3b82f6) | Elements principaux, liens, boutons primaires |
| Vert (#22c55e) | Succes, statut "Interesse", connexions actives |
| Jaune (#eab308) | Avertissements, statut "Repondu", KPIs importants |
| Rouge (#ef4444) | Erreurs, statut "KO", deconnecte |
| Cyan (#06b6d4) | Information, statut "Lu", valeurs settings |
| Gris (#94a3b8) | Texte secondaire, elements inactifs |

---

## 1. Dashboard

Vue d'ensemble de l'activite de prospection.

### Elements affiches

```mermaid
graph TD
    subgraph CARDS["Cartes statistiques"]
        C1["Contacts<br/><b>42</b>"]
        C2["Messages envoyes<br/><b>128</b>"]
        C3["Interesses<br/><b>8</b>"]
        C4["Taux reponse<br/><b>12.5%</b>"]
    end

    subgraph FUNNEL["Funnel de Prospection"]
        F1["Envoyes ████████████████ 128"]
        F2["Lus ██████████████ 96"]
        F3["Reponses ██████ 24"]
        F4["Interesses ████ 8"]
        F5["KO ██ 16"]
    end

    subgraph STATUS["Connexions"]
        S1["LinkedIn: Connecte"]
        S2["Ollama: mistral"]
        S3["Odoo: Active"]
    end

    style CARDS fill:#1e293b,stroke:#3b82f6,color:#e2e8f0
    style FUNNEL fill:#1e293b,stroke:#22c55e,color:#e2e8f0
    style STATUS fill:#1e293b,stroke:#eab308,color:#e2e8f0
```

- **4 cartes KPI** : contacts, messages, interesses, taux de reponse
- **Funnel** : barres de progression colorees (envoyes → lus → reponses → OK/KO)
- **Statut connexions** : badges vert/rouge pour LinkedIn, Ollama, Odoo
- **Bouton Rafraichir** : recharge toutes les stats depuis la BDD

---

## 2. Recherche

Recherche d'entreprises et de contacts avec 2 modes.

### Mode Entreprises (API ouverte)

```mermaid
sequenceDiagram
    participant U as Utilisateur
    participant MC as MyCommercial
    participant API as recherche-entreprises.api.gouv.fr

    U->>MC: Tape "cybersecurite" + clic Rechercher
    MC->>API: GET /search?q=cybersecurite
    Note over MC: Spinner + Toast "Chargement..."
    API-->>MC: 25 resultats (total: 342)
    MC->>MC: Cache en SQLite
    MC-->>U: Tableau avec SIREN, Nom, APE, Ville, Effectifs
```

1. Taper un mot-cle dans la barre de recherche
2. Selectionner le mode **Entreprises** (bouton toggle)
3. Cliquer **Rechercher** ou appuyer sur Entree
4. Les resultats s'affichent dans un tableau triable et redimensionnable

**Colonnes :** SIREN | Nom | Code APE | Libelle APE | Ville | Effectifs

### Mode LinkedIn

1. Selectionner le mode **LinkedIn** (bouton toggle)
2. Taper un mot-cle, cliquer Rechercher
3. Les contacts s'affichent avec un bouton **Sauver** par ligne
4. Cliquer **Sauver** enregistre le contact en base

**Colonnes :** Prenom | Nom | Poste | Entreprise | [Sauver]

---

## 3. Contacts

Liste de tous les contacts sauvegardes.

### Actions par contact

```mermaid
flowchart LR
    C[Contact selectionne] -->|Clic "Message IA"| G[Ollama genere un message]
    G --> B[Brouillon sauvegarde]
    B --> MSG[Visible dans onglet Messages]

    C -->|Clic "Suppr"| D[Contact supprime]
```

| Bouton | Action |
|--------|--------|
| **Message IA** | Genere un message de prospection personnalise via Ollama |
| **Suppr** | Supprime le contact et ses messages associes |
| **Rafraichir** | Recharge la liste depuis la BDD |
| **Page precedente / suivante** | Pagination par 100 contacts |

**Colonnes :** Prenom | Nom | Poste | Entreprise | LinkedIn (coche) | Actions

---

## 4. Messages

Suivi de tous les messages de prospection.

### Cycle de statut

```mermaid
stateDiagram-v2
    [*] --> Brouillon : Creation
    Brouillon --> Envoye : Clic "Statut"
    Envoye --> Delivre : Clic "Statut"
    Delivre --> Lu : Clic "Statut"
    Lu --> Repondu : Clic "Statut"
    Repondu --> Interesse : Clic "Statut"
    Interesse --> PasInteresse : Clic "Statut"
    PasInteresse --> SansReponse : Clic "Statut"
    SansReponse --> Brouillon : Clic "Statut"

    Interesse --> OdooCRM : Clic "Odoo"
    PasInteresse --> OdooCRM : Clic "Odoo"

    state OdooCRM {
        [*] --> Lead
        Lead --> Probabilite70 : Interesse
        Lead --> Probabilite0 : KO
    }
```

| Bouton | Action |
|--------|--------|
| **Statut** | Fait cycler le statut au suivant |
| **Odoo** | Cree un lead CRM dans Odoo avec probabilite selon le statut |

**Colonnes :** Contact | Entreprise | Statut (colore) | Date envoi | Apercu message | Actions

### Couleurs de statut

| Statut | Couleur |
|--------|---------|
| Brouillon | Gris |
| Envoye / Delivre | Bleu |
| Lu | Cyan |
| Repondu | Jaune |
| Interesse | Vert |
| Pas interesse | Rouge |
| Sans reponse | Gris fonce |

---

## 5. Solutions

Gestion des documents et solutions commerciales.

### Interface en deux panneaux

```mermaid
graph LR
    subgraph LEFT["Liste des solutions"]
        S1["CyberShield Pro"]
        S2["DataGuard Suite"]
        S3["CloudArmor"]
    end

    subgraph RIGHT["Detail solution"]
        NOM["Nom: CyberShield Pro"]
        DESC["Description: Solution de<br/>cybersecurite complete..."]
        FILE["Fichier: /docs/cybershield.pdf"]
        RESUME["Resume IA: CyberShield Pro offre<br/>une protection zero-trust..."]
        BTN["Bouton: Generer/Regenerer"]
    end

    S1 -->|Selection| RIGHT

    style LEFT fill:#1e293b,stroke:#64748b,color:#e2e8f0
    style RIGHT fill:#1e293b,stroke:#3b82f6,color:#e2e8f0
```

### Ajouter une solution

1. Cliquer **+ Ajouter une solution** (en haut a droite)
2. Remplir le formulaire :
   - **Nom** : Nom commercial de la solution
   - **Description** : Description detaillee
   - **Fichier** : Chemin vers un document (PDF, TXT, MD - optionnel)
3. Cliquer **Sauvegarder**

### Generer un resume IA

```mermaid
sequenceDiagram
    participant U as Utilisateur
    participant MC as MyCommercial
    participant OL as Ollama

    U->>MC: Selectionne une solution + clic "Resume IA"
    MC->>MC: Lit le fichier ou la description
    MC->>OL: POST /api/generate (resume en 2-3 phrases)
    Note over MC: Toast "Resume IA en cours..."
    OL-->>MC: Resume percutant pour decideurs
    MC->>MC: Sauvegarde en SQLite
    MC-->>U: Resume affiche en vert dans le detail
```

1. Selectionner une solution dans la liste
2. Cliquer **Generer / Regenerer** dans le panneau de detail
3. Le resume est genere par Ollama et sauvegarde en BDD
4. Ce resume est utilise pour personnaliser les messages de prospection

---

## 6. Rapports

Statistiques detaillees et funnel de conversion.

### Deux vues cote a cote

| Gauche : Stats detaillees | Droite : Funnel de conversion |
|--------------------------|------------------------------|
| Messages envoyes: 128 | Barre: Envoyes (100%) |
| Messages lus: 96 | Barre: Lus (75%) |
| Reponses recues: 24 | Barre: Reponses (18.8%) |
| Interesses: 8 | Barre: OK (6.3%) |
| Pas interesses: 16 | Barre: KO (12.5%) |
| Sans reponse: 32 | |
| | Envoi -> Reponse : 18.8% |
| | Reponse -> Interet : 33.3% |

### KPIs en haut

4 cartes : Total contacts | Messages envoyes | Taux de reponse | Taux d'interet

---

## 7. Settings

Toutes les configurations en base de donnees, editables depuis l'interface.

### Interface

```mermaid
graph LR
    subgraph SIDEBAR["Categories"]
        LI["linkedin"]
        OL["ollama"]
        OD["odoo"]
        DG["datagouv"]
        PR["prospection"]
        AP["app"]
    end

    subgraph TABLE["Parametres"]
        K1["auth_method | oauth2 | Methode auth"]
        K2["client_id | ******* | OAuth2 ID"]
        K3["daily_limit | 50 | Limite/jour"]
    end

    subgraph ACTIONS["Actions Ollama"]
        T["Tester connexion"]
        A["Auto-selection modele"]
        M["Modeles detectes: mistral, llama3.1"]
    end

    LI -->|Selection| TABLE
    SIDEBAR --> ACTIONS

    style SIDEBAR fill:#1e293b,stroke:#64748b,color:#e2e8f0
    style TABLE fill:#1e293b,stroke:#3b82f6,color:#e2e8f0
    style ACTIONS fill:#1e293b,stroke:#22c55e,color:#e2e8f0
```

### Editer un parametre

1. Selectionner une categorie dans le panneau gauche
2. Cliquer le bouton **crayon** sur la ligne du parametre
3. Une fenetre modale s'ouvre avec la valeur actuelle
4. Modifier la valeur, cliquer **Sauvegarder**

### Actions speciales Ollama (panneau gauche)

| Bouton | Action |
|--------|--------|
| **Tester connexion** | Contacte Ollama et liste les modeles installes |
| **Auto-selection modele** | Choisit le meilleur modele pour la prospection |

### Categories de settings

| Categorie | Parametres principaux |
|-----------|----------------------|
| **linkedin** | auth_method, client_id, client_secret, access_token, cookie_li_at, daily_limit |
| **ollama** | base_url, model, auto_select, temperature, max_tokens, system_prompt |
| **odoo** | enabled, url, database, username, password, pipeline_id |
| **datagouv** | api_token, sirene_api_url, sirene_api_token, cache_duration_hours |
| **prospection** | postes_cibles, tranches_effectifs, message_template |
| **app** | theme, language, log_level |

---

## Workflow complet

```mermaid
flowchart TD
    START(["Lancer MyCommercial"]) --> CONFIG

    subgraph CONFIG ["1 - Configurer (Settings)"]
        C1["Tester connexion Ollama"]
        C2["Configurer LinkedIn"]
        C3["Definir postes cibles et tranches"]
        C1 --> C2 --> C3
    end

    CONFIG --> SOLUTIONS

    subgraph SOLUTIONS ["2 - Preparer les solutions"]
        S1["Clic + Ajouter une solution"]
        S2["Renseigner nom, description, fichier"]
        S3["Clic Resume IA -> Ollama genere"]
        S1 --> S2 --> S3
    end

    SOLUTIONS --> SEARCH

    subgraph SEARCH ["3 - Rechercher des cibles"]
        R1["Mode Entreprises : recherche DataGouv"]
        R2["Identifier les entreprises cibles"]
        R3["Mode LinkedIn : rechercher les decideurs"]
        R4["Clic Sauver sur chaque contact"]
        R1 --> R2 --> R3 --> R4
    end

    SEARCH --> PROSPECT

    subgraph PROSPECT ["4 - Prospecter"]
        P1["Onglet Contacts : selectionner"]
        P2["Clic Message IA -> Ollama genere"]
        P3["Onglet Messages : verifier le brouillon"]
        P4["Clic Statut -> Envoye"]
        P1 --> P2 --> P3 --> P4
    end

    PROSPECT --> FOLLOW

    subgraph FOLLOW ["5 - Suivre et analyser"]
        F1["Mettre a jour les statuts des messages"]
        F2["Clic Odoo pour sync les leads"]
        F3["Onglet Rapports : analyser le funnel"]
        F1 --> F2 --> F3
    end

    FOLLOW --> ITERATE(["Iterer"])
    ITERATE --> SEARCH

    style CONFIG fill:#0f172a,stroke:#3b82f6,color:#e2e8f0
    style SOLUTIONS fill:#0f172a,stroke:#22c55e,color:#e2e8f0
    style SEARCH fill:#0f172a,stroke:#eab308,color:#e2e8f0
    style PROSPECT fill:#0f172a,stroke:#ef4444,color:#e2e8f0
    style FOLLOW fill:#0f172a,stroke:#a855f7,color:#e2e8f0
```

### Resume en 5 etapes

1. **Configurer** : Settings > Ollama + LinkedIn + postes cibles
2. **Preparer** : Ajouter vos solutions + generer les resumes IA
3. **Rechercher** : DataGouv pour les entreprises, LinkedIn pour les contacts
4. **Prospecter** : Generer des messages IA personnalises, marquer comme envoyes
5. **Suivre** : Mettre a jour les statuts, sync Odoo, analyser les rapports

---

## Codes des tranches d'effectifs INSEE

Utilises dans Settings > prospection > `tranches_effectifs` :

| Code | Tranche |
|------|---------|
| 00 | 0 salarie |
| 01 | 1-2 salaries |
| 02 | 3-5 salaries |
| 03 | 6-9 salaries |
| 11 | 10-19 salaries |
| 12 | 20-49 salaries |
| 21 | 50-99 salaries |
| 22 | 100-199 salaries |
| 31 | 200-249 salaries |
| 32 | 250-499 salaries |
| 41 | 500-999 salaries |
| 42 | 1000-1999 salaries |
| 51 | 2000-4999 salaries |
| 52 | 5000-9999 salaries |
| 53 | 10000+ salaries |
