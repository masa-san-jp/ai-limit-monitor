#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use chrono::{Local, Offset, Utc};
use limit_core::error::FetchError;
use limit_core::http::ReqwestHttp;
use limit_core::model::{Service, ServiceUsage};
use limit_core::notice::Tracker;
use limit_core::view::{self, Snapshot};
use limit_core::{creds, fetch, grok};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindowBuilder, WindowEvent};
use tauri_plugin_notification::NotificationExt;

const GROK_SCRIPT: &str = include_str!("grok_script.js");

#[derive(Default)]
struct Store {
    claude: Option<ServiceUsage>,
    codex: Option<ServiceUsage>,
    grok: Option<ServiceUsage>,
    tracker: Tracker,
}

impl Store {
    fn all(&self) -> Vec<ServiceUsage> {
        [&self.claude, &self.codex, &self.grok]
            .into_iter()
            .flatten()
            .cloned()
            .collect()
    }
}

struct App {
    store: Arc<Mutex<Store>>,
    wake: Mutex<Sender<()>>,
}

fn snapshot(store: &Store) -> Snapshot {
    let tz = *Local::now().offset();
    view::build(&store.all(), Utc::now(), tz)
}

/// UI とメニューバー表示を更新する。
fn publish(app: &AppHandle, store: &Store) {
    let snap = snapshot(store);
    let title = match snap.max_percent {
        Some(p) => format!("{}%", p.round()),
        None => "--".to_string(),
    };
    if let Some(tray) = app.tray_by_id("main") {
        let _ = tray.set_title(Some(title));
    }
    let _ = app.emit("usage-updated", &snap);
}

fn notify(app: &AppHandle, usage: &ServiceUsage, store: &mut Store) {
    for n in store.tracker.update(usage) {
        let _ = app.notification().builder().title(n.title).body(n.body).show();
    }
}

fn poll_once(app: &AppHandle, http: &ReqwestHttp, shared: &Arc<Mutex<Store>>) -> (bool, Option<u64>) {
    let now = Utc::now();
    let mut failed = false;
    let mut retry_after = None;

    for svc in [Service::Claude, Service::Codex] {
        let result = match svc {
            Service::Claude => creds::load_claude(now).and_then(|c| fetch::fetch_claude(http, &c, now)),
            _ => creds::load_codex().and_then(|c| fetch::fetch_codex(http, &c, now)),
        };
        if let Err(e) = &result {
            failed = true;
            if let FetchError::RateLimited { retry_after_secs } = e {
                retry_after = *retry_after_secs;
            }
        }
        let mut s = shared.lock().unwrap();
        let prev = if svc == Service::Claude { s.claude.clone() } else { s.codex.clone() };
        let usage = fetch::resolve(svc, prev.as_ref(), result, now);
        notify(app, &usage, &mut s);
        if svc == Service::Claude {
            s.claude = Some(usage);
        } else {
            s.codex = Some(usage);
        }
    }
    publish(app, &shared.lock().unwrap());
    (failed, retry_after)
}

fn spawn_poller(app: AppHandle, shared: Arc<Mutex<Store>>, rx: std::sync::mpsc::Receiver<()>) {
    std::thread::spawn(move || {
        let Ok(http) = ReqwestHttp::new() else { return };
        let mut failures: u32 = 0;
        loop {
            let (failed, retry_after) = poll_once(&app, &http, &shared);
            failures = if failed { failures + 1 } else { 0 };
            let delay = fetch::next_delay_secs(300, failures, retry_after);
            // 手動更新(refresh_now)で即時に起床できる
            let _ = rx.recv_timeout(Duration::from_secs(delay));
        }
    });
}

/// ネットワークを使わず、残り時間表示だけを30秒ごとに更新する。
fn spawn_ticker(app: AppHandle, shared: Arc<Mutex<Store>>) {
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(30));
        publish(&app, &shared.lock().unwrap());
    });
}

// ---------- commands ----------

