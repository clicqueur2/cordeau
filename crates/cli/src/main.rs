use std::path::Path;
use std::process::ExitCode;

const USAGE: &str = "\
dolibarr-rs — noyau Dolibarr en Rust

USAGE:
  dolibarr-rs schema stats    --src <install/mysql/tables>
  dolibarr-rs schema manifest --src <install/mysql/tables> [--out <fichier>] [--version X] [--commit sha]
  dolibarr-rs schema prepare  --src <install/mysql> --out <repertoire> [--entity 1]
  dolibarr-rs schema verify   [--dsn <url>] [--schema dolibarr] [--manifest <fichier>]

  dolibarr-rs party add       --nom <nom> [--zip ..] [--town ..] [--email ..]
  dolibarr-rs party list      [--limit 20]
  dolibarr-rs party show      --id N
  dolibarr-rs party delete    --id N

  dolibarr-rs money line      --qty N --pu N [--remise P] [--tva N] [--base ht|ttc] [--npr]
";

const DEFAULT_DSN: &str = "mysql://root:root@127.0.0.1:3307/dolibarr";

fn flag<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .map(|s| s.as_str())
}

fn has(args: &[String], name: &str) -> bool {
    args.iter().any(|a| a == name)
}

fn dsn_or_env(args: &[String]) -> String {
    flag(args, "--dsn")
        .map(|s| s.to_string())
        .or_else(|| std::env::var("DOLIBARR_DSN").ok())
        .unwrap_or_else(|| DEFAULT_DSN.to_string())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();

    match (
        args.first().map(|s| s.as_str()),
        args.get(1).map(|s| s.as_str()),
    ) {
        (Some("schema"), Some("stats")) => schema_stats(&args),
        (Some("schema"), Some("manifest")) => schema_manifest(&args),
        (Some("schema"), Some("prepare")) => schema_prepare(&args),
        (Some("schema"), Some("verify")) => schema_verify(&args),
        (Some("schema"), Some("planifier")) => schema_planifier(&args),
        (Some("schema"), Some("migrer")) => schema_migrer(&args),
        (Some("audit"), _) => audit_cmd(&args),
        (Some("tache"), Some(cmd)) => tache_cmd(cmd, &args),
        (Some("adresse"), _) => annuaire_adresse(&args),
        (Some("entreprise"), _) => annuaire_entreprise(&args),
        (Some("party"), Some(cmd)) => party_cmd(cmd, &args),
        (Some("product"), Some(cmd)) => product_cmd(cmd, &args),
        (Some("proposal"), Some(cmd)) => proposal_cmd(cmd, &args),
        (Some("invoice"), Some(cmd)) => invoice_cmd(cmd, &args),
        (Some("company"), Some(cmd)) => company_cmd(cmd, &args),
        (Some("payment"), Some(cmd)) => payment_cmd(cmd, &args),
        (Some("money"), Some("line")) => money_line(&args),
        _ => {
            print!("{USAGE}");
            ExitCode::SUCCESS
        }
    }
}

fn schema_stats(args: &[String]) -> ExitCode {
    let src = match flag(args, "--src") {
        Some(v) => v,
        None => {
            eprintln!("--src requis");
            return ExitCode::FAILURE;
        }
    };
    let loaded = match schema::load_tables(Path::new(src)) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("lecture impossible: {e}");
            return ExitCode::FAILURE;
        }
    };
    let s = schema::Stats::from(&loaded);
    println!("fichiers SQL      {}", s.files);
    println!("instructions      {}", s.statements);
    println!("create table      {}", s.create_table);
    println!("alter table       {}", s.alter_table);
    println!("autres            {}", s.other);
    println!("non comprises     {}", s.unparsed);
    println!("tables            {}", s.tables);
    println!("colonnes          {}", s.columns);
    println!("index             {}", s.indexes);
    println!("cles etrangeres   {}", s.foreign_keys);
    ExitCode::SUCCESS
}

fn schema_manifest(args: &[String]) -> ExitCode {
    let src = match flag(args, "--src") {
        Some(v) => v,
        None => {
            eprintln!("--src requis");
            return ExitCode::FAILURE;
        }
    };
    let loaded = match schema::load_tables(Path::new(src)) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("lecture impossible: {e}");
            return ExitCode::FAILURE;
        }
    };
    let version = flag(args, "--version").unwrap_or("24.0.1");
    let commit = flag(args, "--commit").unwrap_or("unknown");
    let manifest = schema::Manifest::build(&loaded, version, commit);
    let json = serde_json::to_string_pretty(&manifest).expect("serialisation");

    match flag(args, "--out") {
        Some(path) => {
            if let Err(e) = std::fs::write(path, &json) {
                eprintln!("ecriture impossible: {e}");
                return ExitCode::FAILURE;
            }
            println!(
                "{} ({} tables, empreinte {})",
                path,
                manifest.tables.len(),
                manifest.fingerprint
            );
        }
        None => println!("{json}"),
    }
    ExitCode::SUCCESS
}

fn schema_prepare(args: &[String]) -> ExitCode {
    let src = match flag(args, "--src") {
        Some(v) => v,
        None => {
            eprintln!("--src requis (repertoire install/mysql)");
            return ExitCode::FAILURE;
        }
    };
    let out = flag(args, "--out").unwrap_or("sql-ready");
    let entity = flag(args, "--entity")
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(1);

    match schema::prepare_dir(Path::new(src), Path::new(out), entity) {
        Ok(p) => {
            println!(
                "{} fichiers, {} instructions -> {}",
                p.files, p.statements, out
            );
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("preparation impossible: {e}");
            ExitCode::FAILURE
        }
    }
}

