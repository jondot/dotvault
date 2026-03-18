use anyhow::{Context, Result};
use dialoguer::{Input, Select};
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
    let name = match name {
        Some(n) => n,
        None => Input::<String>::new()
            .with_prompt("Env var name")
            .interact_text()?,
    };

    let provider = match provider {
        Some(p) => p,
        None => {
            let idx = Select::new()
                .with_prompt("Provider")
                .items(PROVIDERS)
                .default(0)
                .interact()?;
            PROVIDERS[idx].to_string()
        }
    };

    let reference = match reference {
        Some(r) => r,
        None => Input::<String>::new()
            .with_prompt("Reference (path/URI)")
            .interact_text()?,
    };

    let field = if field.is_some() {
        field
    } else if FIELD_REQUIRED.contains(&provider.as_str()) {
        Some(
            Input::<String>::new()
                .with_prompt("Field name (required)")
                .interact_text()?,
        )
    } else if FIELD_OPTIONAL.contains(&provider.as_str()) {
        let input: String = Input::new()
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

    std::fs::write(&filepath, doc.to_string())
        .with_context(|| format!("failed to write {}", filepath.display()))?;

    println!("\u{2713} Added {name} to {filename}");
    Ok(())
}
