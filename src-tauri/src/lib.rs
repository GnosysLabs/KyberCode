use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use base64::Engine;
use tauri::webview::{NewWindowResponse, PageLoadEvent};
use tauri_plugin_updater::UpdaterExt;
use tauri::{
    AppHandle, Manager, RunEvent, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
};
use url::Url;

mod codex_connect;
mod dsh_launch;
mod host_path;

#[cfg(target_os = "macos")]
use tauri::{LogicalPosition, TitleBarStyle};

const DSH_INSTALL: &str = "npm install -g @deepseek-ai/dsh@0.1.1-rc.2";
const READY_TIMEOUT: Duration = Duration::from_secs(90);
const LOGO_PNG: &[u8] = include_bytes!("../../src/assets/kyber-logo.png");
const CRYSTAL_PNG: &[u8] = include_bytes!("../../src/assets/kyber-crystal.png");
const SKIN_CSS: &str = concat!(
    include_str!("../../src/skin/tokens.css"),
    "\n",
    include_str!("../../src/skin/surfaces.css"),
);
const SKIN_JS: &str = include_str!("../../src/skin/skin.js");

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
    #[cfg(windows)]
    {
        let mut kill = Command::new("taskkill");
        kill.args(["/F", "/T", "/PID", &pid.to_string()]);
        host_path::hide_window(&mut kill);
        let _ = kill.status();
        let _ = child.wait();
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = child.kill();
        let _ = child.wait();
    }
}

fn resolve_dsh_home(app: &AppHandle) -> Result<PathBuf, String> {
    let current = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("could not resolve app data dir: {error}"))?
        .join("dsh");
    if current.exists() {
        return Ok(current);
    }
    if let Ok(home) = app.path().home_dir() {
        let legacy = host_path::legacy_dsh_home(&home);
        if legacy.exists() {
            return Ok(legacy);
        }
    }
    std::fs::create_dir_all(&current)
        .map_err(|error| format!("could not create DSH_HOME {}: {error}", current.display()))?;
    Ok(current)
}

fn spawn_dsh(
    dsh_home: &Path,
    path: &str,
    launch: &dsh_launch::DshLaunch,
) -> std::io::Result<Child> {
    let mut command = dsh_launch::command(
        launch,
        &["web", "--no-open", "--port", "0"],
        dsh_home,
        path,
    );
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    command.spawn()
}

fn is_harness_url(url: &Url) -> bool {
    match (url.scheme(), url.host_str()) {
        ("tauri", _) => true,
        ("http" | "https", Some("127.0.0.1" | "localhost" | "tauri.localhost")) => true,
        _ => false,
    }
}

fn open_in_browser(url: &str) {
    #[cfg(target_os = "macos")]
    let mut command = Command::new("open");
    #[cfg(target_os = "linux")]
    let mut command = Command::new("xdg-open");
    #[cfg(target_os = "windows")]
    let mut command = {
        let mut command = Command::new("cmd");
        command.args(["/C", "start", ""]);
        host_path::hide_window(&mut command);
        command
    };
    let _ = command.arg(url).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn();
}

fn keep_or_handoff(url: &Url) -> bool {
    if is_harness_url(url) {
        return true;
    }
    if matches!(url.scheme(), "http" | "https") {
        open_in_browser(url.as_str());
    }
    false
}

fn strip_ansi(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' && chars.peek() == Some(&'[') {
            chars.next();
            for next in chars.by_ref() {
                if next.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            output.push(ch);
        }
    }
    output
}

fn extract_ready_url(line: &str) -> Option<Url> {
    let cleaned = strip_ansi(line);
    let rest = cleaned.trim().strip_prefix("dsh web:")?.trim();
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

fn png_data_uri(bytes: &[u8]) -> String {
    format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    )
}