/// Planifie une montee de version : quels scripts, dans quel ordre.
///
/// Ne touche pas a la base. Sert a repondre a la seule question qui compte avant
/// de migrer une vieille base : **la chaine est-elle complete** ?
/// Joue les scripts de migration, un par un, chacun dans sa transaction.
///
/// Sans `--appliquer`, la commande affiche ce qu'elle ferait et ne touche a
/// rien : une migration de base ne part pas d'une faute de frappe.
fn schema_migrer(args: &[String]) -> ExitCode {
    let dsn = dsn_or_env(args);
    let depuis = flag(args, "--depuis").unwrap_or_default().to_string();
    let vers = flag(args, "--vers").unwrap_or_default().to_string();
    let appliquer = args.iter().any(|a| a == "--appliquer");
    let src = match flag(args, "--src") {
        Some(s) => s.to_string(),
        None => {
            eprintln!("il manque --src <install/mysql/migration>");
            return ExitCode::FAILURE;
        }
    };

    let mut fichiers = Vec::new();
    match std::fs::read_dir(&src) {
        Ok(e) => {
            for f in e.flatten() {
                if let Some(n) = f.file_name().to_str() {
                    if n.ends_with(".sql") {
                        fichiers.push(n.to_string());
                    }
                }
            }
        }
        Err(e) => {
            eprintln!("lecture impossible de {src} : {e}");
            return ExitCode::FAILURE;
        }
    }

    let etapes = match db::migration::chemin(&depuis, &vers, &fichiers) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("chaine incomplete : {e}");
            return ExitCode::FAILURE;
        }
    };

    if !appliquer {
        println!(
            "{} etape(s) de {depuis} a {vers} — rien n'a ete joue.",
            etapes.len()
        );
        for (i, e) in etapes.iter().enumerate() {
            println!("  {:>2}. {}", i + 1, e.fichier);
        }
        println!("\nAjouter --appliquer pour executer.");
        return ExitCode::SUCCESS;
    }

    let pool = match db::pool(&dsn) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("connexion impossible : {e}");
            return ExitCode::FAILURE;
        }
    };

    for (i, etape) in etapes.iter().enumerate() {
        let chemin = std::path::Path::new(&src).join(&etape.fichier);
        let sql = match std::fs::read_to_string(&chemin) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("{}. {} illisible : {e}", i + 1, etape.fichier);
                return ExitCode::FAILURE;
            }
        };
        // `split_statements` decoupe hors guillemets, sans toucher au texte :
        // c'est le decodeur des migrations. Un decoupage naif sur `;` couperait
        // au milieu d'une valeur qui contient un point-virgule.
        let instructions = schema::ddl::split_statements(&sql);
        println!(
            "{}. {} ({} instructions)",
            i + 1,
            etape.fichier,
            instructions.len()
        );

        match db::migration::executer(&pool, &instructions) {
            Ok(ignorees) if ignorees.is_empty() => {}
            Ok(ignorees) => {
                println!(
                    "  {} instruction(s) ignoree(s) : le schema de la base differe de celui du script",
                    ignorees.len()
                );
                for ligne in ignorees.iter().take(3) {
                    println!("    {ligne}");
                }
            }
            Err(e) => {
                eprintln!("  transaction impossible : {e}");
                return ExitCode::FAILURE;
            }
        }
    }

    println!("\nMigration terminee : {depuis} -> {vers}.");
    println!("Verifier : cordeau schema verify --dsn ... --schema <base>");
    ExitCode::SUCCESS
}

fn schema_planifier(args: &[String]) -> ExitCode {
    let depuis = flag(args, "--depuis").unwrap_or_default();
    let vers = flag(args, "--vers").unwrap_or_default();
    let src = match flag(args, "--src") {
        Some(s) => s.to_string(),
        None => {
            eprintln!("il manque --src <install/mysql/migration>");
            return ExitCode::FAILURE;
        }
    };
    if depuis.is_empty() || vers.is_empty() {
        eprintln!("il manque --depuis <version> et --vers <version>");
        return ExitCode::FAILURE;
    }

    let mut fichiers = Vec::new();
    match std::fs::read_dir(&src) {
        Ok(entrees) => {
            for e in entrees.flatten() {
                if let Some(nom) = e.file_name().to_str() {
                    if nom.ends_with(".sql") {
                        fichiers.push(nom.to_string());
                    }
                }
            }
        }
        Err(e) => {
            eprintln!("lecture impossible de {src} : {e}");
            return ExitCode::FAILURE;
        }
    }

    match db::migration::chemin(depuis, vers, &fichiers) {
        Ok(etapes) => {
            println!("{} etape(s) de {depuis} a {vers} :", etapes.len());
            for (i, e) in etapes.iter().enumerate() {
                println!("  {:>2}. {}  ({} -> {})", i + 1, e.fichier, e.de, e.vers);
            }
            println!("\nRien n'a ete execute : c'est un plan.");
            ExitCode::SUCCESS
        }
        Err(e) => {
            println!("chaine incomplete : {e}");
            ExitCode::FAILURE
        }
    }
}

/// Les tables qui appartiennent a un module, et lequel.
///
/// Dolibarr nomme ses fichiers d'installation `<table>-<module>.sql` pour les
/// tables de module, et `<table>.sql` pour celles du coeur. C'est la seule
/// source fiable : le contenu des fichiers ne le dit pas.
fn modules_de_table(dossier: Option<&str>) -> std::collections::BTreeMap<String, String> {
    let mut map = std::collections::BTreeMap::new();
    let Some(dossier) = dossier else {
        return map;
    };
    let Ok(entrees) = std::fs::read_dir(dossier) else {
        return map;
    };
    for e in entrees.flatten() {
        let nom = e.file_name().to_string_lossy().to_string();
        let Some(base) = nom.strip_suffix(".sql") else {
            continue;
        };
        if let Some((table, module)) = base.rsplit_once('-') {
            // Le suffixe doit ressembler a un nom de module, pas a un morceau
            // de date ou de cle (`llx_x-2020`).
            if !module.is_empty() && module.chars().all(|c| c.is_ascii_lowercase() || c == '_') {
                map.insert(table.to_string(), module.to_string());
            }
        }
    }
    map
}

fn schema_verify(args: &[String]) -> ExitCode {
    let dsn = dsn_or_env(args);
    let schema_name = flag(args, "--schema").unwrap_or("dolibarr");
    let manifest_path = flag(args, "--manifest").unwrap_or("schema-24.0.1.json");

    let raw = match std::fs::read_to_string(manifest_path) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("manifeste illisible ({manifest_path}): {e}");
            return ExitCode::FAILURE;
        }
    };
    let manifest: schema::Manifest = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("manifeste invalide: {e}");
            return ExitCode::FAILURE;
        }
    };

    let pool = match db::pool(&dsn) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("connexion impossible: {e}");
            return ExitCode::FAILURE;
        }
    };
    let live = match db::introspect(&pool, schema_name) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("introspection impossible: {e}");
            return ExitCode::FAILURE;
        }
    };

    let rep = db::compare(&manifest, &live);
    println!(
        "manifeste          {} ({} tables)",
        manifest.dolibarr_version, rep.expected_tables
    );
    println!(
        "base {schema_name:<12}  {} tables, {} colonnes",
        rep.live_tables,
        live.len()
    );
    println!("empreinte          {}", manifest.fingerprint);
    println!("tables absentes    {}", rep.missing_tables.len());
    println!("colonnes absentes  {}", rep.missing_columns.len());
    println!("types differents   {}", rep.type_mismatches.len());
    println!("colonnes en plus   {} (modules)", rep.extra_columns.len());
    println!("tables en plus     {} (modules)", rep.module_tables.len());
    println!("tables inconnues   {}", rep.unknown_tables.len());

    // Une table absente n'est pas forcement une faute : Dolibarr ne cree les
    // tables d'un module qu'a son activation. Le nom du fichier d'installation
    // le dit — `llx_quickmemo_memo-quickmemo.sql` porte son module en suffixe,
    // `llx_facture.sql` n'en a pas. Sans ce tri, l'outil criait au loup sur
    // toute base reelle : 14 « absentes » sur la production, toutes des modules
    // non installes.
    let modules = modules_de_table(flag(args, "--modules"));
    let (de_module, du_coeur): (Vec<&String>, Vec<&String>) = rep
        .missing_tables
        .iter()
        .partition(|t| modules.contains_key(*t));
    for m in du_coeur.iter().take(10) {
        println!("  table absente   {m}");
    }
    if !de_module.is_empty() {
        println!(
            "  {} table(s) de module non installe (normal) :",
            de_module.len()
        );
        for m in de_module.iter().take(10) {
            let module = modules.get(*m).map(String::as_str).unwrap_or("?");
            println!("    {m}  (module {module})");
        }
    }
    for m in rep.missing_columns.iter().take(10) {
        println!("  colonne absente {m}");
    }
    for m in rep.type_mismatches.iter().take(10) {
        println!("  type            {m}");
    }
    for m in rep.extra_columns.iter().take(10) {
        println!("  en plus         {m}");
    }
    for t in rep.module_tables.iter().take(10) {
        println!(
            "  table de module {t} ({})",
            db::module_de(t).unwrap_or("?")
        );
    }
    for t in rep.unknown_tables.iter().take(10) {
        println!("  table inconnue  {t}");
    }

    if rep.is_compatible() {
        println!("RESULTAT           compatible");
        ExitCode::SUCCESS
    } else {
        println!("RESULTAT           incompatible");
        ExitCode::FAILURE
    }
}

