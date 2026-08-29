# Kyber Code

Tauri chrome around the DeepSeek Harness web GUI. The engine is `dsh` only — Kyber Code does not reimplement the agent loop.

The first window is Kyber Code: official mark, near-black overlay titlebar, then the stock `dsh web` interface reskinned to the same mark.

## Development

```sh
npm install
npm install -g @deepseek-ai/dsh@0.1.1-rc.2
npm run tauri dev
```

Kyber Code spawns `dsh web --no-open --port 0` with an explicit `$DSH_HOME` under the app data directory (never implicit `~/.dsh`). It reads the printed tokenized URL (`http://127.0.0.1:<port>/?token=…`), loads that URL in the webview so the Host cookie handshake is first-party, and does not open a system browser.

Credentials: `DEEPSEEK_API_KEY` in the environment, or `$DSH_HOME/.credentials.yaml` after the GUI settings write one.

OpenAI Codex is bundled as [dsh-codex-connect](https://github.com/franksong2702/dsh-codex-connect) `0.1.0-alpha.4.21` (the pin verified against DSH `0.1.1-rc.2`). Kyber Code installs that plugin into the app `web` profile on first boot. Sign in from **Settings → Plugins → Plugin configuration → Codex Connect** — ChatGPT OAuth opens in the system browser, not the Kyber Code webview. Do not paste a Platform API key.
