(function () {
  if (!/(^|\.)grok\.com$/.test(location.hostname)) return;
  if (window.__limitMonitorInstalled) return;
  window.__limitMonitorInstalled = true;

  // 注意: 画面の文言・構造は実アカウントで要較正。見つからなければ何も送らない。
  function extract() {
    var text = (document.body && document.body.innerText) || '';
    var m = text.match(/(使用状況|使用量|usage)[\s\S]{0,300}/i);
    if (!m) return null;
    var seg = m[0];
    var p = seg.match(/(\d{1,3}(?:\.\d+)?)\s*%/);
    if (!p) return null;
    var pct = parseFloat(p[1]);
    var around = seg.slice(Math.max(0, p.index - 20), p.index + p[0].length + 20);
    if (/残り|remaining|left/i.test(around)) pct = 100 - pct;
    var out = { used_percent: pct };
    var iso = seg.match(/\d{4}-\d{2}-\d{2}T[\d:.]+(?:Z|[+-]\d{2}:\d{2})/);
    if (iso) out.resets_at = iso[0];
    else {
      var r = seg.match(/(?:リセット|reset)[^\n]{0,40}/i);
      if (r) out.reset_text = r[0];
    }
    return out;
  }

  var last = '', lastSent = 0;
  setInterval(function () {
    try {
      var d = extract();
      if (!d) return;
      var s = JSON.stringify(d);
      var now = Date.now();
      if (s === last && now - lastSent < 5 * 60 * 1000) return;
      last = s; lastSent = now;
      window.__TAURI_INTERNALS__.invoke('grok_report', { payload: s });
    } catch (e) {}
  }, 5000);
})();
