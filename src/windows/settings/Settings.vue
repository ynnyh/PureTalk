<script setup lang="ts">
import { ref, onMounted, computed, onUnmounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow'

interface Config {
  voiceEngine: string
  voiceHotkey: string
  voiceCloud: { volcAppId: string; volcAccessToken: string }
  downloadProxy: string
  firstRun: boolean
  autostart: boolean
}

interface DownloadState {
  phase: string
  phaseLabel: string
  downloaded: number
  total: number
  speed: number
}

const config = ref<Config>({
  voiceEngine: 'local',
  voiceHotkey: 'CommandOrControl+Shift+Space',
  voiceCloud: { volcAppId: '', volcAccessToken: '' },
  downloadProxy: '',
  firstRun: true,
})

const assetsReady = ref(false)
const downloading = ref(false)
const dl = ref<DownloadState>({ phase: '', phaseLabel: '', downloaded: 0, total: 0, speed: 0 })
const saved = ref(false)
const proxySaved = ref(false)
const showFirstRunTip = ref(false)
const downloadError = ref('')
const recordingHotkey = ref(false)
const hotkeyMsg = ref('')
const hotkeyMsgOk = ref(true)

const updateAvailable = ref(false)
const updateInfo = ref<{ currentVersion: string; latestVersion: string; body: string } | null>(null)
const checkingUpdate = ref(false)
const installingUpdate = ref(false)
const updateMsg = ref('')
const updateMsgOk = ref(true)

let unlistenProgress: (() => void) | null = null
let unlistenDone: (() => void) | null = null

const isLocal = computed(() => config.value.voiceEngine === 'local')
const isCloud = computed(() => config.value.voiceEngine === 'cloud')

const hotkeyDisplay = computed(() => {
  const isMac = navigator.platform.toUpperCase().indexOf('MAC') >= 0
  return config.value.voiceHotkey
    .replace(/CommandOrControl/g, isMac ? '⌘' : 'Ctrl')
    .replace(/Shift/g, isMac ? '⇧' : 'Shift')
    .replace(/Alt/g, isMac ? '⌥' : 'Alt')
    .replace(/\+/g, ' + ')
})

const dlPercent = computed(() => {
  if (dl.value.total <= 0) return 0
  return Math.min(100, Math.round((dl.value.downloaded / dl.value.total) * 100))
})

const dlSpeedText = computed(() => {
  const s = dl.value.speed
  if (s <= 0) return ''
  if (s < 1024) return `${s} B/s`
  if (s < 1024 * 1024) return `${(s / 1024).toFixed(1)} KB/s`
  return `${(s / (1024 * 1024)).toFixed(1)} MB/s`
})

const dlSizeText = computed(() => {
  const fmt = (n: number) => {
    if (n <= 0) return '0'
    if (n < 1024) return `${n} B`
    if (n < 1024 * 1024) return `${(n / 1024).toFixed(0)} KB`
    if (n < 1024 * 1024 * 1024) return `${(n / (1024 * 1024)).toFixed(1)} MB`
    return `${(n / (1024 * 1024 * 1024)).toFixed(2)} GB`
  }
  if (dl.value.total <= 0) return fmt(dl.value.downloaded)
  return `${fmt(dl.value.downloaded)} / ${fmt(dl.value.total)}`
})

onMounted(async () => {
  try {
    config.value = await invoke('config_load')
  } catch (e) {
    console.error('加载配置失败:', e)
  }
  try {
    const status: any = await invoke('voice_assets_status')
    assetsReady.value = status.ready
  } catch {}

  // Load actual autostart status from system
  try {
    const enabled = await invoke('autostart_is_enabled')
    config.value.autostart = enabled as boolean
  } catch (e) {
    console.error('检查自启状态失败:', e)
  }

  if (config.value.firstRun) {
    showFirstRunTip.value = true
  }

  // Listen for download progress
  const appWindow = getCurrentWebviewWindow()
  unlistenProgress = await appWindow.listen<DownloadState>('voice:download-progress', (event) => {
    dl.value = event.payload
  })
  unlistenDone = await appWindow.listen('voice:download-done', () => {
    downloading.value = false
    assetsReady.value = true
    dl.value = { phase: '', phaseLabel: '', downloaded: 0, total: 0, speed: 0 }
  })

  // Listen for update available event (from startup check)
  await appWindow.listen<any>('update:available', (event) => {
    updateAvailable.value = true
    updateInfo.value = {
      currentVersion: event.payload.currentVersion,
      latestVersion: event.payload.latestVersion,
      body: event.payload.body,
    }
  })
})

onUnmounted(() => {
  unlistenProgress?.()
  unlistenDone?.()
  window.removeEventListener('keydown', onHotkeyKeydown, true)
})

async function save() {
  try {
    const wasFirstRun = config.value.firstRun
    config.value.firstRun = false
    await invoke('config_save', { config: config.value })
    await invoke('voice_hotkey_sync')
    saved.value = true
    setTimeout(() => { saved.value = false }, 2000)
    if (wasFirstRun) {
      showFirstRunTip.value = false
    }
  } catch (e) {
    alert('保存失败: ' + e)
  }
}

async function autoSaveProxy() {
  try {
    config.value.firstRun = false
    await invoke('config_save', { config: config.value })
    proxySaved.value = true
    setTimeout(() => { proxySaved.value = false }, 2000)
  } catch {}
}

async function onAutostartChange() {
  try {
    if (config.value.autostart) {
      await invoke('autostart_enable')
    } else {
      await invoke('autostart_disable')
    }
  } catch (e: any) {
    alert('设置开机自启失败: ' + e)
    // Revert on error
    config.value.autostart = !config.value.autostart
  }
}

async function checkForUpdates() {
  checkingUpdate.value = true
  updateMsg.value = ''
  try {
    const result: any = await invoke('check_update')
    if (result.available) {
      updateAvailable.value = true
      updateInfo.value = {
        currentVersion: result.currentVersion,
        latestVersion: result.latestVersion,
        body: result.body,
      }
      showUpdateToast('发现新版本!', true)
    } else {
      showUpdateToast('当前已是最新版本 ✓', true)
    }
  } catch (e: any) {
    showUpdateToast(String(e), false)
  } finally {
    checkingUpdate.value = false
  }
}

function showUpdateToast(msg: string, ok: boolean) {
  updateMsgOk.value = ok
  updateMsg.value = msg
  setTimeout(() => { updateMsg.value = '' }, 4000)
}

async function installUpdate() {
  installingUpdate.value = true
  try {
    await invoke('install_update')
  } catch (e: any) {
    showUpdateToast('安装更新失败: ' + e, false)
    installingUpdate.value = false
  }
}

async function downloadAssets() {
  downloading.value = true
  downloadError.value = ''
  dl.value = { phase: '', phaseLabel: '', downloaded: 0, total: 0, speed: 0 }
  try {
    // Auto-save config so proxy takes effect
    config.value.firstRun = false
    await invoke('config_save', { config: config.value })
    await invoke('voice_download_assets', { proxy: config.value.downloadProxy || null })
    assetsReady.value = true
  } catch (e: any) {
    downloadError.value = String(e)
  } finally {
    downloading.value = false
  }
}

function eventToAccelerator(e: KeyboardEvent): string | null {
  const mods: string[] = []
  if (e.ctrlKey) mods.push('CommandOrControl')
  if (e.altKey) mods.push('Alt')
  if (e.shiftKey) mods.push('Shift')
  if (e.metaKey) mods.push('Super')
  const code = e.code
  let key = ''
  if (/^Key[A-Z]$/.test(code)) key = code.slice(3)
  else if (/^Digit[0-9]$/.test(code)) key = code.slice(5)
  else if (/^F([1-9]|1[0-9]|2[0-4])$/.test(code)) key = code
  else if (code === 'Space') key = 'Space'
  else if (code === 'ArrowUp') key = 'Up'
  else if (code === 'ArrowDown') key = 'Down'
  else if (code === 'ArrowLeft') key = 'Left'
  else if (code === 'ArrowRight') key = 'Right'
  else if (code === 'Enter') key = 'Enter'
  else return null
  // Require at least one strong modifier (Ctrl/Alt/Super) for a safe global shortcut
  if (!e.ctrlKey && !e.altKey && !e.metaKey) return null
  return [...mods, key].join('+')
}

function onHotkeyKeydown(e: KeyboardEvent) {
  e.preventDefault()
  e.stopPropagation()
  if (e.key === 'Escape') { stopRecordHotkey(); return }
  const accel = eventToAccelerator(e)
  if (!accel) return // keep waiting until a modifier + key combo is pressed
  stopRecordHotkey()
  applyHotkey(accel)
}

function startRecordHotkey() {
  recordingHotkey.value = true
  hotkeyMsg.value = ''
  ;(document.activeElement as HTMLElement | null)?.blur()
  window.addEventListener('keydown', onHotkeyKeydown, true)
}

function stopRecordHotkey() {
  if (!recordingHotkey.value) return
  recordingHotkey.value = false
  window.removeEventListener('keydown', onHotkeyKeydown, true)
}

async function applyHotkey(accel: string) {
  const prev = config.value.voiceHotkey
  config.value.voiceHotkey = accel
  try {
    config.value.firstRun = false
    await invoke('config_save', { config: config.value })
    await invoke('voice_hotkey_sync')
    hotkeyMsgOk.value = true
    hotkeyMsg.value = `快捷键已更新为 ${hotkeyDisplay.value} ✓`
    setTimeout(() => { hotkeyMsg.value = '' }, 2500)
  } catch (e) {
    config.value.voiceHotkey = prev
    hotkeyMsgOk.value = false
    hotkeyMsg.value = `设置失败：${e}（可能与其它程序冲突，换一个组合）`
    setTimeout(() => { hotkeyMsg.value = '' }, 4000)
  }
}

function openDir() {
  invoke('voice_open_dir')
}

function closeWindow() {
  getCurrentWebviewWindow().hide()
}
</script>

<template>
  <div class="settings">
    <div class="titlebar" data-tauri-drag-region>
      <h1 data-tauri-drag-region>清语</h1>
      <button class="close-btn" @click="closeWindow">
        <svg width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="1.5"><line x1="1" y1="1" x2="13" y2="13"/><line x1="13" y1="1" x2="1" y2="13"/></svg>
      </button>
    </div>

    <div class="content">
      <!-- Update notification banner -->
      <div v-if="updateAvailable" class="update-banner">
        <div class="update-banner-header">
          <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
            <path d="M12 2v12m0 0l-4-4m4 4l4-4M5 21h14" />
          </svg>
          <span class="update-banner-title">发现新版本 v{{ updateInfo?.latestVersion }}</span>
        </div>
        <p class="update-banner-body" v-if="updateInfo?.body">{{ updateInfo.body }}</p>
        <div class="update-banner-actions">
          <button class="btn primary" @click="installUpdate" :disabled="installingUpdate">
            {{ installingUpdate ? '下载中...' : '立即更新' }}
          </button>
          <button class="btn secondary" @click="updateAvailable = false">稍后</button>
        </div>
      </div>

      <!-- Update check toast -->
      <div v-if="updateMsg" class="toast" :class="{ 'toast-ok': updateMsgOk, 'toast-err': !updateMsgOk }">
        {{ updateMsg }}
      </div>

      <!-- First-run welcome -->
      <div v-if="showFirstRunTip" class="welcome">
        <p class="welcome-title">欢迎使用清语</p>
        <p class="welcome-desc">请先完成以下设置，之后可右键系统托盘图标 → 设置 随时修改。</p>
      </div>

      <!-- Engine Selection -->
      <section class="section">
        <label class="label">转写引擎</label>
        <div class="segment-control">
          <button :class="{ active: isLocal }" @click="config.voiceEngine = 'local'">本地</button>
          <button :class="{ active: isCloud }" @click="config.voiceEngine = 'cloud'">云端</button>
        </div>
      </section>

      <!-- Cloud Engine -->
      <section class="section" v-if="isCloud">
        <label class="label">火山引擎凭证</label>
        <div class="field">
          <input v-model="config.voiceCloud.volcAppId" type="text" placeholder="App ID" />
        </div>
        <div class="field">
          <input v-model="config.voiceCloud.volcAccessToken" type="password" placeholder="Access Token" />
        </div>
      </section>

      <!-- Hotkey -->
      <section class="section">
        <label class="label">快捷键</label>
        <button
          type="button"
          class="hotkey-btn"
          :class="{ recording: recordingHotkey }"
          @click="recordingHotkey ? stopRecordHotkey() : startRecordHotkey()"
        >
          <span v-if="recordingHotkey" class="hotkey-rec">请按下快捷键组合…（Esc 取消）</span>
          <span v-else class="hotkey-current">{{ hotkeyDisplay }}</span>
          <span v-if="!recordingHotkey" class="hotkey-action">点击修改</span>
        </button>
        <p class="hint">点击上方按钮,再按下你想要的组合(需含 Ctrl / Alt 等修饰键)。</p>
        <p class="hotkey-msg" :class="{ err: !hotkeyMsgOk }" v-if="hotkeyMsg">{{ hotkeyMsg }}</p>
      </section>

      <!-- Proxy (for downloads) -->
      <section class="section" v-if="isLocal">
        <label class="label">下载代理（可选）</label>
        <div class="proxy-row">
          <input
            v-model="config.downloadProxy"
            type="text"
            placeholder="http://127.0.0.1:7890"
            @blur="autoSaveProxy"
            class="proxy-input"
          />
          <span class="proxy-saved" v-if="proxySaved">已保存</span>
        </div>
        <p class="hint">设置后自动保存，下载时生效</p>
      </section>

      <!-- Local Engine -->
      <section class="section" v-if="isLocal">
        <label class="label">本地语音资产</label>
        <div v-if="assetsReady && !downloading" class="status ready">
          <span>已就绪</span>
          <button class="link-btn" @click="openDir">打开目录</button>
        </div>
        <div v-else>
          <!-- Download progress -->
          <div v-if="downloading" class="download-panel">
            <div class="dl-header">
              <span class="dl-label">{{ dl.phaseLabel || '准备中...' }}</span>
              <span class="dl-speed" v-if="dlSpeedText">{{ dlSpeedText }}</span>
            </div>
            <div class="dl-bar-track">
              <div class="dl-bar-fill" :style="{ width: dlPercent + '%' }"></div>
            </div>
            <div class="dl-footer">
              <span class="dl-size">{{ dlSizeText }}</span>
              <span class="dl-percent" v-if="dl.total > 0">{{ dlPercent }}%</span>
            </div>
          </div>
          <div v-else>
            <p class="hint">首次使用需下载语音引擎和模型（约 250MB）</p>
            <button class="btn primary" @click="downloadAssets">
              下载语音资产
            </button>
            <p class="error" v-if="downloadError">{{ downloadError }}</p>
          </div>
        </div>
      </section>

      <!-- General Settings -->
      <section class="section">
        <label class="label">通用</label>
        <div class="checkbox-row">
          <label class="checkbox-label">
            <input type="checkbox" v-model="config.autostart" @change="onAutostartChange" />
            <span>开机自启</span>
          </label>
        </div>
        <div style="margin-top: 16px;">
          <button class="btn secondary" @click="checkForUpdates" :disabled="checkingUpdate">
            {{ checkingUpdate ? '检查中...' : '检查更新' }}
          </button>
        </div>
      </section>

      <!-- Save -->
      <div class="actions">
        <button class="btn primary" @click="save">
          {{ saved ? '已保存 ✓' : '保存设置' }}
        </button>
      </div>

      <div v-if="saved && !showFirstRunTip" class="post-save-tip">
        设置已保存。后续可右键系统托盘图标 → 设置 进行修改。
      </div>
    </div>
  </div>
</template>

<style scoped>
.settings {
  --bg: #0b0b14;
  --bg2: #101019;
  --card: #15152a;
  --raise: #1b1b30;
  --line: #26263c;
  --hair: rgba(255, 255, 255, 0.07);
  --txt: #ececf4;
  --dim: #9a9ab6;
  --muted: #62627e;
  --accent: #8b7bf2;
  --accent-2: #a99bff;
  --accent-soft: rgba(139, 123, 242, 0.14);
  --ok: #34d399;
  --mono: "Cascadia Code", "SF Mono", ui-monospace, "Consolas", monospace;
  display: flex;
  flex-direction: column;
  height: 100vh;
  background: var(--bg);
  color: var(--txt);
  font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", "PingFang SC", "Microsoft YaHei", "Noto Sans SC", system-ui, sans-serif;
  -webkit-font-smoothing: antialiased;
}

.titlebar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 14px 22px;
  flex-shrink: 0;
  -webkit-app-region: drag;
  background: var(--bg);
  border-bottom: 1px solid var(--hair);
}

