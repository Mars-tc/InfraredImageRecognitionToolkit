// 零依赖 Tauri 1 IPC 替身（仅测试用）。
//
// @tauri-apps/api 的 invoke 通过 window.__TAURI_IPC__ 与后端通信，
// 并用 window.crypto + `_<callback>` 属性注册回调。这里在纯 Node 中补齐
// 这几个全局，既能断言前端封装发出的命令/参数，又不需要 mock 框架或改应用源码。

export function installMock() {
  const calls = []
  let handler = () => undefined

  globalThis.window = globalThis
  if (typeof globalThis.crypto?.getRandomValues !== 'function') {
    let seed = 1
    globalThis.crypto = {
      getRandomValues(arr) {
        for (let i = 0; i < arr.length; i++) {
          seed = (seed * 1103515245 + 12345) & 0x7fffffff
          arr[i] = seed
        }
        return arr
      }
    }
  }

  globalThis.__TAURI_IPC__ = (msg) => {
    calls.push(msg)
    Promise.resolve().then(() => {
      const ok = globalThis[`_${msg.callback}`]
      const fail = globalThis[`_${msg.error}`]
      try {
        const result = handler(msg)
        if (typeof ok === 'function') ok(result)
      } catch (e) {
        if (typeof fail === 'function') fail(typeof e === 'string' ? e : String(e))
      }
    })
  }

  return {
    calls,
    setHandler(fn) {
      handler = fn
    },
    reset() {
      calls.length = 0
      handler = () => undefined
    },
    last() {
      return calls[calls.length - 1]
    },
    uninstall() {
      delete globalThis.__TAURI_IPC__
      delete globalThis.window
    }
  }
}
