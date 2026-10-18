(function (root) {
  const WARN = 80, DANGER = 95;
  const esc = (s) => String(s).replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));
  const barColor = (p) => (p >= DANGER ? '#E24B4A' : p >= WARN ? '#BA7517' : '#378ADD');
  const pctText = (p) => Math.round(p) + '%';
  const menuTitle = (snap) => (snap.max_percent == null ? '--' : pctText(snap.max_percent));
  const statusTag = (svc) => {
    if (svc.status === 'ok') return '<span class="tag ok">取得済み</span>';
    if (svc.status === 'stale') return '<span class="tag warn">古い値 ' + esc(svc.fetched_at_label) + '</span>';
    return '<span class="tag danger">取得不可</span>';
  };
  const windowHtml = (w) => {
    const p = Math.max(0, Math.min(100, w.used_percent));
    return '<div class="row"><span class="lbl">' + esc(w.label) + '</span>' +
      '<div class="bar"><div class="fill" style="width:' + p + '%;background:' + barColor(p) + '"></div></div>' +
      '<span class="pct">' + pctText(p) + '</span><span class="rst">' + esc(w.remaining_text) + '</span></div>';
  };
  const serviceHtml = (svc) => {
    const body = svc.windows.length
      ? svc.windows.map(windowHtml).join('')
      : '<div class="note">' + esc(svc.message || '') + '</div>';
    const hint = svc.status !== 'ok' && svc.windows.length && svc.message ? '<div class="note">' + esc(svc.message) + '</div>' : '';
    return '<div class="svc"><span>' + esc(svc.service) + '</span>' + statusTag(svc) + '</div>' + body + hint;
  };
  const api = { barColor, pctText, menuTitle, serviceHtml, esc };
  if (typeof module !== 'undefined' && module.exports) module.exports = api;
  else root.Render = api;
})(typeof window !== 'undefined' ? window : globalThis);
