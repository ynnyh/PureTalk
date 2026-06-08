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
})

onUnmounted(() => {
  unlistenProgress?.()
  unlistenDone?.()
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
        <div class="field">
          <input v-model="config.voiceHotkey" type="text" placeholder="CommandOrControl+Shift+Space" />
        </div>
        <p class="hint">当前：<span class="hotkey-preview">{{ hotkeyDisplay }}</span></p>
        <p class="hint">支持的修饰键：Ctrl、Shift、Alt、CommandOrControl</p>
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
  display: flex;
  flex-direction: column;
  height: 100vh;
  background: #1a1a2e;
}

.titlebar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 14px 24px;
  flex-shrink: 0;
  -webkit-app-region: drag;
  background: #1a1a2e;
}

.content {
  flex: 1;
  overflow-y: auto;
  padding: 0 24px 24px;
  max-width: 480px;
  margin: 0 auto;
  width: 100%;
}
.titlebar h1 { font-size: 16px; font-weight: 600; color: #fff; margin: 0; }

.close-btn {
  background: none; border: none; color: #888; cursor: pointer;
  padding: 4px; border-radius: 4px; display: flex; align-items: center;
  -webkit-app-region: no-drag; transition: all 0.15s;
}
.close-btn:hover { background: rgba(255,255,255,0.1); color: #fff; }

.welcome {
  background: linear-gradient(135deg, rgba(108,92,231,0.15), rgba(168,85,247,0.1));
  border: 1px solid rgba(108,92,231,0.3);
  border-radius: 10px; padding: 14px 16px; margin-bottom: 20px;
}
.welcome-title { font-size: 15px; font-weight: 600; color: #e0e0ff; margin-bottom: 4px; }
.welcome-desc { font-size: 13px; color: #a0a0b0; line-height: 1.5; }

.section { margin-bottom: 18px; }
.label {
  display: block; font-size: 12px; font-weight: 500; color: #888;
  margin-bottom: 6px; text-transform: uppercase; letter-spacing: 0.5px;
}

.segment-control { display: flex; background: #2a2a3e; border-radius: 8px; overflow: hidden; }
.segment-control button {
  flex: 1; padding: 8px 16px; border: none; background: transparent;
  color: #a0a0b0; font-size: 14px; cursor: pointer; transition: all 0.2s;
}
.segment-control button.active { background: #6c5ce7; color: #fff; }

.field { margin-bottom: 6px; }
.field input {
  width: 100%; padding: 9px 12px; background: #2a2a3e; border: 1px solid #3a3a4e;
  border-radius: 8px; color: #e0e0e0; font-size: 14px; outline: none; transition: border-color 0.2s;
}
.field input:focus { border-color: #6c5ce7; }
.field input::placeholder { color: #555; }

.proxy-row {
  display: flex; align-items: center; gap: 10px;
}
.proxy-input {
  flex: 1; padding: 9px 12px; background: #2a2a3e; border: 1px solid #3a3a4e;
  border-radius: 8px; color: #e0e0e0; font-size: 14px; outline: none; transition: border-color 0.2s;
}
.proxy-input:focus { border-color: #6c5ce7; }
.proxy-input::placeholder { color: #555; }
.proxy-saved {
  font-size: 12px; color: #22c55e; white-space: nowrap; animation: fadeIn 0.3s ease;
}

.hint { font-size: 12px; color: #666; margin-top: 3px; line-height: 1.4; }
.error { font-size: 12px; color: #f87171; margin-top: 6px; }

.hotkey-preview {
  color: #a0a0ff; font-family: monospace; font-size: 13px;
  background: rgba(108,92,231,0.15); padding: 1px 6px; border-radius: 4px;
}

.status {
  display: flex; align-items: center; justify-content: space-between;
  padding: 9px 12px; background: #2a2a3e; border-radius: 8px;
}
.status.ready { color: #22c55e; }
.link-btn { background: none; border: none; color: #6c5ce7; font-size: 13px; cursor: pointer; text-decoration: underline; }

/* Download panel */
.download-panel {
  background: #2a2a3e; border-radius: 10px; padding: 16px;
}
.dl-header {
  display: flex; justify-content: space-between; align-items: center; margin-bottom: 10px;
}
.dl-label { font-size: 14px; font-weight: 500; color: #e0e0e0; }
.dl-speed { font-size: 12px; color: #6c5ce7; font-family: monospace; }

.dl-bar-track {
  height: 6px; background: #1a1a2e; border-radius: 3px; overflow: hidden;
}
.dl-bar-fill {
  height: 100%; background: linear-gradient(90deg, #6c5ce7, #a855f7);
  border-radius: 3px; transition: width 0.2s ease;
  min-width: 0;
}

.dl-footer {
  display: flex; justify-content: space-between; align-items: center; margin-top: 8px;
}
.dl-size { font-size: 12px; color: #888; font-family: monospace; }
.dl-percent { font-size: 12px; color: #a0a0ff; font-family: monospace; }

.actions { margin-top: 20px; display: flex; justify-content: flex-end; }
.btn {
  padding: 9px 24px; border: none; border-radius: 8px;
  font-size: 14px; font-weight: 500; cursor: pointer; transition: all 0.2s;
}
.btn.primary { background: #6c5ce7; color: #fff; }
.btn.primary:hover { background: #7c6cf7; }
.btn.primary:disabled { opacity: 0.5; cursor: not-allowed; }

.post-save-tip {
  margin-top: 12px; padding: 10px 14px;
  background: rgba(34,197,94,0.1); border: 1px solid rgba(34,197,94,0.25);
  border-radius: 8px; color: #22c55e; font-size: 13px; text-align: center;
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
