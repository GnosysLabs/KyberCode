use std::path::{Path, PathBuf};
use std::process::{Child, Command};

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
        if let Ok(pnpm_home) = std::env::var("PNPM_HOME") {
            dirs.push(pnpm_home);
        }
        if let Ok(local) = std::env::var("LOCALAPPDATA") {
            dirs.push(format!(r"{local}\pnpm"));
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
    find_named(&command_names(name), path)
}

pub fn find_exe(name: &str, path: &str) -> Option<PathBuf> {
    let names = if cfg!(windows) {
        vec![format!("{name}.exe"), name.to_string()]
    } else {
        vec![name.to_string()]
    };
    find_named(&names, path)
}

fn find_named(names: &[String], path: &str) -> Option<PathBuf> {
    let sep = list_separator();
    for dir in path.split(sep) {
        if dir.is_empty() {
            continue;
        }
        for name in names {
            let full = Path::new(dir).join(name);
            if full.is_file() {
                return Some(full);
            }
        }
    }
    None
}

/// Hide the console that Windows allocates for `cmd` / `node` children.
pub fn kill_tree(child: &mut Child) {
    let pid = child.id();
    #[cfg(unix)]
    {
        let _ = Command::new("kill")
            .args(["-TERM", &format!("-{pid}")])
            .status();
        let _ = child.wait();
    }
    #[cfg(windows)]
    {
        let mut kill = Command::new("taskkill");
        kill.args(["/F", "/T", "/PID", &pid.to_string()]);
        hide_window(&mut kill);
        let _ = kill.status();
        let _ = child.wait();
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = child.kill();
        let _ = child.wait();
    }
}

pub fn hide_window(#[allow(unused_variables)] command: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
}

/// JS entry next to an npm shim (`npx.cmd`, `dsh.cmd`) so we can spawn `node`
/// instead of a `.cmd` file that opens Windows Terminal.
pub fn node_cli_for_shim(shim: &Path) -> Option<PathBuf> {
    node_cli_candidates(shim)
        .into_iter()
        .find(|candidate| candidate.is_file())
}

pub fn node_cli_candidates(shim: &Path) -> Vec<PathBuf> {
    let Some(dir) = shim.parent() else {
        return Vec::new();
    };
    let stem = shim
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match stem.as_str() {
        "dsh" => vec![dir.join("node_modules/@deepseek-ai/dsh/lib/bin.js")],
        "npx" => vec![
            dir.join("node_modules/npm/bin/npx-cli.js"),
            dir.join("node_modules/npx/bin/npx-cli.js"),
        ],
        "pnpm" => vec![
            dir.join("node_modules/corepack/dist/pnpm.js"),
            dir.join("pnpm.cjs"),
            dir.join("node_modules/pnpm/bin/pnpm.cjs"),
        ],
        _ => Vec::new(),
    }
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

    #[test]
    fn npx_shim_resolves_to_npx_cli_js() {
        let dir = Path::new("/Program Files/nodejs");
        let candidates = node_cli_candidates(&dir.join("npx.cmd"));
        assert!(candidates.iter().any(|path| path.ends_with("node_modules/npm/bin/npx-cli.js")));
    }

    #[test]
    fn pnpm_shim_resolves_to_corepack() {
        let dir = Path::new("/Program Files/nodejs");
        let candidates = node_cli_candidates(&dir.join("pnpm.cmd"));
        assert!(candidates
            .iter()
            .any(|path| path.ends_with("node_modules/corepack/dist/pnpm.js")));
    }

    #[test]
    fn dsh_shim_resolves_to_lib_bin_js() {
        let dir = Path::new("/Users/me/AppData/Roaming/npm");
        let candidates = node_cli_candidates(&dir.join("dsh.cmd"));
        assert!(candidates
            .iter()
            .any(|path| path.ends_with("node_modules/@deepseek-ai/dsh/lib/bin.js")));
    }
}
