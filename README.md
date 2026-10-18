# AI Limit Monitor

Claude / Codex / Grok の短時間・週間制限の消費率と、リセットまでの残り時間を macOS のメニューバーに表示します。

## Mac でのビルド

前提: Xcode Command Line Tools、Rust(rustup)、Node.js

```
git clone https://github.com/masa-san-jp/ai-limit-monitor.git
cd ai-limit-monitor
cargo test --workspace                 # コアのテスト(38件)。まずここが通ることを確認
python3 scripts/make_icon.py           # 仮アイコン生成(初回のみ)
cd app && npm install
npx tauri icon src-tauri/icons/icon.png   # .icns 等を生成(初回のみ)
npm run tauri dev                      # 開発実行
npm run tauri build                    # .app / .dmg を生成
```

## 事前ログイン

- Claude: `claude` を一度起動してログイン
- Codex: `codex login`
- Grok: メニューバーのポップオーバー下部「Grok にログイン」からアプリ内ウィンドウでログイン。
  ログイン後、Settings > Usage を一度開くと使用量を拾い始めます(要較正、下記)。

## 初回に必ず確認すること

1. Claude/Codex: 表示値を各公式画面(Claude Code `/usage`、Codex Settings > Usage)と突合する。ずれる場合は `core/src/claude.rs` / `codex.rs` のフィールド名を実レスポンスに合わせる。
2. Grok: `app/src-tauri/src/grok_script.js` の抽出ロジックを実画面の文言に合わせる。
3. macOS が Keychain のアクセス許可を求めたら「許可」(Claude Code の認証情報の読み取り。読み取り専用)。

## セキュリティ設計

- パスワードは保存しない。Claude/Codex は既存 CLI の認証を読み取り専用で参照し、コピー・更新・ログ出力をしない(`Secret` 型は Debug/Display が伏せ字、Serialize 非実装)。
- 通信先は `api.anthropic.com` / `chatgpt.com` / `grok.com` のみ(https、リダイレクト追従なし)。
- ローカル UI に許可するコマンドと、grok.com の WebView に許可するコマンド(`grok_report` のみ)を capability で分離。
- ポーリングは5分以上、429 時は指数バックオフ。失敗時は前回値を「古い値」として表示。
- テレメトリなし。

## テスト

```
cargo test --workspace   # コア(38件)
node app/test_render.js  # UI 描画ロジック
```