fn skin_script() -> String {
    format!(
        "(function(){{\nconst KYBER_CSS={css};\nconst KYBER_LOGO={logo};\nconst KYBER_CRYSTAL={crystal};\n{js}\n}})();",
        css = js_string(SKIN_CSS),
        logo = js_string(&png_data_uri(LOGO_PNG)),
        crystal = js_string(&png_data_uri(CRYSTAL_PNG)),
        js = SKIN_JS
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
    let path = host_path::augmented();
    let dsh_home = resolve_dsh_home(app)?;

    eval_status(window, "");

    let launch = dsh_launch::resolve(&path)?;
    let bundled = std::fs::read_to_string(dsh_home.join("profiles/web/package.json"))
        .ok()
        .is_some_and(|body| codex_connect::is_bundled(&body));
    if !bundled {
        eval_status(window, "Installing Codex Connect…");
    }
    if let Err(error) = codex_connect::ensure(&dsh_home, &path, &launch) {
        eval_status(window, "");
        return Err(error);
    }
    eval_status(window, "");
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
            let _ = window.eval(&skin_script());
            thread::sleep(Duration::from_millis(800));
            let _ = window.eval(&skin_script());
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

fn start_updater(app: AppHandle) {
    thread::Builder::new()
        .name("kyber-updater".into())
        .spawn(move || {
            // Give the boot sequence a quiet window before touching the network.
            thread::sleep(Duration::from_secs(10));
            let result = match app.updater() {
                Ok(updater) => tauri::async_runtime::block_on(async move {
                    updater.check().await.map(|update| (update, updater))
                }),
                Err(error) => Err(error),
            };
            match result {
                Ok((Some(update), _updater)) => {
                    println!("kyber-updater: downloading {}", update.version);
                    let installed = tauri::async_runtime::block_on(
                        update.download_and_install(|chunk, total| {
                            if let Some(total) = total {
                                println!(
                                    "kyber-updater: {chunk}/{total} bytes"
                                );
                            }
                        }, || {}),
                    );
                    match installed {
                        Ok(()) => {
                            println!("kyber-updater: installed, restarting");
                            app.restart();
                        }
                        Err(error) => eprintln!("kyber-updater: install failed: {error}"),
                    }
                }
                Ok((None, _)) => println!("kyber-updater: already up to date"),
                Err(error) => eprintln!("kyber-updater: check failed: {error}"),
            }
        })
        .expect("failed to start the updater thread");
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let dsh = Arc::new(DshChild::new());
    let dsh_on_exit = Arc::clone(&dsh);

    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(move |app| {
            let builder =
                WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
                    .title("Kyber Code")
                    .inner_size(1280.0, 800.0)
                    .initialization_script(&skin_script())
                    .on_navigation(keep_or_handoff)
                    .on_new_window(|url, _features| {
                        let _ = keep_or_handoff(&url);
                        NewWindowResponse::Deny
                    });
            // hidden_title/title_bar_style/traffic_light_position are macOS-only APIs.
            #[cfg(target_os = "macos")]
            let builder = builder
                .hidden_title(true)
                .title_bar_style(TitleBarStyle::Overlay)
                .traffic_light_position(LogicalPosition::new(18.0, 18.0));
            let window = builder.build()?;
            start_harness(app.handle().clone(), window, Arc::clone(&dsh));
            start_updater(app.handle().clone());
            Ok(())
        })
        .on_page_load(|webview, payload| {
            if payload.url().host_str() == Some("127.0.0.1")
                && matches!(
                    payload.event(),
                    PageLoadEvent::Started | PageLoadEvent::Finished
                )
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_ready_url_reads_tokenized_loopback() {
        let url = extract_ready_url("dsh web: http://127.0.0.1:4123/?token=abc ready").unwrap();
        assert_eq!(url.host_str(), Some("127.0.0.1"));
        assert_eq!(url.port(), Some(4123));
        assert_eq!(url.query(), Some("token=abc"));
        let colored = extract_ready_url(
            "dsh web: \u{1b}[32mhttp://127.0.0.1:4123/?token=abc\u{1b}[0m ready",
        )
        .unwrap();
        assert_eq!(colored.query(), Some("token=abc"));
    }

    #[test]
    fn extract_ready_url_rejects_non_loopback() {
        assert!(extract_ready_url("dsh web: http://example.com/?token=abc").is_none());
        assert!(extract_ready_url("listening on http://127.0.0.1:9/").is_none());
    }

    #[test]
    fn harness_urls_stay_in_the_app() {
        assert!(is_harness_url(&Url::parse("http://127.0.0.1:4123/?token=abc").unwrap()));
        assert!(is_harness_url(&Url::parse("tauri://localhost/index.html").unwrap()));
        assert!(!is_harness_url(&Url::parse("https://auth.openai.com/oauth/authorize").unwrap()));
    }

    #[test]
    fn skin_script_reskins_in_place_instead_of_bolting_a_logo_bar() {
        let script = skin_script();
        assert!(script.contains("--dsw-alias-bg-base"));
        assert!(script.contains("!important"));
        assert!(script.contains("kyber-hero-mark"));
        assert!(script.contains("KYBER_CRYSTAL"));
        assert!(script.contains("data:image/png;base64,"));
        assert!(script.contains("plugin:window|start_dragging"));
        assert!(script.contains("themeCube"));
        assert!(script.contains("kyber-boot"));
        assert!(script.contains("kyber-throb"));
        assert!(script.contains("color: white"));
        assert!(script.contains("background: #7b61ff !important"));
        assert!(!script.contains("padding-top: 52px"));
        assert!(!script.contains("kyber-chrome"));
    }
}
