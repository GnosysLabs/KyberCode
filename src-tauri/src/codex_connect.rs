use serde::Deserialize;
use serde_json::Value as Json;
use serde_yaml::Value as Yaml;
use std::collections::BTreeMap;
use std::path::Path;
use std::process::Stdio;
use std::thread;
use std::time::{Duration, Instant};

use crate::dsh_launch::{self, DshLaunch};
use crate::host_path;

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

const INSTALL_TIMEOUT: Duration = Duration::from_secs(90);

pub fn ensure(dsh_home: &Path, path: &str, launch: &DshLaunch) -> Result<(), String> {
    let manifest_path = dsh_home.join("profiles/web/package.json");
    let already = std::fs::read_to_string(&manifest_path)
        .ok()
        .is_some_and(|body| is_bundled(&body));
    if !already && !launch.uses_npm() {
        let _ = install(dsh_home, path, launch);
    }
    clear_conflicting_route(&dsh_home.join("settings.yaml"))
}

fn install(dsh_home: &Path, path: &str, launch: &DshLaunch) -> Result<(), String> {
    let mut command = dsh_launch::command(
        launch,
        &["plugin", "--profile", "web", "add", PACKAGE_SPEC],
        dsh_home,
        path,
    );
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command.spawn().map_err(|error| {
        format!(
            "failed to install {PACKAGE_SPEC}: {error}\nCodex Connect needs `dsh` and `pnpm` on PATH."
        )
    })?;
    let started = Instant::now();
    loop {
        if let Ok(Some(status)) = child.try_wait() {
            if status.success() {
                return Ok(());
            }
            return Err(format!(
                "failed to install {PACKAGE_SPEC} ({status})"
            ));
        }
        if started.elapsed() > INSTALL_TIMEOUT {
            host_path::kill_tree(&mut child);
            return Err(format!("timed out installing {PACKAGE_SPEC}"));
        }
        thread::sleep(Duration::from_millis(200));
    }
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

    #[test]
    fn npm_launch_skips_plugin_add() {
        let dir = std::env::temp_dir().join("kyber-codex-skip");
        let _ = std::fs::create_dir_all(&dir);
        let launch = DshLaunch::Binary(std::path::PathBuf::from("npx.cmd"));
        assert!(launch.uses_npm());
        assert!(ensure(&dir, "/bin", &launch).is_ok());
        assert!(!dir.join("profiles/web/package.json").exists());
    }
}
