use crate::model::{Column, ForeignKey, Index, Schema, Table};

#[derive(Debug, Default, Clone)]
pub struct ParseStats {
    pub files: usize,
    pub statements: usize,
    pub create_table: usize,
    pub alter_table: usize,
    pub other: usize,
    pub unparsed: usize,
    pub unparsed_examples: Vec<String>,
}

impl ParseStats {
    fn reject(&mut self, stmt: &str) {
        self.unparsed += 1;
        if self.unparsed_examples.len() < 30 {
            let s: String = stmt.chars().take(160).collect();
            self.unparsed_examples.push(s);
        }
    }
}

pub fn strip_comments(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut quote: Option<char> = None;
    let mut chars = input.chars().peekable();

    while let Some(c) = chars.next() {
        match quote {
            Some(q) => {
                out.push(c);
                if c == '\\' {
                    if let Some(n) = chars.next() {
                        out.push(n);
                    }
                } else if c == q {
                    quote = None;
                }
            }
            None => {
                if c == '\'' || c == '"' || c == '`' {
                    quote = Some(c);
                    out.push(c);
                } else if c == '-' && chars.peek() == Some(&'-') {
                    for n in chars.by_ref() {
                        if n == '\n' {
                            out.push('\n');
                            break;
                        }
                    }
                } else {
                    out.push(c);
                }
            }
        }
    }
    out
}

pub fn split_statements(sql: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quote: Option<char> = None;
    let mut chars = sql.chars().peekable();

    while let Some(c) = chars.next() {
        match quote {
            Some(q) => {
                cur.push(c);
                if c == '\\' {
                    if let Some(n) = chars.next() {
                        cur.push(n);
                    }
                } else if c == q {
                    quote = None;
                }
            }
            None => {
                // Les commentaires : `-- … fin de ligne`, `# … fin de ligne`,
                // `/* … */`. Sans ce traitement, une ligne comme
                // `--Value (Price of the content, for insurance)` se retrouvait
                // collee a l'instruction suivante et MariaDB la refusait.
                // Sur les scripts de Dolibarr 12, cela faisait echouer la moitie
                // des CREATE TABLE (145 tables creees sur 307).
                if c == '-' && chars.peek() == Some(&'-') {
                    // Il faut un blanc apres `--`, sinon c'est un `-` double
                    // dans une expression (a - -b).
                    let mut regard = chars.clone();
                    regard.next();
                    if regard.peek().map(|n| n.is_whitespace()).unwrap_or(true) {
                        chars.next();
                        for n in chars.by_ref() {
                            if n == '\n' {
                                cur.push('\n');
                                break;
                            }
                        }
                        continue;
                    }
                    cur.push(c);
                } else if c == '#' {
                    for n in chars.by_ref() {
                        if n == '\n' {
                            cur.push('\n');
                            break;
                        }
                    }
                } else if c == '/' && chars.peek() == Some(&'*') {
                    chars.next();
                    let mut precedent = ' ';
                    for n in chars.by_ref() {
                        if precedent == '*' && n == '/' {
                            break;
                        }
                        precedent = n;
                    }
                    cur.push(' ');
                } else if c == '\'' || c == '"' || c == '`' {
                    quote = Some(c);
                    cur.push(c);
                } else if c == ';' {
                    let t = cur.trim();
                    if !t.is_empty() {
                        out.push(t.to_string());
                    }
                    cur.clear();
                } else {
                    cur.push(c);
                }
            }
        }
    }

    let t = cur.trim();
    if !t.is_empty() {
        out.push(t.to_string());
    }
    out
}

fn split_top_level(s: &str, sep: char) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut depth = 0i32;
    let mut quote: Option<char> = None;

    for c in s.chars() {
        match quote {
            Some(q) => {
                cur.push(c);
                if c == q {
                    quote = None;
                }
            }
            None => {
                if c == '\'' || c == '"' || c == '`' {
                    quote = Some(c);
                    cur.push(c);
                } else if c == '(' {
                    depth += 1;
                    cur.push(c);
                } else if c == ')' {
                    depth -= 1;
                    cur.push(c);
                } else if c == sep && depth == 0 {
                    let t = cur.trim();
                    if !t.is_empty() {
                        out.push(t.to_string());
                    }
                    cur.clear();
                } else {
                    cur.push(c);
                }
            }
        }
    }

    let t = cur.trim();
    if !t.is_empty() {
        out.push(t.to_string());
    }
    out
}

fn first_token(s: &str) -> String {
    let s = s.trim();
    let end = s
        .find(|c: char| c.is_whitespace() || c == '(')
        .unwrap_or(s.len());
    s[..end].trim().to_lowercase()
}

fn take_ident(s: &str) -> Option<(String, &str)> {
    let s = s.trim_start();
    if s.is_empty() {
        return None;
    }
    if let Some(stripped) = s.strip_prefix('`') {
        let end = stripped.find('`')?;
        return Some((stripped[..end].to_string(), &stripped[end + 1..]));
    }
    let end = s
        .find(|c: char| c.is_whitespace() || c == '(' || c == ',' || c == ')')
        .unwrap_or(s.len());
    if end == 0 {
        return None;
    }
    Some((s[..end].to_string(), &s[end..]))
}