/// Taches planifiees : appelees par le planificateur du systeme (cron, tache
/// planifiee Windows). Pas de demon dans l'application, pas de cron PHP.
/// Coupe un texte a une longueur, sans casser un caractere.
fn tronquer(texte: &str, max: usize) -> String {
    if texte.chars().count() <= max {
        return texte.to_string();
    }
    let mut out: String = texte.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}

/// Le statut d'une facture, en francais.
///
/// Les valeurs sont celles de `llx_facture.fk_statut` ; un statut inconnu est
/// dit tel quel plutot que traduit en « brouillon » par defaut, ce qui
/// masquerait une donnee inattendue.
fn statut_facture(statut: i16) -> &'static str {
    match statut {
        0 => "brouillon",
        1 => "validee",
        2 => "payee",
        3 => "abandonnee",
        _ => "statut inconnu",
    }
}

/// Audit d'une base Dolibarr, **en lecture seule**.
///
/// Sert a brancher le noyau sur une base qui n'est pas la sienne — une copie de
/// production, une sauvegarde restauree — et a voir ce qu'il y trouve, sans
/// pouvoir rien changer. La garantie n'est pas ici : elle est dans
/// `db::audit::lire`, qui ouvre une transaction `READ ONLY`. MariaDB refuse
/// alors toute ecriture, meme si ce fichier se trompait.
fn audit_cmd(args: &[String]) -> ExitCode {
    let dsn = dsn_or_env(args);
    let pool = match db::pool(&dsn) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("connexion impossible: {e}");
            return ExitCode::FAILURE;
        }
    };

    let rapport = match db::audit::lire(&pool) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("audit impossible: {e}");
            return ExitCode::FAILURE;
        }
    };

    println!("Base   : {}", rapport.base);
    println!(
        "Schema : {} tables sur {}",
        rapport.tables, rapport.tables_attendues
    );
    println!("Comptes: {} utilisateur(s) actif(s)", rapport.utilisateurs);

    println!("\nSocietes :");
    for (id, nom) in &rapport.entites {
        println!("  {id:>4}  {nom}");
    }

    if !rapport.sans_nom.is_empty() {
        println!("\nSociete(s) sans nom renseigne :");
        for id in &rapport.sans_nom {
            println!(
                "  entite {id} — le libelle affiche est un repli. Renseigner MAIN_INFO_SOCIETE_NOM (entite {id})."
            );
        }
        println!("  Sans nom, deux societes se ressemblent dans le selecteur, et l'on facture dans la mauvaise.");
    }

    println!("\nObjets :");
    for c in &rapport.comptes {
        println!("  {:<24} {:>8}", c.libelle, c.lignes);
    }

    // L'apercu : la seule facon de verifier qu'on lit la meme chose que
    // Dolibarr. Un compteur ne prouve rien, un montant se compare.
    let entity = flag(args, "--entity")
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(1);
    match db::audit::apercu_factures(&pool, entity, 5) {
        Ok(factures) if !factures.is_empty() => {
            println!("\nDernieres factures de l'entite {entity} :");
            println!(
                "  {:<14} {:<24} {:<12} {:>12} {:>12}",
                "Reference", "Client", "Date", "HT", "TTC"
            );
            for f in &factures {
                println!(
                    "  {:<14} {:<24} {:<12} {:>12} {:>12}  {}",
                    f.reference,
                    tronquer(&f.client, 24),
                    f.date,
                    f.total_ht,
                    f.total_ttc,
                    statut_facture(f.statut)
                );
            }
            println!("  Comparez ces montants a l'ecran Dolibarr : ils doivent etre identiques.");
        }
        Ok(_) => println!("\nAucune facture dans l'entite {entity}."),
        Err(e) => println!("\nApercu impossible : {e}"),
    }

    if !rapport.schema_complet() {
        println!(
            "\nATTENTION : {} table(s) manquante(s) — ce n'est pas une base Dolibarr complete.",
            rapport.tables_attendues - rapport.tables
        );
        return ExitCode::FAILURE;
    }
    if rapport.vides().is_empty() {
        println!("\nToutes les tables surveillees portent des lignes.");
    } else {
        println!("\nTables vides :");
        for c in rapport.vides() {
            println!("  {} ({})", c.libelle, c.table);
        }
        println!("  Une base de production n'a pas de factures vides : verifier que la copie est complete.");
    }
    println!("\nAucune ecriture n'a ete faite : la transaction etait en lecture seule.");
    ExitCode::SUCCESS
}

