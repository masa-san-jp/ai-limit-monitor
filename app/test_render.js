const assert = require('assert');
const R = require('./ui/render.js');
assert.strictEqual(R.barColor(10), '#378ADD');
assert.strictEqual(R.barColor(80), '#BA7517');
assert.strictEqual(R.barColor(95), '#E24B4A');
assert.strictEqual(R.pctText(67.6), '68%');
assert.strictEqual(R.menuTitle({ max_percent: null }), '--');
assert.strictEqual(R.menuTitle({ max_percent: 68.2 }), '68%');
const html = R.serviceHtml({ service: 'Claude<script>', status: 'stale', fetched_at_label: '17:58', message: '再ログインが必要です',
  windows: [{ label: '週間', used_percent: 130, remaining_text: '3日 5時間' }] });
assert(!html.includes('<script>'), 'XSS escape');
assert(html.includes('width:100%'), 'clamp');
assert(html.includes('古い値 17:58'));
assert(html.includes('再ログインが必要です'));
const un = R.serviceHtml({ service: 'Grok', status: 'unavailable', message: 'ログインしてください', windows: [] });
assert(un.includes('取得不可') && un.includes('ログインしてください'));
console.log('render tests ok');
