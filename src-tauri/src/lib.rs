use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use base64::Engine;
use tauri::webview::PageLoadEvent;
use tauri::{AppHandle, Manager, RunEvent, WebviewWindow};
use url::Url;

const DSH_INSTALL: &str = "npm install -g @deepseek-ai/dsh@0.1.1-rc.2";
const READY_TIMEOUT: Duration = Duration::from_secs(90);
const LOGO_PNG: &[u8] = include_bytes!("../../src/assets/kyber-logo.png");

struct DshChild {
    child: Mutex<Option<Child>>,
}

impl DshChild {
    fn new() -> Self {
        Self {
            child: Mutex::new(None),
        }
    }

    fn kill(&self) {
        let Ok(mut slot) = self.child.lock() else {
            return;
        };
        let Some(mut child) = slot.take() else {
            return;
        };
        kill_tree(&mut child);
    }
}

impl Drop for DshChild {
    fn drop(&mut self) {
        self.kill();
    }
}

fn kill_tree(child: &mut Child) {
    let pid = child.id();
    #[cfg(unix)]
    {
        let _ = Command::new("kill")
            .args(["-TERM", &format!("-{pid}")])
            .status();
        let _ = child.wait();
    }
    #[cfg(not(unix))]
    {
        let _ = child.kill();
        let _ = child.wait();
    }
}

fn augmented_path() -> String {
    let extras = ["/opt/homebrew/bin", "/usr/local/bin"];
    let current = std::env::var("PATH").unwrap_or_default();
    let mut parts: Vec<String> = extras.iter().map(|s| (*s).to_string()).collect();
    for part in current.split(':') {
        if !part.is_empty() && !parts.iter().any(|existing| existing == part) {
            parts.push(part.to_string());
        }
    }
    parts.join(":")
}

fn find_on_path(name: &str, path: &str) -> Option<PathBuf> {
    path.split(':')
        .map(|dir| Path::new(dir).join(name))
        .find(|candidate| candidate.is_file())
}

enum DshLaunch {
    Direct(PathBuf),
    Npx(PathBuf),
}

fn resolve_dsh(path: &str) -> Result<DshLaunch, String> {
    if let Ok(explicit) = std::env::var("DSH_BINARY") {
        let binary = PathBuf::from(&explicit);
        if binary.is_file() {
            return Ok(DshLaunch::Direct(binary));
        }
        return Err(format!(
            "DSH_BINARY is set but is not a file: {explicit}"
        ));
    }
    if let Some(dsh) = find_on_path("dsh", path) {
        return Ok(DshLaunch::Direct(dsh));
    }
    if let Some(npx) = find_on_path("npx", path) {
        return Ok(DshLaunch::Npx(npx));
    }
    Err(format!(
        "dsh is not installed. Install with:\n{DSH_INSTALL}"
    ))
}

fn spawn_dsh(dsh_home: &Path, path: &str, launch: &DshLaunch) -> std::io::Result<Child> {
    let mut command = match launch {
        DshLaunch::Direct(binary) => {
            let mut command = Command::new(binary);
            command.args(["web", "--no-open", "--port", "0"]);
            command
        }
        DshLaunch::Npx(npx) => {
            let mut command = Command::new(npx);
            command.args(["--yes", "@deepseek-ai/dsh", "web", "--no-open", "--port", "0"]);
            command
        }
    };
    command
        .env("DSH_HOME", dsh_home)
        .env("PATH", path)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    command.spawn()
}

fn extract_ready_url(line: &str) -> Option<Url> {
    let rest = line.trim().strip_prefix("dsh web:")?.trim();
    let candidate = rest.split_whitespace().next()?;
    let parsed = Url::parse(candidate).ok()?;
    if parsed.scheme() != "http" || parsed.host_str() != Some("127.0.0.1") {
        return None;
    }
    match parsed.path() {
        "" | "/" => Some(parsed),
        _ => None,
    }
}

fn js_string(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "\"\"".into())
}

fn eval_js(window: &WebviewWindow, script: &str) {
    for _ in 0..40 {
        if window.eval(script).is_ok() {
            return;
        }
        thread::sleep(Duration::from_millis(50));
    }
}

fn eval_status(window: &WebviewWindow, text: &str) {
    eval_js(
        window,
        &format!(
            "window.__KYBER__ && window.__KYBER__.setStatus({})",
            js_string(text)
        ),
    );
}