fn tache_cmd(cmd: &str, args: &[String]) -> ExitCode {
    let dsn = dsn_or_env(args);
    let entity = flag(args, "--entity")
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(1);
    let pool = match db::pool(&dsn) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("connexion impossible: {e}");
            return ExitCode::FAILURE;
        }
    };

    let resultat: Result<(), Box<dyn std::error::Error>> = (|| match cmd {
        "relances" => {
            let listes = db::relance::en_retard(&pool, entity)?;
            let total = db::relance::total(&listes);
            println!(
                "{} facture(s) en retard, {} au total",
                listes.len(),
                fmt_dec(Some(total))
            );
            for r in &listes {
                println!(
                    "  {:<14} {:<28} {} j  reste {}",
                    r.reference,
                    r.client,
                    r.jours_retard,
                    fmt_dec(Some(r.reste))
                );
            }
            if has(args, "--noter") {
                let mut conn = pool.get_conn()?;
                for r in &listes {
                    db::journal::noter(
                        &mut conn,
                        entity,
                        r.fk_soc,
                        "facture",
                        r.fk_facture,
                        &format!(
                            "Relance à envoyer : {} ({} j de retard)",
                            r.reference, r.jours_retard
                        ),
                    )?;
                }
                println!("{} événement(s) noté(s) dans l'agenda", listes.len());
            }
            Ok(())
        }
        "notifier" => {
            // Les alertes du moment, puis leur envoi. Aucun intermediaire :
            // le courriel passe par le SMTP configure, le webhook par ureq.
            let jour = db::aujourdhui(&pool)?;
            let alertes = db::notifications::alertes(&pool, entity, &jour)?;
            let config_notif = db::notifications::config(&pool, entity)?;

            if alertes.is_empty() {
                println!("Rien a signaler : aucune notification envoyee.");
                return Ok(());
            }
            println!("{} point(s) a traiter", alertes.len());

            let mut envoyes = 0usize;

            // 1. Le courriel.
            if !config_notif.email.trim().is_empty() {
                match app::smtp_config(&pool) {
                    Ok(smtp) => {
                        let corps = db::notifications::resume(&alertes);
                        let message = mail::Message::texte(
                            &smtp.expediteur,
                            &[config_notif.email.trim()],
                            &format!("Cordeau : {} point(s) a traiter", alertes.len()),
                            &corps,
                        );
                        match mail::envoyer(&smtp, &message) {
                            Ok(()) => {
                                envoyes += 1;
                                println!("  courriel envoye a {}", config_notif.email.trim());
                            }
                            Err(e) => println!("  courriel refuse : {e}"),
                        }
                    }
                    Err(motif) => println!("  courriel impossible : {motif}"),
                }
            }

            // 2. Le webhook : on poste du JSON, on ne suppose rien du service
            //    qui le recoit.
            if !config_notif.webhook.trim().is_empty() {
                let charge = db::notifications::charge_webhook(&alertes);
                match ureq::post(config_notif.webhook.trim())
                    .set("Content-Type", "application/json")
                    .timeout(std::time::Duration::from_secs(15))
                    .send_string(&charge)
                {
                    Ok(r) => {
                        envoyes += 1;
                        println!("  webhook : HTTP {}", r.status());
                    }
                    Err(e) => println!("  webhook refuse : {e}"),
                }
            }

            if envoyes == 0 {
                println!("  aucun canal configurable : rien n'est parti.");
                println!(
                    "  Parametres > Notifications, ou {} / {}",
                    db::notifications::CLE_EMAIL,
                    db::notifications::CLE_WEBHOOK
                );
            }
            Ok(())
        }
        "recurrences" => {
            // Le moteur etait ecrit, rien ne le declenchait. Il rend les
            // brouillons crees : ni validation, ni envoi, comme le reste.
            let jour = if let Some(j) = flag(args, "--jour") {
                j.to_string()
            } else {
                db::aujourdhui(&pool)?
            };
            let max: usize = flag(args, "--max")
                .and_then(|v| v.parse().ok())
                .unwrap_or(100);

            let bilan = db::recur_generation::generer_tout(&pool, entity, &jour, max)?;

            if bilan.rien_a_faire() {
                println!("Aucune echeance due au {jour}.");
                return Ok(());
            }
            if !bilan.creees.is_empty() {
                println!("{} brouillon(s) cree(s) au {jour} :", bilan.creees.len());
                for (source, serie, facture) in &bilan.creees {
                    println!("  serie {source} {serie} -> facture {facture}");
                }
                println!("Aucune validation, aucun envoi : les brouillons sont a relire.");
            }
            if bilan.a_des_echecs() {
                // Dire, et sortir en erreur : une serie refusee chaque nuit
                // doit alerter, pas s'enterrer dans un journal.
                eprintln!("{} serie(s) refusee(s) :", bilan.echecs.len());
                for e in &bilan.echecs {
                    eprintln!("  serie {} {} : {}", e.source, e.serie, e.motif);
                }
                return Err(format!(
                    "{} serie(s) due(s) n'ont pas pu etre facturees",
                    bilan.echecs.len()
                )
                .into());
            }
            Ok(())
        }
        "sauvegarde" => {
            let racine =
                std::env::var("DOLIBARR_SAUVEGARDES").unwrap_or_else(|_| "sauvegardes".to_string());
            let s = db::sauvegarde::sauvegarder(&pool, std::path::Path::new(&racine))?;
            println!(
                "sauvegarde {} — {} tables, {} tiers, {} factures, {} documents, {} octets",
                s.dossier.display(),
                s.tables,
                s.tiers,
                s.factures,
                s.fichiers_documents,
                s.taille_sql
            );
            Ok(())
        }
        "purge" => {
            let jours: u64 = flag(args, "--jours")
                .and_then(|v| v.parse().ok())
                .unwrap_or(90);
            let racine =
                std::env::var("DOLIBARR_SAUVEGARDES").unwrap_or_else(|_| "sauvegardes".to_string());
            let mut n = 0;
            if let Ok(entrees) = std::fs::read_dir(&racine) {
                for e in entrees.flatten() {
                    let chemin = e.path();
                    if !chemin.is_dir() {
                        continue;
                    }
                    let ancien = std::fs::metadata(&chemin)
                        .and_then(|m| m.modified())
                        .map(|m| {
                            m.elapsed()
                                .map(|d| d.as_secs() > jours * 86_400)
                                .unwrap_or(false)
                        })
                        .unwrap_or(false);
                    if ancien {
                        std::fs::remove_dir_all(&chemin)?;
                        n += 1;
                    }
                }
            }
            println!("{n} sauvegarde(s) de plus de {jours} jours supprimée(s)");
            Ok(())
        }
        _ => Err(format!("tache inconnue: {cmd} (relances, sauvegarde, purge)").into()),
    })();

    match resultat {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("erreur: {e}");
            ExitCode::FAILURE
        }
    }
}

fn party_cmd(cmd: &str, args: &[String]) -> ExitCode {
    let dsn = dsn_or_env(args);
    let entity = flag(args, "--entity")
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(1);

    let pool = match db::pool(&dsn) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("connexion impossible: {e}");
            return ExitCode::FAILURE;
        }
    };

    match party_run(cmd, args, &pool, entity) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("erreur: {e}");
            ExitCode::FAILURE
        }
    }
}