.content {
  flex: 1;
  overflow-y: auto;
  padding: 22px 24px 28px;
  max-width: 480px;
  margin: 0 auto;
  width: 100%;
}
.titlebar h1 { font-size: 15px; font-weight: 600; color: var(--txt); margin: 0; letter-spacing: 0.3px; }

.close-btn {
  background: none; border: none; color: var(--muted); cursor: pointer;
  padding: 5px; border-radius: 7px; display: flex; align-items: center;
  -webkit-app-region: no-drag; transition: all 0.15s;
}
.close-btn:hover { background: rgba(255, 255, 255, 0.08); color: var(--txt); }

.welcome {
  background: linear-gradient(135deg, rgba(139, 123, 242, 0.16), rgba(168, 85, 247, 0.08));
  border: 1px solid rgba(139, 123, 242, 0.3);
  border-radius: 12px; padding: 14px 16px; margin-bottom: 22px;
}
.welcome-title { font-size: 14.5px; font-weight: 600; color: #e6e2ff; margin-bottom: 4px; }
.welcome-desc { font-size: 12.5px; color: var(--dim); line-height: 1.5; }

.update-banner {
  margin-bottom: 20px;
  padding: 16px 18px;
  background: linear-gradient(135deg, rgba(139, 123, 242, 0.12), rgba(109, 94, 240, 0.08));
  border: 1px solid rgba(139, 123, 242, 0.3);
  border-radius: 12px;
  animation: fadeIn 0.3s ease;
}
.update-banner-header {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-bottom: 8px;
}
.update-banner-header svg {
  color: var(--accent-2);
  flex-shrink: 0;
}
.update-banner-title {
  font-size: 15px;
  font-weight: 600;
  color: var(--accent-2);
}
.update-banner-body {
  margin: 8px 0 12px 30px;
  font-size: 13px;
  color: var(--dim);
  line-height: 1.5;
  white-space: pre-line;
}
.update-banner-actions {
  display: flex;
  gap: 10px;
  margin-left: 30px;
}
.update-banner-actions .btn {
  padding: 7px 16px;
  font-size: 13px;
}

.toast {
  padding: 10px 16px; border-radius: 8px; font-size: 13px; margin-bottom: 14px;
  animation: fadeIn 0.2s ease;
}
.toast-ok { background: rgba(139,123,242,0.12); color: var(--accent); border: 1px solid rgba(139,123,242,0.25); }
.toast-err { background: rgba(242,80,80,0.12); color: #f25050; border: 1px solid rgba(242,80,80,0.25); }
@keyframes fadeIn { from { opacity: 0; transform: translateY(-6px); } to { opacity: 1; transform: translateY(0); } }

.section { margin-bottom: 18px; }
.label {
  display: block; font-size: 11px; font-weight: 600; color: var(--muted);
  margin-bottom: 8px; text-transform: uppercase; letter-spacing: 0.5px;
}

.segment-control { display: flex; background: var(--card); border: 1px solid var(--line); border-radius: 9px; padding: 3px; gap: 2px; }
.segment-control button {
  flex: 1; padding: 8px 16px; border: none; background: transparent; border-radius: 7px;
  color: var(--dim); font-size: 13.5px; cursor: pointer; transition: all 0.18s;
}
.segment-control button.active { background: linear-gradient(135deg, var(--accent), #6d5ef0); color: #fff; }

.field { margin-bottom: 8px; }
.field input {
  width: 100%; padding: 10px 12px; background: var(--card); border: 1px solid var(--line);
  border-radius: 9px; color: var(--txt); font-size: 13.5px; outline: none; transition: border-color 0.18s;
}
.field input:focus { border-color: var(--accent); }
.field input::placeholder { color: var(--muted); }

.proxy-row {
  display: flex; align-items: center; gap: 10px;
}
.proxy-input {
  flex: 1; padding: 10px 12px; background: var(--card); border: 1px solid var(--line);
  border-radius: 9px; color: var(--txt); font-size: 13.5px; outline: none; transition: border-color 0.18s;
}
.proxy-input:focus { border-color: var(--accent); }
.proxy-input::placeholder { color: var(--muted); }
.proxy-saved {
  font-size: 12px; color: var(--ok); white-space: nowrap; animation: fadeIn 0.3s ease;
}

.hint { font-size: 12px; color: var(--muted); margin-top: 8px; line-height: 1.5; }
.error { font-size: 12px; color: #f87171; margin-top: 8px; }

.hotkey-preview {
  color: var(--accent-2); font-family: var(--mono); font-size: 12px;
  background: var(--accent-soft); padding: 2px 7px; border-radius: 5px;
}

.hotkey-btn {
  width: 100%; display: flex; align-items: center; justify-content: space-between; gap: 10px;
  padding: 11px 14px; background: var(--card); border: 1px solid var(--line);
  border-radius: 9px; color: var(--txt); font: inherit; cursor: pointer; transition: all 0.18s;
}
.hotkey-btn:hover { border-color: var(--accent); }
.hotkey-btn.recording { border-color: var(--accent); box-shadow: 0 0 0 3px var(--accent-soft); }
.hotkey-current { font-family: var(--mono); font-size: 13.5px; color: var(--accent-2); letter-spacing: 0.5px; }
.hotkey-rec { font-size: 13px; color: var(--accent-2); }
.hotkey-action { font-size: 12px; color: var(--muted); }
.hotkey-msg { font-size: 12.5px; color: var(--ok); margin-top: 8px; animation: fadeIn 0.3s ease; }
.hotkey-msg.err { color: #f87171; }

.status {
  display: flex; align-items: center; justify-content: space-between;
  padding: 10px 14px; background: var(--card); border: 1px solid var(--line); border-radius: 9px;
}
.status.ready { color: var(--ok); }
.link-btn { background: none; border: none; color: var(--accent-2); font-size: 12.5px; cursor: pointer; }
.link-btn:hover { text-decoration: underline; }

/* Download panel */
.download-panel {
  background: var(--card); border: 1px solid var(--line); border-radius: 11px; padding: 15px;
}
.dl-header {
  display: flex; justify-content: space-between; align-items: center; margin-bottom: 10px;
}
.dl-label { font-size: 13.5px; font-weight: 500; color: var(--txt); }
.dl-speed { font-size: 12px; color: var(--accent-2); font-family: var(--mono); }

.dl-bar-track {
  height: 6px; background: var(--bg); border-radius: 4px; overflow: hidden;
}
.dl-bar-fill {
  height: 100%; background: linear-gradient(90deg, var(--accent), #a855f7);
  border-radius: 4px; transition: width 0.2s ease;
  min-width: 0;
}

.dl-footer {
  display: flex; justify-content: space-between; align-items: center; margin-top: 8px;
}
.dl-size { font-size: 11.5px; color: var(--muted); font-family: var(--mono); }
.dl-percent { font-size: 11.5px; color: var(--accent-2); font-family: var(--mono); }

.actions { margin-top: 24px; display: flex; justify-content: flex-end; }

.checkbox-row {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-top: 12px;
}

.checkbox-label {
  display: flex;
  align-items: center;
  gap: 8px;
  cursor: pointer;
  user-select: none;
  color: var(--txt);
  font-size: 14px;
}

.checkbox-label input[type="checkbox"] {
  width: 18px;
  height: 18px;
  cursor: pointer;
  accent-color: var(--accent);
}

.checkbox-label:hover {
  color: var(--accent-2);
}

.btn {
  padding: 10px 22px; border: none; border-radius: 9px;
  font-size: 13.5px; font-weight: 600; cursor: pointer; transition: all 0.18s;
}
.btn.primary { background: linear-gradient(135deg, var(--accent), #6d5ef0); color: #fff; }
.btn.primary:hover { transform: translateY(-1px); box-shadow: 0 8px 24px -8px var(--accent-soft); }
.btn.primary:disabled { opacity: 0.5; cursor: not-allowed; transform: none; }

.btn.secondary {
  background: var(--card); color: var(--accent-2);
  border: 1px solid var(--line);
}
.btn.secondary:hover {
  border-color: var(--accent);
  background: var(--raise);
  transform: translateY(-1px);
}

.post-save-tip {
  margin-top: 12px; padding: 10px 14px;
  background: rgba(52, 211, 153, 0.1); border: 1px solid rgba(52, 211, 153, 0.26);
  border-radius: 9px; color: var(--ok); font-size: 13px; text-align: center;
  animation: fadeIn 0.3s ease;
}
@keyframes fadeIn {
  from { opacity: 0; transform: translateY(-4px); }
  to { opacity: 1; transform: translateY(0); }
}

</style>

<style>
/* Scrollbar styling for the scrollable content area */
.content::-webkit-scrollbar {
  width: 6px;
}
.content::-webkit-scrollbar-track {
  background: transparent;
}
.content::-webkit-scrollbar-thumb {
  background: rgba(255,255,255,0.15);
  border-radius: 3px;
}
.content::-webkit-scrollbar-thumb:hover {
  background: rgba(255,255,255,0.25);
}
</style>