#[tauri::command]
fn get_snapshot(state: State<App>) -> Snapshot {
    snapshot(&state.store.lock().unwrap())
}

#[tauri::command]
fn refresh_now(state: State<App>) {
    let _ = state.wake.lock().unwrap().send(());
}

#[tauri::command]
fn quit_app(app: AppHandle) {
    app.exit(0);
}

#[tauri::command]
fn open_grok_login(app: AppHandle) -> Result<(), String> {
    show_grok_window(&app, true)
}

/// grok.com の注入スクリプトからの報告。サイズ上限と解析失敗時の破棄で入力を絞る。
#[tauri::command]
fn grok_report(app: AppHandle, state: State<App>, payload: String) {
    if payload.len() > 4096 {
        return;
    }
    let now = Utc::now();
    let Ok(windows) = grok::parse(&payload, now) else { return };
    let mut s = state.store.lock().unwrap();
    let usage = fetch::resolve(Service::Grok, s.grok.as_ref(), Ok(windows), now);
    notify(&app, &usage, &mut s);
    s.grok = Some(usage);
    publish(&app, &s);
}

fn show_grok_window(app: &AppHandle, visible: bool) -> Result<(), String> {
    if let Some(w) = app.get_webview_window("grok") {
        if visible {
            let _ = w.show();
            let _ = w.set_focus();
        }
        return Ok(());
    }
    let url = grok::USAGE_PAGE.parse().map_err(|_| "invalid url".to_string())?;
    WebviewWindowBuilder::new(app, "grok", WebviewUrl::External(url))
        .title("Grok にログイン")
        .inner_size(480.0, 720.0)
        .visible(visible)
        .initialization_script(GROK_SCRIPT)
        .build()
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn toggle_popover(app: &AppHandle, rect: tauri::Rect) {
    let Some(w) = app.get_webview_window("main") else { return };
    if w.is_visible().unwrap_or(false) {
        let _ = w.hide();
        return;
    }
    let scale = w.scale_factor().unwrap_or(1.0);
    let pos = rect.position.to_physical::<f64>(scale);
    let size = rect.size.to_physical::<f64>(scale);
    let win_w = w.outer_size().map(|s| s.width as f64).unwrap_or(340.0 * scale);
    let x = pos.x + size.width / 2.0 - win_w / 2.0;
    let y = pos.y + size.height;
    let _ = w.set_position(tauri::PhysicalPosition::new(x, y));
    let _ = w.show();
    let _ = w.set_focus();
}

fn main() {
    let (tx, rx) = channel::<()>();
    let store = Arc::new(Mutex::new(Store::default()));

    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .manage(App { store: store.clone(), wake: Mutex::new(tx) })
        .invoke_handler(tauri::generate_handler![
            get_snapshot,
            refresh_now,
            quit_app,
            open_grok_login,
            grok_report
        ])
        .on_window_event(|window, event| match (window.label(), event) {
            // ポップオーバーはフォーカスを失ったら閉じる
            ("main", WindowEvent::Focused(false)) => {
                let _ = window.hide();
            }
            // Grok ウィンドウは閉じずに隠す(セッションと使用量の監視を維持)
            ("grok", WindowEvent::CloseRequested { api, .. }) => {
                api.prevent_close();
                let _ = window.hide();
            }
            _ => {}
        })
        .setup(move |app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let handle = app.handle().clone();
            TrayIconBuilder::with_id("main")
                .icon(app.default_window_icon().cloned().ok_or("icon missing")?)
                .title("--")
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        rect,
                        ..
                    } = event
                    {
                        toggle_popover(tray.app_handle(), rect);
                    }
                })
                .build(app)?;

            // Grok は非表示ウィンドウで常駐させ、ログイン済みなら使用量を拾う
            let _ = show_grok_window(&handle, false);
            spawn_poller(handle.clone(), store.clone(), rx);
            spawn_ticker(handle, store.clone());
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running app");
}
