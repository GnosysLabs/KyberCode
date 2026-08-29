use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::host_path;

const DSH_PACKAGE: &str = "@deepseek-ai/dsh@0.1.1-rc.2";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Runner {
    Direct,
    Npx,
    PnpmDlx,
}

pub enum DshLaunch {
    Binary(PathBuf),
    Node {
        node: PathBuf,
        script: PathBuf,
        runner: Runner,
    },
}

impl DshLaunch {
    pub fn uses_npm(&self) -> bool {
        match self {
            DshLaunch::Node {
                runner: Runner::Npx,
                ..
            } => true,
            DshLaunch::Binary(binary) => looks_like(binary, "npx"),
            _ => false,
        }
    }
}

pub fn resolve(path: &str) -> Result<DshLaunch, String> {
    if let Ok(explicit) = std::env::var("DSH_BINARY") {
        let binary = PathBuf::from(&explicit);
        if !binary.is_file() {
            return Err(format!("DSH_BINARY is set but is not a file: {explicit}"));
        }
        return Ok(unwrap_or_binary(binary, path, Runner::Direct));
    }
    if let Some(dsh) = host_path::find("dsh", path) {
        return Ok(unwrap_or_binary(dsh, path, Runner::Direct));
    }
    if let Some(pnpm) = host_path::find("pnpm", path) {
        if let Some(launch) = unwrap_shim(&pnpm, path, Runner::PnpmDlx) {
            return Ok(launch);
        }
        return Ok(DshLaunch::Binary(pnpm));
    }
    if let Some(npx) = host_path::find("npx", path) {
        if let Some(launch) = unwrap_shim(&npx, path, Runner::Npx) {
            return Ok(launch);
        }
        return Ok(DshLaunch::Binary(npx));
    }
    Err(format!(
        "dsh is not installed. Install with:\npnpm add -g {DSH_PACKAGE}"
    ))
}

fn unwrap_or_binary(shim: PathBuf, path: &str, runner: Runner) -> DshLaunch {
    unwrap_shim(&shim, path, runner).unwrap_or(DshLaunch::Binary(shim))
}

fn unwrap_shim(shim: &Path, path: &str, runner: Runner) -> Option<DshLaunch> {
    let node = host_path::find_exe("node", path).or_else(|| sibling_node(shim))?;
    let script = host_path::node_cli_for_shim(shim)?;
    Some(DshLaunch::Node {
        node,
        script,
        runner,
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
            prefix_package_runner(&mut command, binary_runner(binary));
            command.args(args);
            command
        }
        DshLaunch::Node {
            node,
            script,
            runner,
        } => {
            let mut command = Command::new(node);
            command.arg(script);
            prefix_package_runner(&mut command, *runner);
            command.args(args);
            command
        }
    };
    host_path::hide_window(&mut command);
    command
        .env("DSH_HOME", dsh_home)
        .env("PATH", path)
        .env("CI", "1")
        .env("COREPACK_ENABLE_DOWNLOAD_PROMPT", "0")
        .env("NPM_CONFIG_UPDATE_NOTIFIER", "false")
        .stdin(Stdio::null());
    command
}

fn prefix_package_runner(command: &mut Command, runner: Runner) {
    match runner {
        Runner::Direct => {}
        Runner::Npx => {
            command.args(["--yes", DSH_PACKAGE]);
        }
        Runner::PnpmDlx => {
            command.args(["dlx", DSH_PACKAGE]);
        }
    }
}

fn binary_runner(binary: &Path) -> Runner {
    if looks_like(binary, "npx") {
        Runner::Npx
    } else if looks_like(binary, "pnpm") {
        Runner::PnpmDlx
    } else {
        Runner::Direct
    }
}

fn looks_like(binary: &Path, name: &str) -> bool {
    binary
        .file_stem()
        .and_then(|stem| stem.to_str())
        .is_some_and(|stem| stem.eq_ignore_ascii_case(name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn npx_shim_fallback_still_passes_the_package() {
        let launch = DshLaunch::Binary(PathBuf::from("npx.cmd"));
        let rendered = format!(
            "{:?}",
            command(&launch, &["web"], Path::new("/tmp/dsh"), "/bin")
        );
        assert!(rendered.contains("--yes"));
        assert!(rendered.contains(DSH_PACKAGE));
        assert!(launch.uses_npm());
    }

    #[test]
    fn pnpm_dlx_does_not_go_through_npm() {
        let launch = DshLaunch::Node {
            node: PathBuf::from("node.exe"),
            script: PathBuf::from("pnpm.js"),
            runner: Runner::PnpmDlx,
        };
        let rendered = format!(
            "{:?}",
            command(&launch, &["web"], Path::new("/tmp/dsh"), "/bin")
        );
        assert!(rendered.contains("pnpm.js"));
        assert!(rendered.contains("dlx"));
        assert!(rendered.contains(DSH_PACKAGE));
        assert!(!rendered.contains("npx"));
        assert!(!launch.uses_npm());
    }

    #[test]
    fn node_npx_cli_does_not_go_through_cmd() {
        let launch = DshLaunch::Node {
            node: PathBuf::from("node.exe"),
            script: PathBuf::from("npx-cli.js"),
            runner: Runner::Npx,
        };
        let rendered = format!(
            "{:?}",
            command(&launch, &["web"], Path::new("/tmp/dsh"), "/bin")
        );
        assert!(rendered.contains("node.exe"));
        assert!(rendered.contains("npx-cli.js"));
        assert!(!rendered.contains("npx.cmd"));
        assert!(launch.uses_npm());
    }
}
