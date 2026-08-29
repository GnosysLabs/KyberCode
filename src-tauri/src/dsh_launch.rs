use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::host_path;

const DSH_PACKAGE: &str = "@deepseek-ai/dsh@0.1.1-rc.2";

pub enum DshLaunch {
    Binary(PathBuf),
    Node {
        node: PathBuf,
        script: PathBuf,
        via_npx: bool,
    },
}

pub fn resolve(path: &str) -> Result<DshLaunch, String> {
    if let Ok(explicit) = std::env::var("DSH_BINARY") {
        let binary = PathBuf::from(&explicit);
        if !binary.is_file() {
            return Err(format!("DSH_BINARY is set but is not a file: {explicit}"));
        }
        return Ok(unwrap_or_binary(binary, path));
    }
    if let Some(dsh) = host_path::find("dsh", path) {
        return Ok(unwrap_or_binary(dsh, path));
    }
    if let Some(npx) = host_path::find("npx", path) {
        if let Some(launch) = unwrap_shim(&npx, path, true) {
            return Ok(launch);
        }
        return Ok(DshLaunch::Binary(npx));
    }
    Err(format!(
        "dsh is not installed. Install with:\nnpm install -g {DSH_PACKAGE}"
    ))
}

fn unwrap_or_binary(shim: PathBuf, path: &str) -> DshLaunch {
    unwrap_shim(&shim, path, false).unwrap_or(DshLaunch::Binary(shim))
}

fn unwrap_shim(shim: &Path, path: &str, via_npx: bool) -> Option<DshLaunch> {
    let node = host_path::find_exe("node", path).or_else(|| sibling_node(shim))?;
    let script = host_path::node_cli_for_shim(shim)?;
    Some(DshLaunch::Node {
        node,
        script,
        via_npx,
    })
}

fn sibling_node(shim: &Path) -> Option<PathBuf> {
    let name = if cfg!(windows) { "node.exe" } else { "node" };
    let candidate = shim.parent()?.join(name);
    candidate.is_file().then_some(candidate)
}

pub fn command(launch: &DshLaunch, args: &[&str], dsh_home: &Path, path: &str) -> Command {
    let mut command = match launch {
        DshLaunch::Binary(binary) => {
            let mut command = Command::new(binary);
            if looks_like_npx(binary) {
                command.args(["--yes", DSH_PACKAGE]);
            }
            command.args(args);
            command
        }
        DshLaunch::Node {
            node,
            script,
            via_npx,
        } => {
            let mut command = Command::new(node);
            command.arg(script);
            if *via_npx {
                command.args(["--yes", DSH_PACKAGE]);
            }
            command.args(args);
            command
        }
    };
    host_path::hide_window(&mut command);
    command
        .env("DSH_HOME", dsh_home)
        .env("PATH", path)
        .env("CI", "1")
        .env("NPM_CONFIG_UPDATE_NOTIFIER", "false")
        .stdin(Stdio::null());
    command
}

fn looks_like_npx(binary: &Path) -> bool {
    binary
        .file_stem()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.eq_ignore_ascii_case("npx"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn npx_shim_fallback_still_passes_the_package() {
        let launch = DshLaunch::Binary(PathBuf::from("npx.cmd"));
        let command = command(&launch, &["web"], Path::new("/tmp/dsh"), "/bin");
        let rendered = format!("{command:?}");
        assert!(rendered.contains("--yes"));
        assert!(rendered.contains(DSH_PACKAGE));
        assert!(rendered.contains("web"));
    }

    #[test]
    fn node_npx_cli_does_not_go_through_cmd() {
        let launch = DshLaunch::Node {
            node: PathBuf::from("node.exe"),
            script: PathBuf::from("npx-cli.js"),
            via_npx: true,
        };
        let rendered = format!(
            "{:?}",
            command(&launch, &["web"], Path::new("/tmp/dsh"), "/bin")
        );
        assert!(rendered.contains("node.exe"));
        assert!(rendered.contains("npx-cli.js"));
        assert!(!rendered.contains("npx.cmd"));
    }
}