fn eval_error(window: &WebviewWindow, text: &str) {
    eval_js(
        window,
        &format!(
            "window.__KYBER__ && window.__KYBER__.setError({})",
            js_string(text)
        ),
    );
}

fn logo_data_uri() -> String {
    format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(LOGO_PNG)
    )
}

fn skin_script() -> String {
    let logo = logo_data_uri();
    format!(
        r##"(function () {{
  try {{
    if (location.hostname !== '127.0.0.1') return;
    var LOGO = {logo};
    function ensureDark() {{
      document.documentElement.setAttribute('data-ds-dark-theme', '');
      if (document.body) document.body.setAttribute('data-ds-dark-theme', '');
    }}
    function ensureSkin() {{
      if (document.getElementById('kyber-skin')) return;
      var style = document.createElement('style');
      style.id = 'kyber-skin';
      style.textContent = [
        'html, body {{ background: #07070b !important; color-scheme: dark; }}',
        'body {{ padding-top: 52px !important; }}',
        'html, body, body[data-ds-dark-theme] {{',
        '  --dsw-alias-bg-base: #07070b;',
        '  --dsw-alias-bg-layer-1: #0c0c12;',
        '  --dsw-alias-bg-layer-2: #12121a;',
        '  --dsw-alias-bg-layer-3: #181822;',
        '  --dsw-alias-bg-module-platform: #0c0c12;',
        '  --dsw-alias-bg-overlay: #1a1a24;',
        '  --dsw-specific-sidebar-fill: #07070b;',
        '  --dsw-specific-sidebar-nav-item-active: #16161f;',
        '  --dsw-specific-sidebar-nav-item-hover: #101018;',
        '  --dsw-specific-bubble: #12121a;',
        '  --dsw-specific-bubble-highlight: #1a1428;',
        '  --dsw-specific-input-major: #12121a;',
        '  --dsw-specific-selector: #12121a;',
        '  --dsw-specific-menu: #12121a;',
        '  --dsw-alias-button-info-fill: #7b61ff;',
        '  --dsw-alias-button-info-hover: #ff4dff;',
        '  --dsw-alias-state-business-primary: #7b61ff;',
        '  --dsw-alias-state-business-tertiary: #1a1428;',
        '  --dsw-static-deepseek-400: #9a6bff;',
        '  --dsw-static-deepseek-450: #7b61ff;',
        '  --dsw-static-deepseek-500: #7b61ff;',
        '  --dsw-alias-brand-primary-new-colorprimary-new-color: #7b61ff;',
        '}}',
        '#kyber-chrome {{',
        '  position: fixed; top: 0; left: 0; right: 0; height: 52px;',
        '  z-index: 2147483647; display: flex; align-items: center;',
        '  padding: 0 16px 0 86px; background: #07070b;',
        '  box-shadow: inset 0 -1px 0 rgba(123, 97, 255, 0.16);',
        '}}',
        '#kyber-chrome [data-tauri-drag-region] {{',
        '  position: absolute; inset: 0 0 0 86px;',
        '}}',
        '#kyber-chrome img {{',
        '  position: relative; z-index: 1; height: 22px; width: auto;',
        '  pointer-events: none; user-select: none;',
        '}}',
        '[class*="EmptyState"] svg, [class*="empty-state"] svg,',
        '[class*="Welcome"] svg, [class*="lockup"] svg,',
        '[class*="stone"] svg, [class*="diamond"] svg {{ display: none !important; }}',
        '[style*="254, 245, 231"], [style*="#fef5e7"], [style*="#FEF5E7"] {{',
        '  background: #07070b !important;',
        '}}'
      ].join('\\n');
      (document.head || document.documentElement).appendChild(style);
    }}
    function ensureChrome() {{
      if (document.getElementById('kyber-chrome') || !document.body) return;
      var bar = document.createElement('div');
      bar.id = 'kyber-chrome';
      var drag = document.createElement('div');
      drag.setAttribute('data-tauri-drag-region', '');
      var img = document.createElement('img');
      img.src = LOGO;
      img.alt = 'Kyber';
      bar.appendChild(drag);
      bar.appendChild(img);
      document.body.insertBefore(bar, document.body.firstChild);
    }}
    function apply() {{
      ensureDark();
      ensureSkin();
      ensureChrome();
    }}
    apply();
    document.addEventListener('DOMContentLoaded', apply);
    if (!window.__KYBER_SKIN_OBS__) {{
      window.__KYBER_SKIN_OBS__ = true;
      new MutationObserver(apply).observe(document.documentElement, {{
        childList: true,
        subtree: true,
        attributes: true,
        attributeFilter: ['data-ds-dark-theme']
      }});
    }}
  }} catch (e) {{}}
}})();"##,
        logo = js_string(&logo)
    )
}

