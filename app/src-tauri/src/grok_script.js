(function () {
  if (location.hostname !== 'grok.com') return;
  if (window.__limitMonitorInstalled) return;
  window.__limitMonitorInstalled = true;

  // 週間枠と方向が明示された値だけを採用する。
  // プリペイド残高や旧 Cursor の月間枠を週間使用量と混同しない。
  function extract(text) {
    var lines = text.split(/\n/).map(function (s) { return s.trim(); }).filter(Boolean);
    var candidates = [];
    for (var i = 0; i < lines.length; i++) {
      if (!/weekly|this week|週間|今週/i.test(lines[i])) continue;
      // 実 dashboard は「72% used」の次に週間枠の説明を表示する。
      // 説明中の credits は週間枠と残高の利用順序であり、別枠の見出しではない。
      if (/^Weekly included usage\b/i.test(lines[i])) {
        var preceding = (lines[i - 1] || '').match(/^(\d+(?:\.\d+)?)\s*%\s*(used|remaining)$/i);
        if (preceding) {
          var value = Number(preceding[1]);
          if (Number.isFinite(value) && value >= 0 && value <= 100) {
            candidates.push({ used_percent: /^remaining$/i.test(preceding[2]) ? 100 - value : value });
          }
        }
        continue;
      }
      var section = [];
      for (var j = i; j < Math.min(lines.length, i + 8); j++) {
        if (/prepaid|credit|billing|monthly|クレジット|残高|月間|請求/i.test(lines[j])) break;
        if (j > i && /weekly|this week|週間|今週/i.test(lines[j])) break;
        section.push(lines[j]);
      }
      var seg = section.join('\n');
      var percentages = seg.match(/\d+(?:\.\d+)?\s*%/g) || [];
      if (percentages.length !== 1) continue;
      var used = /\bused\b|\bconsumed\b|使用済|消費|使用率/i.test(seg);
      var remaining = /\bremaining\b|\bleft\b|残り|残量/i.test(seg);
      // "reset in 2 days" のようなリセット文言は方向判定から除外する。
      if (used === remaining) continue;
      var pct = parseFloat(percentages[0]);
      if (!Number.isFinite(pct) || pct < 0 || pct > 100) continue;
      var out = { used_percent: remaining ? 100 - pct : pct };
      var resetLine = section.findIndex(function (s) { return /reset|リセット/i.test(s); });
      if (resetLine !== -1) {
        var reset = section.slice(resetLine, resetLine + 2).join(' ');
        var iso = reset.match(/\d{4}-\d{2}-\d{2}T[\d:.]+(?:Z|[+-]\d{2}:\d{2})/);
        if (iso) out.resets_at = iso[0];
        else out.reset_text = reset;
      }
      candidates.push(out);
    }
    return candidates.length === 1 ? candidates[0] : null;
  }

  function extractMeter() {
    var meters = document.querySelectorAll('[role="progressbar"], progress, meter');
    var values = [];
    for (var i = 0; i < meters.length; i++) {
      var meter = meters[i];
      if (!/^Weekly included usage$/i.test(meter.getAttribute('aria-label') || '')) continue;
      var direction = (meter.getAttribute('aria-valuetext') || '').match(/^(\d+(?:\.\d+)?)\s*%\s*(used|remaining)$/i);
      var raw = meter.getAttribute('aria-valuenow');
      if (!direction || raw === null || !raw.trim()) continue;
      var value = Number(raw);
      if (!Number.isFinite(value) || value < 0 || value > 100 || Math.abs(value - Number(direction[1])) > 1) continue;
      values.push({ used_percent: /^remaining$/i.test(direction[2]) ? 100 - value : value });
    }
    return values.length === 1 ? values[0] : null;
  }

  var last = '', lastSent = 0;
  setInterval(function () {
    try {
      if (!/^\/dashboard(?:\/|$)/.test(location.pathname)) {
        document.title = 'Grok 使用量 — ダッシュボードを開いてください';
        return;
      }
      var d = extractMeter() || extract((document.body && document.body.innerText) || '');
      if (!d) {
        document.title = 'Grok 使用量 — 週間枠を読み取れません';
        return;
      }
      var s = JSON.stringify(d);
      var now = Date.now();
      if (s === last && now - lastSent < 5 * 60 * 1000) return;
      window.__TAURI_INTERNALS__.invoke('grok_report', { payload: s }).then(function () {
        last = s; lastSent = now;
        document.title = 'Grok 使用量 — ' + Math.round(d.used_percent) + '% 送信済み';
      }).catch(function () {
        document.title = 'Grok 使用量 — アプリへの送信に失敗';
      });
    } catch (e) {
      document.title = 'Grok 使用量 — 読み取り処理に失敗';
    }
  }, 5000);
})();