fn column_list(s: &str) -> Vec<String> {
    let start = match s.find('(') {
        Some(i) => i,
        None => return Vec::new(),
    };
    let chars: Vec<char> = s.chars().collect();
    let open = s[..start].chars().count();
    let mut depth = 0i32;
    let mut quote: Option<char> = None;
    let mut out = String::new();
    let mut i = open;
    while i < chars.len() {
        let c = chars[i];
        match quote {
            Some(q) => {
                out.push(c);
                if c == q {
                    quote = None;
                }
            }
            None => {
                if c == '\'' || c == '"' || c == '`' {
                    quote = Some(c);
                    out.push(c);
                } else if c == '(' {
                    depth += 1;
                    if depth > 1 {
                        out.push(c);
                    }
                } else if c == ')' {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                    out.push(c);
                } else {
                    out.push(c);
                }
            }
        }
        i += 1;
    }
    split_top_level(&out, ',')
        .into_iter()
        .map(|c| c.trim().trim_matches('`').to_string())
        .filter(|c| !c.is_empty())
        .collect()
}

fn extract_default(s: &str) -> Option<String> {
    let up = s.to_uppercase();
    let idx = up.find("DEFAULT")?;
    let before_ok = idx == 0
        || !up[..idx]
            .chars()
            .last()
            .map(|c| c.is_alphanumeric() || c == '_')
            .unwrap_or(false);
    if !before_ok {
        return None;
    }
    let rest = s[idx + "DEFAULT".len()..].trim_start();
    if rest.is_empty() {
        return None;
    }
    let first = rest.chars().next().unwrap();
    if first == '\'' || first == '"' {
        let mut out = String::from(first);
        for c in rest[1..].chars() {
            out.push(c);
            if c == first {
                break;
            }
        }
        Some(out)
    } else {
        let end = rest
            .find(|c: char| c.is_whitespace() || c == ',')
            .unwrap_or(rest.len());
        Some(rest[..end].to_string())
    }
}

fn parse_column(item: &str) -> Option<Column> {
    let (name, rest) = take_ident(item)?;
    let rest = rest.trim_start();
    let (base, rest2) = take_ident(rest)?;
    let rest2 = rest2.trim_start();

    let (sql_type, rest3) = if rest2.starts_with('(') {
        let close = rest2.find(')')?;
        (format!("{}{}", base, &rest2[..=close]), &rest2[close + 1..])
    } else {
        (base, rest2)
    };

    let up = rest3.to_uppercase();
    Some(Column {
        name,
        sql_type: sql_type.to_lowercase(),
        nullable: !up.contains("NOT NULL"),
        default: extract_default(rest3),
        auto_increment: up.contains("AUTO_INCREMENT"),
        primary_key: up.contains("PRIMARY KEY"),
        unique: up.contains("UNIQUE"),
    })
}

fn parse_create_table(stmt: &str) -> Option<(String, Table)> {
    let up = stmt.to_uppercase();
    let pos = up.find("CREATE TABLE")?;
    let rest = stmt[pos + "CREATE TABLE".len()..].trim_start();
    let rest = if rest.len() >= 13 && rest[..13].eq_ignore_ascii_case("IF NOT EXISTS") {
        rest[13..].trim_start()
    } else {
        rest
    };

    let (name, rest) = take_ident(rest)?;
    let chars: Vec<char> = rest.chars().collect();
    let open = chars.iter().position(|c| *c == '(')?;

    let mut depth = 0i32;
    let mut quote: Option<char> = None;
    let mut body = String::new();
    let mut i = open;
    while i < chars.len() {
        let c = chars[i];
        match quote {
            Some(q) => {
                if depth >= 1 {
                    body.push(c);
                }
                if c == q {
                    quote = None;
                }
            }
            None => {
                if c == '\'' || c == '"' || c == '`' {
                    quote = Some(c);
                    if depth >= 1 {
                        body.push(c);
                    }
                } else if c == '(' {
                    depth += 1;
                    if depth > 1 {
                        body.push(c);
                    }
                } else if c == ')' {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                    body.push(c);
                } else if depth >= 1 {
                    body.push(c);
                }
            }
        }
        i += 1;
    }

    let mut table = Table::new(&name);
    let mut pending_pk: Vec<String> = Vec::new();

    for item in split_top_level(&body, ',') {
        let up = item.to_uppercase();
        if up.starts_with("PRIMARY KEY") {
            pending_pk.extend(column_list(&item));
            continue;
        }
        if up.starts_with("UNIQUE") {
            let cols = column_list(&item);
            table.indexes.push(Index {
                name: format!("uk_{}_{}", name, cols.join("_")),
                columns: cols,
                unique: true,
            });
            continue;
        }
        if up.starts_with("INDEX ") || up.starts_with("KEY ") || up.starts_with("CONSTRAINT") {
            let cols = column_list(&item);
            let idx_name = up.split_whitespace().nth(1).unwrap_or("idx").to_lowercase();
            table.indexes.push(Index {
                name: idx_name,
                columns: cols,
                unique: false,
            });
            continue;
        }
        if let Some(col) = parse_column(&item) {
            table.columns.push(col);
        }
    }

    for c in table.columns.iter_mut() {
        if pending_pk.iter().any(|p| p.eq_ignore_ascii_case(&c.name)) {
            c.primary_key = true;
        }
    }

    Some((name, table))
}

