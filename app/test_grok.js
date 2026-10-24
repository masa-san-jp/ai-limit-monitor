const assert = require('assert');
const vm = require('vm');
const fs = require('fs');
const script = fs.readFileSync(__dirname + '/src-tauri/src/grok_script.js', 'utf8');
function harness(text, pathname = '/dashboard', hostname = 'grok.com', meters = [], invoke) {
  let tick;
  const reports = [];
  const doc = { body: { innerText: text }, querySelectorAll: () => meters, title: '' };
  vm.runInNewContext(script, {
    location: { hostname, pathname }, document: doc,
    window: { __TAURI_INTERNALS__: { invoke: invoke || ((_, args) => {
      reports.push(JSON.parse(args.payload)); return Promise.resolve();
    }) } }, setInterval: fn => { tick = fn; }
  });
  if (tick) tick();
  return { reports, doc };
}
function read(...args) {
  return harness(...args).reports;
}
const meter = (value, direction = '72% used', label = 'Weekly included usage') => ({ getAttribute: name => ({
  'aria-label': label, 'aria-valuenow': value, 'aria-valuetext': direction
})[name] ?? null });
assert.strictEqual(read('', '/dashboard', 'grok.com', [meter('72.30769')])[0].used_percent, 72.30769);
for (const meters of [[meter('101')], [meter('')], [meter(null)], [meter('72', '72%')], [meter('72', '72% used', 'Monthly usage')], [meter('72'), meter('72')]]) {
  assert.deepStrictEqual(read('', '/dashboard', 'grok.com', meters), []);
}
assert.strictEqual(read('Weekly usage\n30% used\nResets in 2 days')[0].used_percent, 30);
assert.strictEqual(read('週間使用量\n残り 70%\nリセット 2日')[0].used_percent, 30);
assert.strictEqual(read('Weekly pool\n70% remaining')[0].used_percent, 30);
const dashboard = 'Current plan\nPremium\n72% used\nWeekly included usage · Resets Oct 28\nIncluded usage with your subscription. Used before your credits.\nAccount balance\n$0.00 used this month\nCredits available $0.00';
assert.strictEqual(read(dashboard)[0].used_percent, 72);
assert.strictEqual(read('72.30769% used\nWeekly included usage · Resets Oct 28')[0].used_percent, 72.30769);
for (const value of ['-1% used', '101% used', '72%', '72% used 28% remaining']) {
  assert.deepStrictEqual(read(value + '\nWeekly included usage · Resets Oct 28'), []);
}
assert.deepStrictEqual(read(dashboard + '\n20% used\nWeekly included usage'), []);
assert.strictEqual(read('Weekly usage\n30% used\nResets at 2026-10-25T08:00:00Z')[0].resets_at, '2026-10-25T08:00:00Z');
for (const text of [
  'Usage\n30% used', 'Monthly usage\n30% used',
  'Weekly usage\nPrepaid credit\n30% used', 'Weekly usage\n130% used',
  'Weekly usage\n30%', 'Weekly usage\n30% used\n70% remaining',
  'Weekly usage\n30% used\nWeekly usage\n40% used'
]) assert.deepStrictEqual(read(text), [], text);
assert.deepStrictEqual(read('Weekly usage\n30% used', '/'), []);
assert.deepStrictEqual(read('Weekly usage\n30% used', '/dashboard', 'accounts.x.ai'), []);
console.log('grok extraction tests ok (synthetic fixtures; live DOM not verified)');
async function checkDelivery() {
  const sent = harness(dashboard);
  await Promise.resolve(); await Promise.resolve();
  assert.match(sent.doc.title, /72% 送信済み/);
  const rejected = harness(dashboard, '/dashboard', 'grok.com', [], () => Promise.reject(new Error('denied')));
  await Promise.resolve(); await Promise.resolve();
  assert.match(rejected.doc.title, /送信に失敗/);
  assert.match(harness('').doc.title, /週間枠を読み取れません/);
  console.log('grok delivery status tests ok');
}
checkDelivery().catch(error => { console.error(error); process.exitCode = 1; });
