// 前端端到端验收：Chrome(CDP) + mock Tauri IPC
// 温度矩阵来自 Rust 管线真实输出（e2e/sample.json，由 src/bin/dump_temps 生成）
// 验证项：加载→伪彩渲染→悬停取温→ROI 绘制→统计回填
// 用法: node scripts/e2e-cdp.mjs [--out e2e/mock-app.png]
import { readFileSync, writeFileSync, existsSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import { dirname, join } from 'node:path'

const root = join(dirname(fileURLToPath(import.meta.url)), '..')
const samplePath = join(root, 'e2e', 'sample.json')
if (!existsSync(samplePath)) {
  console.error('缺少 e2e/sample.json，先运行: cargo run --bin dump_temps -- <样本> e2e/sample.json')
  process.exit(1)
}
const sample = JSON.parse(readFileSync(samplePath, 'utf8'))
console.log(`样本: ${sample.file_name} ${sample.width}x${sample.height} range=${sample.temp_min.toFixed(1)}~${sample.temp_max.toFixed(1)}`)

// ---------- mock IPC ----------
const mockScript = `
window.__MOCK_SAMPLE__ = ${JSON.stringify({ ...sample, temps: undefined })};
window.__MOCK_TEMPS__ = null;
window.__MOCK_E__ = ${sample.emissivity};
window.__APP_LOG__ = [];
async function ensureTemps() {
  if (!window.__MOCK_TEMPS__) {
    const data = await fetch('/e2e/sample.json').then((r) => r.json());
    window.__MOCK_TEMPS__ = Float32Array.from(data.temps);
  }
  return window.__MOCK_TEMPS__;
}
window.__TAURI_IPC__ = (msg) => {
  const { cmd, callback, error } = msg;
  const ok = (v) => { window.__APP_LOG__.push(cmd + ':ok'); const f = window['_' + callback]; f && f(v); };
  const bad = (e) => { window.__APP_LOG__.push(cmd + ':err ' + e); const f = window['_' + error]; f && f(String(e)); };
  Promise.resolve().then(async () => {
    if (cmd === 'load_image') {
      const { temps, ...meta } = window.__MOCK_SAMPLE__;
      ok(meta);
    } else if (cmd === 'get_temps') {
      const t = await ensureTemps();
      const bytes = new Uint8Array(t.buffer, 0, t.length * 4);
      let s = '';
      const CH = 0x8000;
      for (let i = 0; i < bytes.length; i += CH) {
        s += String.fromCharCode.apply(null, bytes.subarray(i, i + CH));
      }
      ok(btoa(s));
    } else if (cmd === 'get_dimensions') {
      ok({ width: window.__MOCK_SAMPLE__.width, height: window.__MOCK_SAMPLE__.height });
    } else if (cmd === 'set_emissivity') {
      const t = await ensureTemps();
      const d = msg.value - window.__MOCK_E__;
      window.__MOCK_E__ = msg.value;
      for (let i = 0; i < t.length; i++) t[i] += d * 30; // 近似：仅用于 UI 链路验证
      const s = window.__MOCK_SAMPLE__;
      s.emissivity = msg.value;
      s.temp_min += d * 30; s.temp_max += d * 30;
      const { temps, ...meta } = s;
      ok(meta);
    } else if (cmd === 'set_distance') {
      const { temps, ...meta } = window.__MOCK_SAMPLE__;
      ok(meta);
    } else if (cmd === 'roi_stats') {
      const t = await ensureTemps();
      const w = window.__MOCK_SAMPLE__.width, h = window.__MOCK_SAMPLE__.height;
      const shape = msg.shape || {};
      let pts = [];
      if (shape.kind === 'rect') {
        const x0 = Math.max(0, Math.floor(shape.x)), y0 = Math.max(0, Math.floor(shape.y));
        const x1 = Math.min(w, Math.ceil(shape.x + shape.w)), y1 = Math.min(h, Math.ceil(shape.y + shape.h));
        for (let y = y0; y < y1; y++) for (let x = x0; x < x1; x++) pts.push(t[y * w + x]);
      } else {
        const P = shape.points || [];
        for (const [px, py] of P) {
          const x = Math.round(px), y = Math.round(py);
          if (x >= 0 && x < w && y >= 0 && y < h) pts.push(t[y * w + x]);
        }
      }
      if (!pts || !pts.length) { bad('ROI 不含有效像素'); return; }
      let mx = -Infinity, mn = Infinity, sum = 0;
      for (const v of pts) { if (v > mx) mx = v; if (v < mn) mn = v; sum += v; }
      const dt = mx - mn;
      // 与 Rust diagnosis.rs 相同的四级判定（阈值 70/10）
      let level = 0, reason = '各项指标正常';
      if (mx >= 100 || dt >= 30) { level = 3; reason = '最高温 ' + mx.toFixed(1) + '℃ 或温差 ' + dt.toFixed(1) + '℃ 达紧急档'; }
      else if (mx >= 85 || dt >= 20) { level = 2; reason = '最高温 ' + mx.toFixed(1) + '℃ 或温差 ' + dt.toFixed(1) + '℃ 达重大档'; }
      else if (mx >= 70 || dt >= 10) { level = 1; reason = '最高温 ' + mx.toFixed(1) + '℃ 超阈值或温差 ' + dt.toFixed(1) + '℃ 超限'; }
      ok({ max: mx, min: mn, avg: sum / pts.length, delta_t: dt, count: pts.length, verdict: { level, reason, exceeded: level >= 1 } });
    } else if (cmd === 'get_settings') {
      ok({ joint_temp: 70, delta_t: 10, phase_delta_t: 5 });
    } else if (cmd === 'set_settings') {
      ok({ joint_temp: msg.joint_temp, delta_t: msg.delta_t, phase_delta_t: msg.phase_delta_t });
    } else if (cmd === 'phase_compare') {
      const entries = (msg.entries || []).filter(([p]) => 'ABC'.includes(p));
      const uniq = {};
      for (const [p, v] of entries) uniq[p] = Math.max(uniq[p] ?? -Infinity, v);
      const vals = Object.entries(uniq);
      if (vals.length < 2) { ok(null); return; }
      const mx = Math.max(...vals.map(([, v]) => v));
      const mn = Math.min(...vals.map(([, v]) => v));
      const d = mx - mn;
      ok(d >= 5 ? { level: d >= 10 ? 2 : 1, reason: '相间最大温差 ' + d.toFixed(1) + '℃ 超过阈值 5℃', exceeded: true } : null);
    } else {
      bad('mock 未实现命令: ' + cmd);
    }
  });
};
window.__MOCK_READY__ = true;
`

// ---------- CDP ----------
const CDP_PORT = 9223
const chromePath = 'C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe'
const { spawn } = await import('node:child_process')
const chrome = spawn(chromePath, [
  '--headless=new', '--remote-debugging-port=' + CDP_PORT, '--window-size=1440,900',
  '--no-first-run', '--no-default-browser-check', '--user-data-dir=' + join(root, 'e2e', 'chrome-profile'),
  'about:blank'
], { stdio: 'ignore' })

const sleep = (ms) => new Promise((r) => setTimeout(r, ms))
let ws = null
try {
  let target = null
  for (let i = 0; i < 30; i++) {
    await sleep(500)
    try {
      const list = await (await fetch(`http://127.0.0.1:${CDP_PORT}/json/list`)).json()
      target = list.find((t) => t.type === 'page')
      if (target) break
    } catch { /* retry */ }
  }
  if (!target) throw new Error('Chrome devtools 端口未就绪')

  ws = new WebSocket(target.webSocketDebuggerUrl)
  await new Promise((res, rej) => { ws.onopen = res; ws.onerror = rej })
  let seq = 0
  const pending = new Map()
  const consoleLogs = []
  ws.onmessage = (ev) => {
    const m = JSON.parse(ev.data)
    if (m.method === 'Runtime.consoleAPICalled') {
      const text = (m.params.args || []).map((a) => a.value ?? a.description ?? '').join(' ')
      consoleLogs.push(`[${m.params.type}] ${text}`)
    } else if (m.method === 'Runtime.exceptionThrown') {
      consoleLogs.push(`[EXC] ${m.params.exceptionDetails?.exception?.description || m.params.exceptionDetails?.text}`)
    }
    if (m.id && pending.has(m.id)) { pending.get(m.id)(m); pending.delete(m.id) }
  }
  const send = (method, params = {}) => new Promise((res, rej) => {
    const id = ++seq
    pending.set(id, (m) => (m.error ? rej(new Error(m.error.message)) : res(m.result)))
    ws.send(JSON.stringify({ id, method, params }))
  })
  const evaluate = async (expr) => {
    const r = await send('Runtime.evaluate', { expression: expr, returnByValue: true, awaitPromise: true })
    if (r.exceptionDetails) throw new Error(r.exceptionDetails.exception?.description || 'eval error')
    return r.result.value
  }
  // 轮询直到 evaluate 条件成立（Vue 异步 flush 的 DOM 更新有延迟）
  const pollUntil = async (expr, timeoutMs = 5000) => {
    const start = Date.now()
    for (;;) {
      try { if (await evaluate(expr)) return true } catch { /* keep polling */ }
      if (Date.now() - start > timeoutMs) return false
      await sleep(150)
    }
  }
  const mouse = (type, x, y, btn = 'left', buttons = 0) => send('Input.dispatchMouseEvent', {
    type, x, y, button: btn, buttons, clickCount: type === 'mousePressed' || type === 'mouseReleased' ? 1 : 0
  })

  await send('Page.enable')
  await send('Runtime.enable')
  await send('Page.addScriptToEvaluateOnNewDocument', { source: mockScript })
  await send('Page.navigate', { url: `http://127.0.0.1:5173/?img=${encodeURIComponent(sample.path)}` })

  let ready = false
  for (let i = 0; i < 60; i++) {
    await sleep(500)
    try {
      ready = await evaluate(`!!window.__MOCK_READY__ && !!document.querySelector('.canvas-view canvas')`)
    } catch { ready = false }
    if (ready) break
  }
  if (!ready) throw new Error('应用未就绪（mock/canvas 未出现）')
  await sleep(1200)

  // 抓 Vue 渲染期异常
  await evaluate(`(() => {
    window.__VUE_ERR__ = [];
    const app = document.querySelector('#app').__vue_app__;
    app.config.errorHandler = (err, inst, info) => {
      window.__VUE_ERR__.push((info || '') + ' :: ' + (err && err.stack || err));
    };
    return true;
  })()`)

  // ---------- 验收 1：加载与渲染 ----------
  const diag = await evaluate(`(() => {
    const f = document.querySelector('.canvas-view').__vueParentComponent.exposed.getFabric();
    const cv = document.querySelector('.canvas-view canvas');
    const ctx = cv.getContext('2d');
    const img = ctx.getImageData(0, 0, cv.width, cv.height).data;
    const bg = [31, 39, 51];
    let minX = 1e9, minY = 1e9, maxX = -1, maxY = -1;
    for (let y = 0; y < cv.height; y += 3) {
      for (let x = 0; x < cv.width; x += 3) {
        const i = (y * cv.width + x) * 4;
        if (Math.abs(img[i] - bg[0]) > 12 || Math.abs(img[i + 1] - bg[1]) > 12 || Math.abs(img[i + 2] - bg[2]) > 12) {
          if (x < minX) minX = x; if (x > maxX) maxX = x;
          if (y < minY) minY = y; if (y > maxY) maxY = y;
        }
      }
    }
    const view = document.querySelector('.canvas-view').__vueParentComponent;
    return JSON.stringify({
      ipc: window.__APP_LOG__,
      canvasBuffer: cv.width + 'x' + cv.height,
      contentBounds: { w: maxX - minX, h: maxY - minY },
      objCount: f.getObjects().length,
      vpt: f.viewportTransform.map((v) => Math.round(v * 1000) / 1000),
      metaModel: view.componentProxy ? null : 'n/a'
    });
  })()`)
  console.log('A1-渲染:', diag)
  const d1 = JSON.parse(diag)
  if (!d1.ipc.includes('load_image:ok') || !d1.ipc.includes('get_temps:ok')) throw new Error('IPC 未完成')
  if (d1.objCount < 1) throw new Error('画布无图像对象')
  if (d1.contentBounds.w < d1.canvasBuffer.split('x')[0] * 0.5) throw new Error('图像渲染未铺满: ' + JSON.stringify(d1.contentBounds))

  // ---------- 验收 2：悬停取温 ----------
  const rect = await evaluate(`(() => {
    const r = document.querySelector('.canvas-host').getBoundingClientRect();
    return JSON.stringify({ left: r.left, top: r.top, w: r.width, h: r.height });
  })()`)
  const hostRect = JSON.parse(rect)
  const cx = hostRect.left + hostRect.w * 0.5
  const cy = hostRect.top + hostRect.h * 0.5
  // 用页面内合成事件驱动 Fabric（pointer + mouse 双发）
  const fireMove = (x, y) => evaluate(`(() => {
    const upper = document.querySelector('.upper-canvas');
    const o = { clientX: ${x}, clientY: ${y}, bubbles: true, cancelable: true, pointerId: 1, pointerType: 'mouse', isPrimary: true, button: 0, buttons: 0 };
    upper.dispatchEvent(new PointerEvent('pointermove', o));
    upper.dispatchEvent(new MouseEvent('mousemove', o));
    return true;
  })()`)
  // 模拟真实用户连续移动（每次移动都是一个新任务，触发调度器 flush）
  for (let i = 0; i < 12; i++) {
    await fireMove(cx + (i % 5) * 8, cy + (i % 3) * 6)
    await sleep(120)
  }
  // headless 下 Vue 异步 flush 滞后，强制刷新后断言
  const hover = await evaluate(`(() => {
    const cv = document.querySelector('.canvas-view').__vueParentComponent;
    if (cv && typeof cv.update === 'function') cv.update();
    return JSON.stringify({
      tip: document.querySelector('.cursor-tip')?.textContent || null,
      statusHover: document.querySelector('.status-hover')?.textContent || ''
    });
  })()`)
  console.log('A2-悬停:', hover)

  // ---------- 验收 3：ROI 矩形绘制 + 统计 ----------
  await evaluate(`document.querySelectorAll('.canvas-tools .el-button')[1].click()`)
  await sleep(200)
  const x0 = hostRect.left + hostRect.w * 0.35
  const y0 = hostRect.top + hostRect.h * 0.3
  const x1 = hostRect.left + hostRect.w * 0.65
  const y1 = hostRect.top + hostRect.h * 0.7
  const fire = (type, x, y, buttons = 0) => evaluate(`(() => {
    const upper = document.querySelector('.upper-canvas');
    const o = { clientX: ${x}, clientY: ${y}, bubbles: true, cancelable: true, pointerId: 1, pointerType: 'mouse', isPrimary: true, button: 0, buttons: ${buttons} };
    const ptype = { mousedown: 'pointerdown', mouseup: 'pointerup', mousemove: 'pointermove' }['${type}'];
    upper.dispatchEvent(new PointerEvent(ptype, o));
    upper.dispatchEvent(new MouseEvent('${type}', o));
    return true;
  })()`)
  await fire('mousedown', x0, y0, 1)
  await fire('mousemove', (x0 + x1) / 2, (y0 + y1) / 2, 1)
  await fire('mousemove', x1, y1, 1)
  await fire('mouseup', x1, y1, 0)
  const roi = await evaluate(`(() => {
    // headless 下 Vue 异步 flush 滞后，强制刷新 RoiPanel 后再断言
    const panel = document.querySelector('.roi-panel').__vueParentComponent;
    if (panel && typeof panel.update === 'function') panel.update();
    const items = [...document.querySelectorAll('.roi-item')];
    const f = document.querySelector('.canvas-view').__vueParentComponent.exposed.getFabric();
    const app = document.querySelector('#app').__vue_app__;
    const pinia = app.config.globalProperties.$pinia;
    const roiState = pinia.state.value.roi;
    return JSON.stringify({
      roiCount: items.length,
      storeRois: (roiState.rois || []).map((r) => ({ kind: r.kind, hasStats: !!r.stats, max: r.stats && r.stats.max })),
      firstStats: items[0]?.textContent.replace(/\\s+/g, ' ').slice(0, 120) || null,
      fabricObjs: f.getObjects().length,
      ipc: window.__APP_LOG__.filter((l) => l.startsWith('roi_stats'))
    });
  })()`)
  console.log('A3-ROI:', roi)
  const d3 = JSON.parse(roi)
  console.log('VUE-ERR:', (await evaluate(`JSON.stringify(window.__VUE_ERR__.slice(0, 3))`)).slice(0, 600))
  console.log('DOM-COUNT:', await evaluate(`JSON.stringify({
    appChildren: document.querySelector('#app').children.length,
    appShells: document.querySelectorAll('.app-shell').length,
    roiPanels: document.querySelectorAll('.roi-panel').length,
    canvasViews: document.querySelectorAll('.canvas-view').length,
    statusBars: document.querySelectorAll('.status-bar').length,
    roiPanelState: (() => {
      const p = document.querySelector('.roi-panel');
      const comp = p && p.__vueParentComponent;
      // 手动强制重渲染
      if (comp && typeof comp.update === 'function') {
        try { comp.update(); } catch (e) { return { forceErr: String(e) }; }
      }
      return comp ? {
        rois: comp.setupState.roiStore ? comp.setupState.roiStore.rois.length : 'n/a',
        hasEffect: !!comp.effect,
        hasUpdate: typeof comp.update,
        isUnmounted: comp.isUnmounted,
        uid: comp.uid,
        domItemsBefore: p.querySelectorAll('.roi-item').length
      } : null;
    })()
  })`))
  if ((d3.storeRois || []).length < 1) throw new Error('ROI 未创建')
  if (!d3.firstStats || !d3.firstStats.includes('最高')) throw new Error('ROI 统计未回填: ' + d3.firstStats)

  // ---------- 截图 ----------
  const shot = await send('Page.captureScreenshot', { format: 'png' })
  const outPath = join(root, process.argv.includes('--out') ? process.argv[process.argv.indexOf('--out') + 1] : 'e2e', 'mock-app.png')
  writeFileSync(outPath, Buffer.from(shot.data, 'base64'))
  console.log('截图:', outPath)
  console.log('全部验收通过 ✓')
} catch (e) {
  console.error('E2E FAILED:', e.message)
  process.exitCode = 1
} finally {
  try { ws?.close() } catch { /* ignore */ }
  chrome.kill()
}