fn parse_alter(stmt: &str, schema: &mut Schema) -> bool {
    let up = stmt.to_uppercase();
    if !up.starts_with("ALTER TABLE") {
        return false;
    }
    let rest = stmt["ALTER TABLE".len()..].trim_start();
    let (table_name, rest) = match take_ident(rest) {
        Some(v) => v,
        None => return false,
    };
    let rest_up = rest.to_uppercase();

    if rest_up.contains("FOREIGN KEY") && rest_up.contains("CONSTRAINT") {
        let name = rest_up
            .split("CONSTRAINT")
            .nth(1)
            .and_then(|s| s.split_whitespace().next())
            .unwrap_or("fk")
            .to_lowercase();
        let cols = column_list(rest);
        let after_ref = match rest_up.find("REFERENCES") {
            Some(i) => {
                let chars: Vec<char> = rest.chars().collect();
                let idx = rest[..i].chars().count();
                let tail: String = chars[idx + "REFERENCES".len()..].iter().collect();
                tail.trim_start().to_string()
            }
            None => return false,
        };
        let (ref_table, tail) = match take_ident(&after_ref) {
            Some(v) => v,
            None => return false,
        };
        let ref_cols = column_list(tail);
        let t = schema.table_mut(&table_name);
        t.foreign_keys.push(ForeignKey {
            name,
            columns: cols,
            ref_table,
            ref_columns: ref_cols,
        });
        return true;
    }

    if rest_up.contains("ADD UNIQUE INDEX")
        || rest_up.contains("ADD UNIQUE KEY")
        || rest_up.contains("ADD UNIQUE ")
        || (rest_up.contains("ADD CONSTRAINT") && rest_up.contains("UNIQUE"))
    {
        let name = if let Some(i) = rest_up.find("ADD UNIQUE INDEX") {
            first_token(&rest_up[i + "ADD UNIQUE INDEX".len()..])
        } else if let Some(i) = rest_up.find("ADD UNIQUE KEY") {
            first_token(&rest_up[i + "ADD UNIQUE KEY".len()..])
        } else if let Some(i) = rest_up.find("ADD UNIQUE") {
            first_token(&rest_up[i + "ADD UNIQUE".len()..])
        } else if let Some(i) = rest_up.find("CONSTRAINT") {
            first_token(&rest_up[i + "CONSTRAINT".len()..])
        } else {
            String::new()
        };
        let idx_name = if name.is_empty() {
            format!("uk_{}", table_name)
        } else {
            name
        };
        let cols = column_list(rest);
        let t = schema.table_mut(&table_name);
        t.indexes.push(Index {
            name: idx_name,
            columns: cols,
            unique: true,
        });
        return true;
    }

    if rest_up.contains("ADD INDEX") || rest_up.contains("ADD KEY") {
        let after = if let Some(i) = rest_up.find("ADD INDEX") {
            &rest_up[i + "ADD INDEX".len()..]
        } else {
            let i = rest_up.find("ADD KEY").unwrap();
            &rest_up[i + "ADD KEY".len()..]
        };
        let name = after
            .split_whitespace()
            .next()
            .unwrap_or("idx")
            .to_lowercase();
        let cols = column_list(rest);
        let t = schema.table_mut(&table_name);
        t.indexes.push(Index {
            name,
            columns: cols,
            unique: false,
        });
        return true;
    }

    if rest_up.contains("ADD PRIMARY KEY") {
        let cols = column_list(rest);
        let t = schema.table_mut(&table_name);
        for c in t.columns.iter_mut() {
            if cols.iter().any(|p| p.eq_ignore_ascii_case(&c.name)) {
                c.primary_key = true;
            }
        }
        return true;
    }

    false
}

pub fn parse_sql(schema: &mut Schema, sql: &str, stats: &mut ParseStats) {
    let clean = strip_comments(sql);
    for stmt in split_statements(&clean) {
        stats.statements += 1;
        let up = stmt.to_uppercase();
        if up.starts_with("CREATE TABLE") {
            stats.create_table += 1;
            if let Some((name, table)) = parse_create_table(&stmt) {
                schema.tables.insert(name, table);
            } else {
                stats.reject(&stmt);
            }
        } else if up.starts_with("ALTER TABLE") {
            stats.alter_table += 1;
            if !parse_alter(&stmt, schema) {
                stats.reject(&stmt);
            }
        } else {
            stats.other += 1;
        }
    }
}