fn party_run(
    cmd: &str,
    args: &[String],
    pool: &db::Pool,
    entity: i64,
) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        "add" => {
            let nom = flag(args, "--nom").ok_or("--nom requis")?;
            let mut new = db::party::NewParty::client(nom);
            if let Some(v) = flag(args, "--alias") {
                new.name_alias = Some(v.to_string());
            }
            if let Some(v) = flag(args, "--zip") {
                new.zip = Some(v.to_string());
            }
            if let Some(v) = flag(args, "--town") {
                new.town = Some(v.to_string());
            }
            if let Some(v) = flag(args, "--email") {
                new.email = Some(v.to_string());
            }
            if let Some(v) = flag(args, "--phone") {
                new.phone = Some(v.to_string());
            }
            if let Some(v) = flag(args, "--siret") {
                new.siret = Some(v.to_string());
            }
            let id = db::party::create(pool, entity, &new)?;
            if let Some(p) = db::party::get(pool, entity, id)? {
                println!(
                    "cree  rowid={}  code_client={}  {}",
                    p.rowid,
                    p.code_client.unwrap_or_default(),
                    p.nom
                );
            } else {
                println!("cree  rowid={id}");
            }
            Ok(())
        }
        "list" => {
            let limit = flag(args, "--limit")
                .and_then(|v| v.parse().ok())
                .unwrap_or(20);
            let total = db::party::count(pool, entity)?;
            let listes = db::party::list(pool, entity, limit, 0)?;
            println!("{total} tiers dans l'entite {entity}");
            for p in listes {
                println!(
                    "{:>6}  {:<12}  {:<40}  {}",
                    p.rowid,
                    p.code_client.unwrap_or_default(),
                    p.nom,
                    p.town.unwrap_or_default()
                );
            }
            Ok(())
        }
        "show" => {
            let id = flag(args, "--id")
                .and_then(|v| v.parse::<i64>().ok())
                .ok_or("--id requis")?;
            match db::party::get(pool, entity, id)? {
                Some(p) => {
                    println!("rowid          {}", p.rowid);
                    println!("nom            {}", p.nom);
                    println!("code client    {}", p.code_client.unwrap_or_default());
                    println!("client         {}", p.client);
                    println!("fournisseur    {}", p.fournisseur);
                    println!("adresse        {}", p.address.unwrap_or_default());
                    println!("code postal    {}", p.zip.unwrap_or_default());
                    println!("ville          {}", p.town.unwrap_or_default());
                    println!("email          {}", p.email.unwrap_or_default());
                    println!("statut         {}", p.statut);
                    println!("cree le        {}", p.datec.unwrap_or_default());
                }
                None => println!("aucun tiers {id} dans l'entite {entity}"),
            }
            Ok(())
        }
        "delete" => {
            let id = flag(args, "--id")
                .and_then(|v| v.parse::<i64>().ok())
                .ok_or("--id requis")?;
            db::party::delete(pool, entity, id)?;
            println!("supprime rowid={id}");
            Ok(())
        }
        _ => Err(format!("commande inconnue: party {cmd}").into()),
    }
}

fn product_cmd(cmd: &str, args: &[String]) -> ExitCode {
    let dsn = dsn_or_env(args);
    let entity = flag(args, "--entity")
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(1);

    let pool = match db::pool(&dsn) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("connexion impossible: {e}");
            return ExitCode::FAILURE;
        }
    };

    match product_run(cmd, args, &pool, entity) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("erreur: {e}");
            ExitCode::FAILURE
        }
    }
}

fn fmt_dec(d: Option<rust_decimal::Decimal>) -> String {
    d.map(|x| x.normalize().to_string()).unwrap_or_default()
}

fn parse_dec(v: Option<&str>) -> Option<rust_decimal::Decimal> {
    use std::str::FromStr;
    v.and_then(|s| rust_decimal::Decimal::from_str(s.trim()).ok())
}

fn product_run(
    cmd: &str,
    args: &[String],
    pool: &db::Pool,
    entity: i64,
) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        "add" => {
            let reference = flag(args, "--ref").ok_or("--ref requis")?;
            let label = flag(args, "--label").ok_or("--label requis")?;
            let mut new = db::product::NewProduct::new(reference, label);
            new.price = parse_dec(flag(args, "--prix"));
            new.tva_tx = parse_dec(flag(args, "--tva"));
            if let Some(v) = flag(args, "--barcode") {
                new.barcode = Some(v.to_string());
            }
            if has(args, "--service") {
                new.fk_product_type = db::product::TYPE_SERVICE;
            }
            if let Some(code) = flag(args, "--unite") {
                new.fk_unit = db::product::unit_id(pool, code)?;
                if new.fk_unit.is_none() {
                    eprintln!("unite inconnue: {code}");
                }
            }
            let id = db::product::create(pool, entity, &new)?;
            if let Some(p) = db::product::get(pool, entity, id)? {
                println!(
                    "cree  rowid={}  ref={}  {}  prix={}  tva={}",
                    p.rowid,
                    p.ref_,
                    p.label,
                    fmt_dec(p.price),
                    fmt_dec(p.tva_tx)
                );
            } else {
                println!("cree  rowid={id}");
            }
            Ok(())
        }
        "list" => {
            let limit = flag(args, "--limit")
                .and_then(|v| v.parse().ok())
                .unwrap_or(20);
            let total = db::product::count(pool, entity)?;
            println!("{total} produits dans l'entite {entity}");
            for p in db::product::list(pool, entity, limit, 0)? {
                println!(
                    "{:>6}  {:<16}  {:<44}  {:>10}  {}",
                    p.rowid,
                    p.ref_,
                    p.label,
                    fmt_dec(p.price),
                    if p.est_service() {
                        "service"
                    } else {
                        "produit"
                    }
                );
            }
            Ok(())
        }
        "show" => {
            let id = flag(args, "--id")
                .and_then(|v| v.parse::<i64>().ok())
                .ok_or("--id requis")?;
            match db::product::get(pool, entity, id)? {
                Some(p) => {
                    println!("rowid          {}", p.rowid);
                    println!("reference      {}", p.ref_);
                    println!("libelle        {}", p.label);
                    println!(
                        "type           {}",
                        if p.est_service() {
                            "service"
                        } else {
                            "produit"
                        }
                    );
                    println!("prix HT        {}", fmt_dec(p.price));
                    println!("prix TTC       {}", fmt_dec(p.price_ttc));
                    println!("base           {}", p.price_base_type);
                    println!("taux TVA       {}", fmt_dec(p.tva_tx));
                    println!(
                        "unite          {}",
                        p.fk_unit.map(|v| v.to_string()).unwrap_or_default()
                    );
                    println!("archive        {}", p.hidden);
                    println!("cree le        {}", p.datec.unwrap_or_default());
                }
                None => println!("aucun produit {id} dans l'entite {entity}"),
            }
            Ok(())
        }
        "delete" => {
            let id = flag(args, "--id")
                .and_then(|v| v.parse::<i64>().ok())
                .ok_or("--id requis")?;
            db::product::delete(pool, entity, id)?;
            println!("supprime rowid={id}");
            Ok(())
        }
        _ => Err(format!("commande inconnue: product {cmd}").into()),
    }
}

/// Compose les lignes du PDF : sections en bande pleine largeur, sous-total
/// calcule a la fin de chaque section. Les titres de section ne comptent pas
/// dans les totaux.
fn proposal_cmd(cmd: &str, args: &[String]) -> ExitCode {
    let dsn = dsn_or_env(args);
    let entity = flag(args, "--entity")
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(1);

    let pool = match db::pool(&dsn) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("connexion impossible: {e}");
            return ExitCode::FAILURE;
        }
    };

    match proposal_run(cmd, args, &pool, entity) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("erreur: {e}");
            ExitCode::FAILURE
        }
    }
}

