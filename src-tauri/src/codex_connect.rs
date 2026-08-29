use serde::Deserialize;
use serde_json::Value as Json;
use serde_yaml::Value as Yaml;
use std::collections::BTreeMap;
use std::path::Path;
use std::process::{Command, Stdio};

use crate::DshLaunch;

pub const PACKAGE_NAME: &str = "dsh-codex-connect";
pub const PACKAGE_SPEC: &str = "dsh-codex-connect@0.1.0-alpha.4.21";

#[derive(Debug, Deserialize)]
struct ProfileManifest {
    #[serde(default)]
    dependencies: BTreeMap<String, Json>,
    #[serde(default)]
    dsh: Option<DshManifest>,
}

#[derive(Debug, Deserialize)]
struct DshManifest {
    #[serde(default)]
    profile: Option<ProfileLayer>,
}

#[derive(Debug, Deserialize)]
struct ProfileLayer {
    #[serde(default)]
    bundles: Vec<String>,
}

pub fn is_bundled(package_json: &str) -> bool {
    let Ok(manifest) = serde_json::from_str::<ProfileManifest>(package_json) else {
        return false;
    };
    let in_deps = manifest.dependencies.contains_key(PACKAGE_NAME);
    let in_bundles = manifest
        .dsh
        .and_then(|section| section.profile)
        .is_some_and(|profile| profile.bundles.iter().any(|name| name == PACKAGE_NAME));
    in_deps && in_bundles
}

pub fn without_pi_ai_openai_codex(settings_yaml: &str) -> Result<Option<String>, String> {
    let mut document: Yaml = serde_yaml::from_str(settings_yaml)
        .map_err(|error| format!("could not parse settings.yaml: {error}"))?;
    let Some(providers) = document
        .get_mut("llm-pi-ai")
        .and_then(Yaml::as_mapping_mut)
        .and_then(|llm| llm.get_mut(Yaml::from("providers")))
        .and_then(Yaml::as_mapping_mut)
    else {
        return Ok(None);
    };
    if providers.remove(Yaml::from("openai-codex")).is_none() {
        return Ok(None);
    }
    serde_yaml::to_string(&document)
        .map(Some)
        .map_err(|error| format!("could not rewrite settings.yaml: {error}"))
}

pub fn ensure(dsh_home: &Path, path: &str, launch: &DshLaunch) -> Result<(), String> {
    let manifest_path = dsh_home.join("profiles/web/package.json");
    let already = std::fs::read_to_string(&manifest_path)
        .ok()
        .is_some_and(|body| is_bundled(&body));
    if !already {
        install(dsh_home, path, launch)?;
    }
    clear_conflicting_route(&dsh_home.join("settings.yaml"))
}

fn install(dsh_home: &Path, path: &str, launch: &DshLaunch) -> Result<(), String> {
    let mut command = match launch {
        DshLaunch::Direct(binary) => {
            let mut command = Command::new(binary);
            command.args(["plugin", "--profile", "web", "add", PACKAGE_SPEC]);
            command
        }
        DshLaunch::Npx(npx) => {
            let mut command = Command::new(npx);
            command.args([
                "--yes",
                "@deepseek-ai/dsh",
                "plugin",
                "--profile",
                "web",
                "add",
                PACKAGE_SPEC,
            ]);
            command
        }
    };
    let output = command
        .env("DSH_HOME", dsh_home)
        .env("PATH", path)
        .stdin(Stdio::null())
        .output()
        .map_err(|error| {
            format!(
                "failed to install {PACKAGE_SPEC}: {error}\nCodex Connect needs `dsh` and `pnpm` on PATH."
            )
        })?;
    if output.status.success() {
        return Ok(());
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    Err(format!(
        "failed to install {PACKAGE_SPEC} ({})\n{stdout}{stderr}",
        output.status
    ))
}

fn clear_conflicting_route(settings_path: &Path) -> Result<(), String> {
    let Ok(current) = std::fs::read_to_string(settings_path) else {
        return Ok(());
    };
    let Some(next) = without_pi_ai_openai_codex(&current)? else {
        return Ok(());
    };
    std::fs::write(settings_path, next)
        .map_err(|error| format!("could not update {}: {error}", settings_path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_requires_dependency_and_profile_layer() {
        let ready = r#"{
          "dependencies": { "dsh-codex-connect": "0.1.0-alpha.4.21" },
          "dsh": { "profile": { "bundles": ["@deepseek-ai/dsh-base", "dsh-codex-connect"] } }
        }"#;
        assert!(is_bundled(ready));
        assert!(!is_bundled(r#"{ "dependencies": { "dsh-codex-connect": "1" } }"#));
        assert!(!is_bundled(
            r#"{ "dsh": { "profile": { "bundles": ["dsh-codex-connect"] } } }"#
        ));
    }

    #[test]
    fn strips_only_the_manual_openai_codex_route() {
        let settings = r#"
llm-pi-ai:
  providers:
    openrouter:
      apiKeyEnv: OPENROUTER_API_KEY
    openai-codex:
      models: [{ id: gpt-5.6-sol }]
agent-default-model:
  provider: openai-codex
  model: gpt-5.6-sol
"#;
        let next = without_pi_ai_openai_codex(settings).unwrap().unwrap();
        assert!(!next.contains("openai-codex:\n"));
        assert!(next.contains("openrouter:"));
        assert!(next.contains("provider: openai-codex"));
        assert!(without_pi_ai_openai_codex(&next).unwrap().is_none());
    }
}
