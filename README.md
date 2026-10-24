# AI Limit Monitor

Claude / Codex / Grok の短時間・週間制限の消費率と、リセットまでの残り時間を macOS のメニューバーに表示します。

**試験版 / macOS 専用。** 各サービスの公式製品ではありません。非公式エンドポイントや画面の表示形式に依存するため、サービス側の変更で取得できなくなる場合があります。

- Claude Code / Codex: 開発環境で上限表示を確認済み。他のアカウント・プランでの動作は未保証です。
- Grok: アプリ内の別ウィンドウでログインし、ダッシュボードの週間使用量を確認できます。メニューバーへの自動反映は実験的で、未取得のままになる場合があります。
- Grok のリセット日が日付のみの場合、正確なリセット時刻や残り時間は表示しません。

## Mac でのビルド

前提: Xcode Command Line Tools、Rust(rustup)、Node.js

```
git clone https://github.com/masa-san-jp/ai-limit-monitor.git
cd ai-limit-monitor
cargo test --workspace                 # コアのテスト(39件)。まずここが通ることを確認
python3 scripts/make_icon.py           # 仮アイコン生成(初回のみ)
cd app && npm install
npx tauri icon src-tauri/icons/icon.png   # .icns 等を生成(初回のみ)
npm run tauri dev                      # 開発実行
npm run tauri build                    # .app / .dmg を生成
```

## 事前ログイン

- Claude: `claude` を一度起動してログイン
- Codex: `codex login`
- Grok: メニューバーのポップオーバー下部「Grok / Cursor 使用量」からアプリ内ウィンドウでログイン。
  統合済みのアカウントでログインし、`https://grok.com/dashboard` の Usage を開いてください。
  The X Subscription の共有週間枠を対象にします。旧 Cursor プランの月間枠・プリペイド残高・第三者モデルの課金は対象外です。
  開発環境ではアプリ内ログインと週間使用量画面の表示を確認済みです。画面を開けることと、メニューバーへの自動反映の成功は別です。

## 初回に必ず確認すること

1. Claude/Codex: 表示値を各公式画面(Claude Code `/usage`、Codex Settings > Usage)と突合する。ずれる場合は `core/src/claude.rs` / `codex.rs` のフィールド名を実レスポンスに合わせる。
2. Grok: 「Grok / Cursor 使用量」からダッシュボードを開く。自動反映されない場合は、別ウィンドウの表示を参照してください。
3. macOS が Keychain のアクセス許可を求めたら「許可」(Claude Code の認証情報の読み取り。読み取り専用)。

## セキュリティ設計

- Claude/Codex は既存 CLI の認証を読み取り専用で参照し、アプリ独自のファイルへの保存・更新・ログ出力をしません(`Secret` 型は Debug/Display が伏せ字、Serialize 非実装)。トークンは各サービスへの認証付きリクエストに使います。
- Rust の HTTP 取得処理は `api.anthropic.com` / `chatgpt.com` / `grok.com` を許可リストとし、https のみ・リダイレクト追従なしです。現在の Claude/Codex の取得先は最初の2ホストです。
- Grok の WebView は上記 HTTP 許可リストの対象外です。ログイン画面やページ内リソースへの通信はサイトと WebView が行い、Cookie などのログインセッションは WebView が端末内に保持する場合があります。
- ローカル UI に許可するコマンドと、grok.com の WebView に許可するコマンド(`grok_report` のみ)を capability で分離。
- Claude/Codex の定期ポーリングは5分以上、失敗時はバックオフし、前回値を「古い値」として表示します。手動更新は即時実行します。Grok の画面読み取りは5秒ごとで、値が変わらない場合のアプリへの報告は5分間抑制します。
- アプリ独自のテレメトリはありません。WebView で開く外部サイトの計測処理は、そのサイト側の動作に従います。
- 認証情報・Cookie・実アカウントのレスポンスを Issue やスクリーンショットへ含めないでください。

## テスト

```
cargo test --workspace   # コア(39件)
cd app && npm run test:ui  # UI 描画・Grok 抽出ロジック
```

Grok の抽出テストは合成画面テキストによるものです。週間枠と使用済み／残量の方向が明示された単一のパーセントだけを採用し、不明確な表示は送信しません。実画面との突合は引き続き必要です。

## ライセンス

[MIT](LICENSE)。サービス名・商標は各権利者に帰属します。