/// Une ligne en CLI : `quantite|prix_unitaire_ht|taux_tva|libelle[|remise]`.
fn parse_ligne(s: &str) -> Result<db::proposal::NewProposalLine, Box<dyn std::error::Error>> {
    let parts: Vec<&str> = s.split('|').collect();
    if parts.len() < 4 {
        return Err(
            format!("ligne invalide « {s} » : attendu quantite|pu|tva|libelle[|remise]").into(),
        );
    }
    let qty = parse_dec(Some(parts[0])).ok_or("quantite invalide")?;
    let pu = parse_dec(Some(parts[1])).ok_or("prix unitaire invalide")?;
    let tva = parse_dec(Some(parts[2])).ok_or("taux de TVA invalide")?;
    let remise = match parts.get(4) {
        Some(v) => parse_dec(Some(v)).ok_or("remise invalide")?,
        None => rust_decimal::Decimal::ZERO,
    };
    Ok(db::proposal::NewProposalLine {
        fk_product: None,
        label: parts[3].trim().to_string(),
        description: None,
        qty,
        unit_price: pu,
        tva_tx: tva,
        remise_percent: remise,
        section: None,
    })
}

fn proposal_run(
    cmd: &str,
    args: &[String],
    pool: &db::Pool,
    entity: i64,
) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        // Un devis doit etre valide puis cloture pour devenir facturable :
        // Dolibarr n'ouvre les conversions qu'au statut signe (2).
        "validate" => {
            let id = flag(args, "--id")
                .and_then(|v| v.parse::<i64>().ok())
                .ok_or("--id requis")?;
            db::proposal::validate(pool, entity, id)?;
            println!("valide rowid={id}");
            Ok(())
        }
        "close" => {
            let id = flag(args, "--id")
                .and_then(|v| v.parse::<i64>().ok())
                .ok_or("--id requis")?;
            let statut = flag(args, "--statut")
                .and_then(|v| v.parse::<i16>().ok())
                .unwrap_or(2);
            db::proposal::cloturer(pool, entity, id, statut)?;
            println!("cloture rowid={id} statut={statut}");
            Ok(())
        }
        "add" => {
            let fk_soc = flag(args, "--soc")
                .and_then(|v| v.parse::<i64>().ok())
                .ok_or("--soc requis")?;
            let mut lines = Vec::new();
            let mut section: Option<String> = None;
            let mut i = 0;
            while i < args.len() {
                if args[i] == "--section" {
                    section = args.get(i + 1).cloned();
                    i += 2;
                    continue;
                }
                if args[i] == "--ligne" {
                    if let Some(v) = args.get(i + 1) {
                        let mut ligne = parse_ligne(v)?;
                        ligne.section = section.clone();
                        lines.push(ligne);
                    }
                }
                i += 1;
            }
            if lines.is_empty() {
                return Err("au moins un --ligne est requis".into());
            }
            let new = db::proposal::NewProposal {
                fk_soc,
                note_private: flag(args, "--note").map(|s| s.to_string()),
                note_public: None,
                datep: None,
                fk_cond_reglement: flag(args, "--cond").and_then(|v| v.parse().ok()),
                fin_validite: flag(args, "--validite").map(|s| s.to_string()),
                deposit_percent: flag(args, "--acompte")
                    .and_then(|v| v.replace(',', ".").parse().ok()),
                fk_projet: flag(args, "--projet").and_then(|v| v.parse().ok()),
                lines,
            };
            let id = db::proposal::create(pool, entity, &new)?;
            if let Some(p) = db::proposal::get(pool, entity, id)? {
                println!(
                    "cree  rowid={}  ref={}  total_ht={}  total_ttc={}",
                    p.rowid,
                    p.ref_,
                    fmt_dec(p.total_ht),
                    fmt_dec(p.total_ttc)
                );
            }
            Ok(())
        }
        "list" => {
            let limit = flag(args, "--limit")
                .and_then(|v| v.parse().ok())
                .unwrap_or(20);
            let total = db::proposal::count(pool, entity)?;
            println!("{total} devis dans l'entite {entity}");
            for p in db::proposal::list(pool, entity, limit, 0)? {
                println!(
                    "{:>6}  {:<14}  {:>10}  {:>10}  statut={}",
                    p.rowid,
                    p.ref_,
                    fmt_dec(p.total_ht),
                    fmt_dec(p.total_ttc),
                    p.fk_statut
                );
            }
            Ok(())
        }
        "show" => {
            let id = flag(args, "--id")
                .and_then(|v| v.parse::<i64>().ok())
                .ok_or("--id requis")?;
            let p = db::proposal::get(pool, entity, id)?.ok_or("devis introuvable")?;
            println!("ref            {}", p.ref_);
            println!("tiers          {}", p.fk_soc.unwrap_or(0));
            println!("statut         {}", p.fk_statut);
            println!("total HT       {}", fmt_dec(p.total_ht));
            println!("total TVA      {}", fmt_dec(p.total_tva));
            println!("total TTC      {}", fmt_dec(p.total_ttc));
            for l in db::proposal::lines(pool, entity, id)? {
                println!(
                    "  rang {:>2}  {:<44}  {:>10} x {:>10}  TVA {:>4}  = {:>10}",
                    l.rang,
                    l.label.unwrap_or_default(),
                    fmt_dec(l.qty),
                    fmt_dec(l.subprice),
                    fmt_dec(l.tva_tx),
                    fmt_dec(l.total_ht)
                );
            }
            Ok(())
        }
        "pdf" => {
            let id = flag(args, "--id")
                .and_then(|v| v.parse::<i64>().ok())
                .ok_or("--id requis")?;
            let out = flag(args, "--out").ok_or("--out requis")?;
            let rendu = app::devis_pdf(pool, entity, id)?;
            std::fs::write(out, &rendu.bytes)?;
            println!(
                "{}  {} pages  {} octets",
                out,
                rendu.pages,
                rendu.bytes.len()
            );
            Ok(())
        }
        "delete" => {
            let id = flag(args, "--id")
                .and_then(|v| v.parse::<i64>().ok())
                .ok_or("--id requis")?;
            db::proposal::delete(pool, entity, id)?;
            println!("supprime rowid={id}");
            Ok(())
        }
        _ => Err(format!("commande inconnue: proposal {cmd}").into()),
    }
}

fn invoice_cmd(cmd: &str, args: &[String]) -> ExitCode {
    let dsn = dsn_or_env(args);
    let entity = flag(args, "--entity")
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(1);

    let pool = match db::pool(&dsn) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("connexion impossible: {e}");
            return ExitCode::FAILURE;
        }
    };

    match invoice_run(cmd, args, &pool, entity) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("erreur: {e}");
            ExitCode::FAILURE
        }
    }
}

