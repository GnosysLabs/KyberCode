use serde_yaml::Value as Yaml;
use std::path::Path;

/// Must match DSH `WELCOME_NOTICE_VERSION` in ui-settings-models onboarding-copy.
const WELCOME_NOTICE_VERSION: &str = "2026-08-13.1";

pub fn with_welcome_ack(settings_yaml: &str) -> Result<Option<String>, String> {
    let mut document: Yaml = if settings_yaml.trim().is_empty() {
        Yaml::Mapping(serde_yaml::Mapping::new())
    } else {
        serde_yaml::from_str(settings_yaml)
            .map_err(|error| format!("could not parse settings.yaml: {error}"))?
    };
    let root = document
        .as_mapping_mut()
        .ok_or_else(|| "settings.yaml root is not a mapping".to_string())?;
    let onboarding = root
        .entry(Yaml::from("ui-onboarding"))
        .or_insert_with(|| Yaml::Mapping(serde_yaml::Mapping::new()));
    let onboarding = onboarding
        .as_mapping_mut()
        .ok_or_else(|| "ui-onboarding is not a mapping".to_string())?;
    let key = Yaml::from("welcomeNoticeVersion");
    if onboarding.get(&key) == Some(&Yaml::String(WELCOME_NOTICE_VERSION.into())) {
        return Ok(None);
    }
    onboarding.insert(key, Yaml::from(WELCOME_NOTICE_VERSION));
    serde_yaml::to_string(&document)
        .map(Some)
        .map_err(|error| format!("could not rewrite settings.yaml: {error}"))
}

pub fn acknowledge_welcome(settings_path: &Path) -> Result<(), String> {
    let current = std::fs::read_to_string(settings_path).unwrap_or_default();
    let Some(next) = with_welcome_ack(&current)? else {
        return Ok(());
    };
    if let Some(parent) = settings_path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| {
            format!("could not create {}: {error}", parent.display())
        })?;
    }
    std::fs::write(settings_path, next)
        .map_err(|error| format!("could not update {}: {error}", settings_path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_welcome_ack_on_empty_settings() {
        let next = with_welcome_ack("").unwrap().unwrap();
        assert!(next.contains("ui-onboarding:"));
        assert!(next.contains(WELCOME_NOTICE_VERSION));
        assert!(with_welcome_ack(&next).unwrap().is_none());
    }

    #[test]
    fn keeps_other_settings() {
        let next = with_welcome_ack("agent-default-model:\n  provider: openrouter\n")
            .unwrap()
            .unwrap();
        assert!(next.contains("openrouter"));
        assert!(next.contains("welcomeNoticeVersion"));
    }
}