fn tail_lines(lines: &Mutex<Vec<String>>) -> String {
    lines
        .lock()
        .map(|buffer| buffer.join("\n"))
        .unwrap_or_default()
}

fn pump_output(
    reader: impl BufRead + Send + 'static,
    lines: Arc<Mutex<Vec<String>>>,
    ready: Arc<Mutex<Option<Url>>>,
) {
    thread::spawn(move || {
        for line in reader.lines().map_while(Result::ok) {
            if let Ok(mut buffer) = lines.lock() {
                buffer.push(line.clone());
                if buffer.len() > 80 {
                    let overflow = buffer.len() - 80;
                    buffer.drain(0..overflow);
                }
            }
            if let Some(url) = extract_ready_url(&line) {
                if let Ok(mut slot) = ready.lock() {
                    *slot = Some(url);
                }
            }
        }
    });
}

fn boot_dsh(app: &AppHandle, window: &WebviewWindow, dsh: &DshChild) -> Result<(), String> {
    let path = augmented_path();
    let dsh_home = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("could not resolve app data dir: {error}"))?
        .join("dsh");
    std::fs::create_dir_all(&dsh_home)
        .map_err(|error| format!("could not create DSH_HOME {}: {error}", dsh_home.display()))?;

    eval_status(window, "Starting DeepSeek Harness…");

    let launch = resolve_dsh(&path)?;
    let mut child = spawn_dsh(&dsh_home, &path, &launch).map_err(|error| {
        format!("failed to spawn dsh web: {error}\nInstall with:\n{DSH_INSTALL}")
    })?;

    let stdout = child.stdout.take().ok_or("dsh stdout was not piped")?;
    let stderr = child.stderr.take().ok_or("dsh stderr was not piped")?;
    *dsh.child.lock().map_err(|_| "dsh process lock poisoned")? = Some(child);

    let lines = Arc::new(Mutex::new(Vec::<String>::new()));
    let ready = Arc::new(Mutex::new(None::<Url>));
    pump_output(BufReader::new(stdout), Arc::clone(&lines), Arc::clone(&ready));
    pump_output(BufReader::new(stderr), Arc::clone(&lines), Arc::clone(&ready));

    let started = Instant::now();
    loop {
        if let Some(url) = ready.lock().ok().and_then(|slot| slot.clone()) {
            window
                .navigate(url)
                .map_err(|error| format!("failed to load the dsh GUI: {error}"))?;
            for _ in 0..20 {
                thread::sleep(Duration::from_millis(250));
                let _ = window.eval(&skin_script());
            }
            return Ok(());
        }

        if let Ok(mut slot) = dsh.child.lock() {
            if let Some(child) = slot.as_mut() {
                if let Ok(Some(status)) = child.try_wait() {
                    return Err(format!(
                        "dsh web exited ({status}) before the GUI was ready.\n{}",
                        tail_lines(&lines)
                    ));
                }
            }
        }

        if started.elapsed() > READY_TIMEOUT {
            dsh.kill();
            return Err(format!(
                "dsh web did not print a tokenized URL within 90s.\n{}",
                tail_lines(&lines)
            ));
        }

        thread::sleep(Duration::from_millis(100));
    }
}

fn start_harness(app: AppHandle, window: WebviewWindow, dsh: Arc<DshChild>) {
    thread::Builder::new()
        .name("kyber-dsh".into())
        .spawn(move || {
            if let Err(error) = boot_dsh(&app, &window, &dsh) {
                eval_error(&window, &error);
            }
        })
        .expect("failed to start the dsh supervisor thread");
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let dsh = Arc::new(DshChild::new());
    let dsh_on_exit = Arc::clone(&dsh);

    tauri::Builder::default()
        .setup(move |app| {
            let window = app
                .get_webview_window("main")
                .ok_or("missing main window")?;
            start_harness(app.handle().clone(), window, Arc::clone(&dsh));
            Ok(())
        })
        .on_page_load(|webview, payload| {
            if payload.event() == PageLoadEvent::Finished
                && payload.url().host_str() == Some("127.0.0.1")
            {
                let _ = webview.eval(&skin_script());
            }
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(move |_app, event| {
            if matches!(event, RunEvent::Exit | RunEvent::ExitRequested { .. }) {
                dsh_on_exit.kill();
            }
        });
}