fn invoice_run(
    cmd: &str,
    args: &[String],
    pool: &db::Pool,
    entity: i64,
) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        "from-proposal" => {
            let devis = flag(args, "--devis")
                .and_then(|v| v.parse::<i64>().ok())
                .ok_or("--devis requis")?;
            let id = db::invoice::create_from_proposal(pool, entity, devis)?;
            if let Some(f) = db::invoice::get(pool, entity, id)? {
                println!(
                    "cree  rowid={}  ref={}  total_ht={}  total_ttc={}",
                    f.rowid,
                    f.ref_,
                    fmt_dec(f.total_ht),
                    fmt_dec(f.total_ttc)
                );
            }
            Ok(())
        }
        "list" => {
            let limit = flag(args, "--limit")
                .and_then(|v| v.parse().ok())
                .unwrap_or(20);
            let total = db::invoice::count(pool, entity)?;
            println!("{total} factures dans l'entite {entity}");
            for f in db::invoice::list(pool, entity, limit, 0)? {
                println!(
                    "{:>6}  {:<14}  {:>10}  statut={} paye={}",
                    f.rowid,
                    f.ref_,
                    fmt_dec(f.total_ttc),
                    f.fk_statut,
                    f.paye
                );
            }
            Ok(())
        }
        "show" => {
            let id = flag(args, "--id")
                .and_then(|v| v.parse::<i64>().ok())
                .ok_or("--id requis")?;
            let f = db::invoice::get(pool, entity, id)?.ok_or("facture introuvable")?;
            println!("ref            {}", f.ref_);
            println!("tiers          {}", f.fk_soc.unwrap_or(0));
            println!("statut         {}", f.fk_statut);
            println!(
                "echeance       {}",
                f.date_lim_reglement.unwrap_or_default()
            );
            println!("total HT       {}", fmt_dec(f.total_ht));
            println!("total TVA      {}", fmt_dec(f.total_tva));
            println!("total TTC      {}", fmt_dec(f.total_ttc));
            let b = db::payment::balance(pool, entity, id)?;
            println!("regle          {}", fmt_dec(Some(b.regle)));
            println!(
                "reste du       {}{}",
                fmt_dec(Some(b.reste)),
                if b.soldee { "  (soldee)" } else { "" }
            );
            for l in db::invoice::lines(pool, entity, id)? {
                println!(
                    "  rang {:>2}  {:<44}  {:>10} x {:>10}  TVA {:>4}  = {:>10}",
                    l.rang,
                    l.label.unwrap_or_default(),
                    fmt_dec(l.qty),
                    fmt_dec(l.subprice),
                    fmt_dec(l.tva_tx),
                    fmt_dec(l.total_ht)
                );
            }
            Ok(())
        }
        "avoir" => {
            let id = flag(args, "--id")
                .and_then(|v| v.parse::<i64>().ok())
                .ok_or("--id requis")?;
            let avoir = db::invoice::create_avoir(pool, entity, id)?;
            if let Some(a) = db::invoice::get(pool, entity, avoir)? {
                println!(
                    "avoir  rowid={}  ref={}  total_ttc={}",
                    a.rowid,
                    a.ref_,
                    fmt_dec(a.total_ttc)
                );
            }
            Ok(())
        }
        "validate" => {
            let id = flag(args, "--id")
                .and_then(|v| v.parse::<i64>().ok())
                .ok_or("--id requis")?;
            db::invoice::validate(pool, entity, id)?;
            println!("facture {id} validee");
            Ok(())
        }
        "pdf" => {
            let id = flag(args, "--id")
                .and_then(|v| v.parse::<i64>().ok())
                .ok_or("--id requis")?;
            let out = flag(args, "--out").ok_or("--out requis")?;
            let rendu = app::facture_pdf(pool, entity, id)?;
            std::fs::write(out, &rendu.bytes)?;
            println!(
                "{}  {} pages  {} octets",
                out,
                rendu.pages,
                rendu.bytes.len()
            );
            Ok(())
        }
        "delete" => {
            let id = flag(args, "--id")
                .and_then(|v| v.parse::<i64>().ok())
                .ok_or("--id requis")?;
            db::invoice::delete(pool, entity, id)?;
            println!("supprime rowid={id}");
            Ok(())
        }
        _ => Err(format!("commande inconnue: invoice {cmd}").into()),
    }
}

fn company_cmd(cmd: &str, args: &[String]) -> ExitCode {
    let dsn = dsn_or_env(args);
    let entity = flag(args, "--entity")
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(1);

    let pool = match db::pool(&dsn) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("connexion impossible: {e}");
            return ExitCode::FAILURE;
        }
    };

    match company_run(cmd, args, &pool, entity) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("erreur: {e}");
            ExitCode::FAILURE
        }
    }
}

fn company_run(
    cmd: &str,
    args: &[String],
    pool: &db::Pool,
    entity: i64,
) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        "show" => {
            let c = db::company::get(pool, entity)?;
            println!("nom            {}", c.nom);
            println!(
                "forme          {}",
                c.forme_juridique.clone().unwrap_or_default()
            );
            println!("capital        {}", c.capital.clone().unwrap_or_default());
            println!("rcs            {}", c.rcs.clone().unwrap_or_default());
            println!("adresse        {}", c.adresse.clone().unwrap_or_default());
            println!(
                "code postal    {}",
                c.code_postal.clone().unwrap_or_default()
            );
            println!("ville          {}", c.ville.clone().unwrap_or_default());
            println!("siret          {}", c.siret.clone().unwrap_or_default());
            println!("tva intracom   {}", c.tva_intra.clone().unwrap_or_default());
            println!("email          {}", c.email.clone().unwrap_or_default());
            println!(
                "regime TVA     {}",
                if c.franchise_en_base {
                    "franchise en base (293 B)"
                } else {
                    "assujetti"
                }
            );
            println!("delai paiement {} jours", c.delai_paiement_jours);
            let manquants = legal::champs_manquants(
                &app::societe_legale(&c),
                &legal::Client {
                    nom: "client".into(),
                    ..Default::default()
                },
            );
            if manquants.is_empty() {
                println!("conformite     complete");
            } else {
                println!("conformite     manque : {}", manquants.join(", "));
            }
            Ok(())
        }
        "set" => {
            let mut c = db::company::get(pool, entity)?;
            if let Some(v) = flag(args, "--nom") {
                c.nom = v.to_string();
            }
            if let Some(v) = flag(args, "--forme") {
                c.forme_juridique = Some(v.to_string());
            }
            if let Some(v) = flag(args, "--capital") {
                c.capital = Some(v.to_string());
            }
            if let Some(v) = flag(args, "--rcs") {
                c.rcs = Some(v.to_string());
            }
            if let Some(v) = flag(args, "--adresse") {
                c.adresse = Some(v.to_string());
            }
            if let Some(v) = flag(args, "--cp") {
                c.code_postal = Some(v.to_string());
            }
            if let Some(v) = flag(args, "--ville") {
                c.ville = Some(v.to_string());
            }
            if let Some(v) = flag(args, "--siret") {
                c.siret = Some(v.to_string());
            }
            if let Some(v) = flag(args, "--siren") {
                c.siren = Some(v.to_string());
            }
            if let Some(v) = flag(args, "--tva-intra") {
                c.tva_intra = Some(v.to_string());
            }
            if let Some(v) = flag(args, "--email") {
                c.email = Some(v.to_string());
            }
            if let Some(v) = flag(args, "--tel") {
                c.telephone = Some(v.to_string());
            }
            if let Some(v) = flag(args, "--franchise") {
                c.franchise_en_base = v == "1" || v.eq_ignore_ascii_case("true");
            }
            if let Some(v) = flag(args, "--assurance-assureur") {
                c.assurance_assureur = Some(v.to_string());
            }
            if let Some(v) = flag(args, "--assurance-police") {
                c.assurance_police = Some(v.to_string());
            }
            if let Some(v) = flag(args, "--assurance-couverture") {
                c.assurance_couverture = Some(v.to_string());
            }
            if let Some(v) = flag(args, "--delai") {
                c.delai_paiement_jours = v.parse().unwrap_or(30);
            }
            db::company::save(pool, &c)?;
            println!("societe enregistree : {}", c.nom);
            Ok(())
        }
        _ => Err(format!("commande inconnue: company {cmd}").into()),
    }
}

