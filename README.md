# Kyber

Tauri chrome around the DeepSeek Harness web GUI. The engine is `dsh` only — Kyber does not reimplement the agent loop.

The first window is Kyber: official mark, near-black overlay titlebar, then the stock `dsh web` interface reskinned to the same mark.

## Development

```sh
npm install
npm install -g @deepseek-ai/dsh@0.1.1-rc.2
npm run tauri dev
```

Kyber spawns `dsh web --no-open --port 0` with an explicit `$DSH_HOME` under the app data directory (never implicit `~/.dsh`). It reads the printed tokenized URL (`http://127.0.0.1:<port>/?token=…`), loads that URL in the webview so the Host cookie handshake is first-party, and does not open a system browser.

Credentials: `DEEPSEEK_API_KEY` in the environment, or `$DSH_HOME/.credentials.yaml` after the GUI settings write one.
