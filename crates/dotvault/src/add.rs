use anyhow::{Context, Result};
use dialoguer::{theme::ColorfulTheme, Input, Select};
use std::path::Path;

const PROVIDERS: &[&str] = &[
    "1password",
    "aws",
    "hashicorp",
    "gcp",
    "keychain",
    "age",
    "env",
    "keyzero",
];

const FIELD_REQUIRED: &[&str] = &["hashicorp"];
const FIELD_OPTIONAL: &[&str] = &["aws"];

pub fn add_secret(
    dir: &Path,
    local: bool,
    name: Option<String>,
    provider: Option<String>,
    reference: Option<String>,
    field: Option<String>,
) -> Result<()> {
    let theme = ColorfulTheme::default();

    let name = match name {
        Some(n) => n,
        None => Input::<String>::with_theme(&theme)
            .with_prompt("Env var name")
            .interact_text()?,
    };

    let provider = match provider {
        Some(p) => p,
        None => {
            let idx = Select::with_theme(&theme)
                .with_prompt("Provider")
                .items(PROVIDERS)
                .default(0)
                .interact()?;
            PROVIDERS[idx].to_string()
        }
    };

    let reference = match reference {
        Some(r) => r,
        None => Input::<String>::with_theme(&theme)
            .with_prompt("Reference (path/URI)")
            .interact_text()?,
    };

    let field = if field.is_some() {
        field
    } else if FIELD_REQUIRED.contains(&provider.as_str()) {
        Some(
            Input::<String>::with_theme(&theme)
                .with_prompt("Field name (required)")
                .interact_text()?,
        )
    } else if FIELD_OPTIONAL.contains(&provider.as_str()) {
        let input: String = Input::with_theme(&theme)
            .with_prompt("Field name (optional, press enter to skip)")
            .allow_empty(true)
            .interact_text()?;
        if input.is_empty() {
            None
        } else {
            Some(input)
        }
    } else {
        None
    };

    let filename = if local {
        ".dotvault.local.toml"
    } else {
        ".dotvault.toml"
    };
    let filepath = dir.join(filename);

    let contents = if filepath.exists() {
        std::fs::read_to_string(&filepath)
            .with_context(|| format!("failed to read {}", filepath.display()))?
    } else {
        String::new()
    };

    let mut doc = contents
        .parse::<toml_edit::DocumentMut>()
        .with_context(|| format!("failed to parse {}", filepath.display()))?;

    // Ensure [secrets] table exists
    if !doc.contains_key("secrets") {
        doc["secrets"] = toml_edit::Item::Table(toml_edit::Table::new());
    }

    // Build the inline table for the secret entry
    let mut entry = toml_edit::InlineTable::new();
    entry.insert("provider", provider.as_str().into());
    entry.insert("ref", reference.as_str().into());
    if let Some(ref f) = field {
        entry.insert("field", f.as_str().into());
    }

    doc["secrets"][&name] = toml_edit::value(entry);

    if has_environment_sections(&doc) {
        eprintln!(
            "note: this config has environment-specific sections. \
             The secret was added to [secrets] (development only)."
        );
    }

    std::fs::write(&filepath, doc.to_string())
        .with_context(|| format!("failed to write {}", filepath.display()))?;

    println!("\u{2713} Added {name} to {filename}");
    Ok(())
}

fn has_environment_sections(doc: &toml_edit::DocumentMut) -> bool {
    doc.iter()
        .any(|(k, v)| {
            k != "providers"
                && k != "secrets"
                && v.as_table().map_or(false, |t| t.contains_key("secrets"))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detects_environment_sections() {
        let doc: toml_edit::DocumentMut = r#"
[secrets.KEY]
provider = "env"
ref = "VAR"

[production.secrets.KEY]
provider = "env"
ref = "PROD_VAR"
"#
        .parse()
        .unwrap();

        assert!(has_environment_sections(&doc));
    }

    #[test]
    fn test_no_environment_sections() {
        let doc: toml_edit::DocumentMut = r#"
[secrets.KEY]
provider = "env"
ref = "VAR"
"#
        .parse()
        .unwrap();

        assert!(!has_environment_sections(&doc));
    }
}
