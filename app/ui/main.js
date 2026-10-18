const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;
const $ = (id) => document.getElementById(id);

function render(snap) {
  $('services').innerHTML = snap.services.map(Render.serviceHtml).join('');
  $('updated').textContent = '最終更新 ' + snap.updated_label;
}

$('refresh').addEventListener('click', () => invoke('refresh_now'));
$('quit').addEventListener('click', () => invoke('quit_app'));
$('grok-login').addEventListener('click', () => invoke('open_grok_login'));

listen('usage-updated', (e) => render(e.payload));
invoke('get_snapshot').then(render);
