use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-env-changed=OPENRHIZA_GEMINI_API_KEY");
    println!("cargo:rerun-if-env-changed=GEMINI_API_KEY");
    println!("cargo:rerun-if-changed=.env");

    if let Some(api_key) = load_gemini_api_key() {
        println!("cargo:rustc-env=OPENRHIZA_GEMINI_API_KEY={api_key}");
    }
}

fn load_gemini_api_key() -> Option<String> {
    env::var("OPENRHIZA_GEMINI_API_KEY")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            env::var("GEMINI_API_KEY")
                .ok()
                .filter(|value| !value.trim().is_empty())
        })
        .or_else(load_gemini_api_key_from_dotenv)
}

fn load_gemini_api_key_from_dotenv() -> Option<String> {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").ok()?);
    let dotenv_path = manifest_dir.join(".env");
    let text = fs::read_to_string(dotenv_path).ok()?;

    for raw_line in text.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let Some((name, value)) = line.split_once('=') else {
            continue;
        };

        let key = name.trim();
        if key != "OPENRHIZA_GEMINI_API_KEY" && key != "GEMINI_API_KEY" {
            continue;
        }

        let trimmed = value.trim().trim_matches('"').trim_matches('\'');
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }

    None
}