fn mode_paiement(s: &str) -> Option<i64> {
    match s.trim().to_ascii_lowercase().as_str() {
        "virement" | "vir" => Some(db::payment::MODE_VIREMENT),
        "cheque" | "chq" => Some(db::payment::MODE_CHEQUE),
        "especes" | "liq" => Some(db::payment::MODE_ESPECES),
        "carte" | "cb" => Some(db::payment::MODE_CARTE),
        "prelevement" | "pre" => Some(db::payment::MODE_PRELEVEMENT),
        _ => None,
    }
}

fn payment_cmd(cmd: &str, args: &[String]) -> ExitCode {
    let dsn = dsn_or_env(args);
    let entity = flag(args, "--entity")
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(1);

    let pool = match db::pool(&dsn) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("connexion impossible: {e}");
            return ExitCode::FAILURE;
        }
    };

    match payment_run(cmd, args, &pool, entity) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("erreur: {e}");
            ExitCode::FAILURE
        }
    }
}

fn payment_run(
    cmd: &str,
    args: &[String],
    pool: &db::Pool,
    entity: i64,
) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        "add" => {
            let fk_facture = flag(args, "--facture")
                .and_then(|v| v.parse::<i64>().ok())
                .ok_or("--facture requis")?;
            let montant = parse_dec(flag(args, "--montant")).ok_or("--montant requis")?;
            let mode = flag(args, "--mode")
                .and_then(mode_paiement)
                .ok_or("--mode requis (virement, cheque, especes, carte, prelevement)")?;
            let new = db::payment::NewPayment {
                fk_facture,
                montant,
                fk_paiement: mode,
                num_paiement: flag(args, "--num").map(|s| s.to_string()),
                note: flag(args, "--note").map(|s| s.to_string()),
                date: flag(args, "--date").map(|s| s.to_string()),
                fk_compte: flag(args, "--compte").and_then(|v| v.parse().ok()),
            };
            let id = db::payment::create(pool, entity, &new)?;
            let b = db::payment::balance(pool, entity, fk_facture)?;
            println!(
                "reglement rowid={}  total TTC={}  regle={}  reste={}{}",
                id,
                fmt_dec(Some(b.total_ttc)),
                fmt_dec(Some(b.regle)),
                fmt_dec(Some(b.reste)),
                if b.soldee { "  FACTURE SOLDEE" } else { "" }
            );
            Ok(())
        }
        "list" => {
            let fk_facture = flag(args, "--facture")
                .and_then(|v| v.parse::<i64>().ok())
                .ok_or("--facture requis")?;
            let b = db::payment::balance(pool, entity, fk_facture)?;
            println!(
                "total TTC {}  regle {}  reste {}{}",
                fmt_dec(Some(b.total_ttc)),
                fmt_dec(Some(b.regle)),
                fmt_dec(Some(b.reste)),
                if b.soldee { "  (soldee)" } else { "" }
            );
            for p in db::payment::list_for_invoice(pool, entity, fk_facture)? {
                println!(
                    "{:>6}  {:<16}  {}  {:>10}  mode={}  {}",
                    p.rowid,
                    p.ref_,
                    p.datep.unwrap_or_default(),
                    fmt_dec(p.amount),
                    p.fk_paiement.unwrap_or(0),
                    p.num_paiement.unwrap_or_default()
                );
            }
            Ok(())
        }
        "delete" => {
            let id = flag(args, "--id")
                .and_then(|v| v.parse::<i64>().ok())
                .ok_or("--id requis")?;
            db::payment::delete(pool, entity, id)?;
            println!("supprime rowid={id}");
            Ok(())
        }
        _ => Err(format!("commande inconnue: payment {cmd}").into()),
    }
}

/// Recherche une adresse dans la Base Adresse Nationale.
fn annuaire_adresse(args: &[String]) -> ExitCode {
    let q = match flag(args, "--q") {
        Some(v) => v,
        None => {
            eprintln!("--q requis");
            return ExitCode::FAILURE;
        }
    };
    match annuaire::adresses(q, 5) {
        Ok(liste) if liste.is_empty() => {
            println!("aucune adresse trouvée");
            ExitCode::SUCCESS
        }
        Ok(liste) => {
            for a in liste {
                println!(
                    "{:<62} {:>5}  INSEE {}",
                    a.libelle,
                    format!("{:.0}%", a.score * 100.0),
                    a.code_insee
                );
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("erreur: {e}");
            ExitCode::FAILURE
        }
    }
}

/// Remplit une fiche à partir d'un SIREN ou d'un SIRET.
fn annuaire_entreprise(args: &[String]) -> ExitCode {
    let siret = match flag(args, "--siret").or_else(|| flag(args, "--siren")) {
        Some(v) => v,
        None => {
            eprintln!("--siret ou --siren requis");
            return ExitCode::FAILURE;
        }
    };
    match annuaire::par_siret(siret) {
        Ok(Some(e)) => {
            println!("dénomination   {}", e.nom);
            println!("SIREN          {}", e.siren);
            println!("SIRET (siège)  {}", e.siret);
            println!("adresse        {}", e.adresse);
            println!("code postal    {}", e.code_postal);
            println!("commune        {}", e.commune);
            ExitCode::SUCCESS
        }
        Ok(None) => {
            println!("aucune entreprise trouvée pour {siret}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("erreur: {e}");
            ExitCode::FAILURE
        }
    }
}

fn money_line(args: &[String]) -> ExitCode {
    let qty = flag(args, "--qty").unwrap_or("1");
    let pu = flag(args, "--pu").unwrap_or("0");
    let remise = flag(args, "--remise").unwrap_or("0");
    let tva = flag(args, "--tva").unwrap_or("20");

    let mut input = money::LineInput::new(qty, pu, remise, tva);
    if flag(args, "--base")
        .map(|b| b.eq_ignore_ascii_case("ttc"))
        .unwrap_or(false)
    {
        input = input.ttc();
    }
    if has(args, "--npr") {
        input = input.not_collected();
    }

    let t = money::line_totals(&input);
    println!("PU HT      {:.2}", t.unit_price_ht);
    println!("brut HT    {:.2}", t.total_ht_before_discount);
    println!("remise     {:.2}", t.discount_amount);
    println!("total HT   {:.2}", t.total_ht);
    println!("TVA        {:.2}", t.total_vat);
    println!("total TTC  {:.2}", t.total_ttc);
    ExitCode::SUCCESS
}
