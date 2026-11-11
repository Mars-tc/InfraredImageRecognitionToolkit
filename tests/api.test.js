import test, { beforeEach, after } from 'node:test'
import assert from 'node:assert/strict'
import { installMock } from '../scripts/fixtures/tauri-ipc-mock.js'

const mock = installMock()
const api = await import('../src/api/tauri.js')

beforeEach(() => mock.reset())
after(() => mock.uninstall())

/** f32 小端矩阵 → base64 */
function toB64(values) {
  const bytes = new Uint8Array(new Float32Array(values).buffer)
  let bin = ''
  for (const b of bytes) bin += String.fromCharCode(b)
  return btoa(bin)
}

test('base64ToFloat32 按小端解码', () => {
  assert.equal(api.base64ToFloat32('').length, 0)
  const src = [1.5, -2.25, 0, 100.125]
  const out = api.base64ToFloat32(toB64(src))
  assert.ok(out instanceof Float32Array)
  assert.deepEqual([...out], src)
  // 合法 base64 但字节数不是 4 的倍数：必须显式报错而非静默截断
  assert.throws(
    () => api.base64ToFloat32(btoa('\x00\x00\x00\x00\x00')),
    /温度矩阵字节数异常/
  )
})

test('IPC 封装的命令名与参数映射（对照 main.rs）', async () => {
  const cases = [
    [() => api.loadImage('/tmp/a.jpg'), 'load_image', { path: '/tmp/a.jpg', distance: null }],
    [() => api.setEmissivity(0.9), 'set_emissivity', { value: 0.9 }],
    [() => api.setDistance(5), 'set_distance', { value: 5 }],
    [() => api.roiStats({ kind: 'rect' }), 'roi_stats', { shape: { kind: 'rect' } }],
    [() => api.phaseCompare([['A', 50]]), 'phase_compare', { entries: [['A', 50]] }],
    [() => api.batchParse('/tmp/dir'), 'batch_parse', { dir: '/tmp/dir', recursive: true }],
    [() => api.exportCsv('/tmp/a.csv'), 'export_csv', { path: '/tmp/a.csv' }],
    [() => api.exportBatchCsv([], '/tmp/b.csv'), 'export_batch_csv', { rows: [], path: '/tmp/b.csv' }],
    [() => api.exportAnnotated('/tmp/a.png'), 'export_annotated', { path: '/tmp/a.png', rois: [] }],
    [() => api.ledgerAdd({ tower_no: 'T1' }), 'ledger_add', { input: { tower_no: 'T1' } }],
    [() => api.ledgerQuery(), 'ledger_query', { keyword: '', level: null, limit: 200, offset: 0 }],
    [() => api.ledgerUpdate(3, { tower_no: 'T2' }), 'ledger_update', { id: 3, input: { tower_no: 'T2' } }],
    [() => api.ledgerDelete(3), 'ledger_delete', { id: 3 }],
    [
      () => api.exportReport([1], '/tmp/r.docx', '张三', '2026-11-04', '班组'),
      'export_report',
      { ids: [1], path: '/tmp/r.docx', inspector: '张三', date: '2026-11-04', org: '班组' }
    ]
  ]

  // getTemps 需要合法的 base64 响应才能完成解码
  mock.setHandler(() => toB64([]))

  for (const [call, cmd, args] of cases) {
    const before = mock.calls.length
    await call()
    const msg = mock.calls[before]
    assert.equal(msg.cmd, cmd, `命令名不匹配: ${cmd}`)
    for (const [k, v] of Object.entries(args)) {
      assert.deepEqual(msg[k], v, `${cmd}.${k}`)
    }
  }

  for (const [call, cmd] of [
    [() => api.getTemps(), 'get_temps'],
    [() => api.getDimensions(), 'get_dimensions'],
    [() => api.getSettings(), 'get_settings'],
    [() => api.ledgerStats(), 'ledger_stats']
  ]) {
    const before = mock.calls.length
    await call()
    assert.equal(mock.calls[before].cmd, cmd)
  }
})

test('setSettings 展开三键而非嵌套 payload', async () => {
  const payload = { joint_temp: 80, delta_t: 12, phase_delta_t: 6 }
  await api.setSettings(payload)
  const msg = mock.last()
  assert.equal(msg.cmd, 'set_settings')
  assert.equal(msg.joint_temp, 80)
  assert.equal(msg.delta_t, 12)
  assert.equal(msg.phase_delta_t, 6)
  assert.equal(msg.payload, undefined, '不应嵌套 payload')
})

test('getTemps 解码后端返回的 base64 温度矩阵', async () => {
  mock.setHandler(() => toB64([29.5, 30.25, 31.0]))
  const temps = await api.getTemps()
  assert.ok(temps instanceof Float32Array)
  assert.deepEqual([...temps], [29.5, 30.25, 31.0])
  assert.equal(mock.last().cmd, 'get_temps')
})

test('invoke 失败时透传后端错误', async () => {
  mock.setHandler(() => {
    throw '不是辐射原图'
  })
  await assert.rejects(() => api.loadImage('/tmp/bad.jpg'), (e) => e === '不是辐射原图')
})

test('文件对话框走 tauri 内部命令', async () => {
  mock.setHandler(() => '/tmp/picked.jpg')
  assert.equal(await api.pickImage(), '/tmp/picked.jpg')
  let inner = mock.last().message
  assert.equal(mock.last().cmd, 'tauri')
  assert.equal(inner.cmd, 'openDialog')
  assert.deepEqual(inner.options.filters[0].extensions, ['jpg', 'jpeg'])

  await api.pickFolder()
  inner = mock.last().message
  assert.equal(inner.cmd, 'openDialog')
  assert.equal(inner.options.directory, true)

  await api.pickSave('a.csv', ['csv'])
  inner = mock.last().message
  assert.equal(inner.cmd, 'saveDialog')
  assert.equal(inner.options.defaultPath, 'a.csv')
})
