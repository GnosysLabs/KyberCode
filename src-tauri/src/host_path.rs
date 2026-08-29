use std::path::{Path, PathBuf};

const LEGACY_BUNDLE_ID: &str = "com.kyber.app";

pub fn list_separator() -> char {
    if cfg!(windows) {
        ';'
    } else {
        ':'
    }
}

pub fn command_names(name: &str) -> Vec<String> {
    if cfg!(windows) {
        vec![
            format!("{name}.cmd"),
            format!("{name}.exe"),
            format!("{name}.bat"),
            name.to_string(),
        ]
    } else {
        vec![name.to_string()]
    }
}

pub fn extra_dirs() -> Vec<String> {
    let mut dirs = Vec::new();
    if cfg!(windows) {
        if let Ok(program_files) = std::env::var("ProgramFiles") {
            dirs.push(format!(r"{program_files}\nodejs"));
        }
        if let Ok(app_data) = std::env::var("APPDATA") {
            dirs.push(format!(r"{app_data}\npm"));
        }
        if let Ok(local) = std::env::var("LOCALAPPDATA") {
            dirs.push(format!(r"{local}\fnm"));
        }
    } else {
        dirs.push("/opt/homebrew/bin".into());
        dirs.push("/usr/local/bin".into());
    }
    dirs
}

pub fn augmented() -> String {
    let sep = list_separator();
    let current = std::env::var("PATH").unwrap_or_default();
    let mut parts = extra_dirs();
    for part in current.split(sep) {
        if !part.is_empty() && !parts.iter().any(|existing| existing == part) {
            parts.push(part.to_string());
        }
    }
    parts.join(&sep.to_string())
}

pub fn find(name: &str, path: &str) -> Option<PathBuf> {
    let sep = list_separator();
    for dir in path.split(sep) {
        if dir.is_empty() {
            continue;
        }
        for candidate in command_names(name) {
            let full = Path::new(dir).join(candidate);
            if full.is_file() {
                return Some(full);
            }
        }
    }
    None
}

pub fn legacy_dsh_home(home: &Path) -> PathBuf {
    if cfg!(windows) {
        let roaming = std::env::var("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|_| home.join("AppData").join("Roaming"));
        roaming.join(LEGACY_BUNDLE_ID).join("dsh")
    } else if cfg!(target_os = "macos") {
        home.join("Library/Application Support")
            .join(LEGACY_BUNDLE_ID)
            .join("dsh")
    } else {
        home.join(".local/share").join(LEGACY_BUNDLE_ID).join("dsh")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_names_keep_the_bare_binary() {
        assert!(command_names("npx").contains(&"npx".into()));
        assert!(command_names("dsh").contains(&"dsh".into()));
    }

    #[test]
    fn windows_names_include_cmd_wrappers() {
        let names = command_names("npx");
        if cfg!(windows) {
            assert!(names.iter().any(|name| name == "npx.cmd"));
        } else {
            assert_eq!(names, vec!["npx".to_string()]);
        }
    }

    #[test]
    fn macos_legacy_home_uses_the_old_bundle_id() {
        if cfg!(target_os = "macos") {
            let path = legacy_dsh_home(Path::new("/Users/me"));
            assert_eq!(
                path,
                PathBuf::from("/Users/me/Library/Application Support/com.kyber.app/dsh")
            );
        }
    }
}
