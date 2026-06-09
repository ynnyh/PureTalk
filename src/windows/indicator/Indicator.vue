<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow'

const state = ref<'idle' | 'recording' | 'transcribing' | 'done'>('idle')
const streamingText = ref('')

onMounted(async () => {
  const appWindow = getCurrentWebviewWindow()
  await appWindow.listen<{ state: string }>('indicator:state', (event) => {
    state.value = event.payload.state as any
    if (state.value !== 'recording') {
      streamingText.value = '' // Clear preview when not recording
    }
  })

  // Listen for streaming text updates
  await appWindow.listen<{ text: string }>('voice:streaming-text', (event) => {
    streamingText.value = event.payload.text
  })
})
</script>

<template>
  <div class="indicator" :class="state" v-if="state !== 'idle'">
    <div class="wave" v-if="state === 'recording' || state === 'transcribing'" aria-hidden="true">
      <i></i><i></i><i></i><i></i><i></i><i></i><i></i>
    </div>
    <svg
      class="check"
      v-else-if="state === 'done'"
      width="16" height="16" viewBox="0 0 24 24"
      fill="none" stroke="currentColor" stroke-width="3"
      stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"
    >
      <polyline points="20 6 9 17 4 12" />
    </svg>
    <span class="text">
      <template v-if="state === 'recording'">
        {{ streamingText || '录音中' }}
      </template>
      <template v-else-if="state === 'transcribing'">转写中…</template>
      <template v-else-if="state === 'done'">完成</template>
    </span>
  </div>
</template>

<style scoped>
.indicator {
  display: inline-flex;
  align-items: center;
  gap: 9px;
  padding: 10px 16px;
  max-width: 340px;
  background: rgba(13, 13, 20, 0.82);
  border: 1px solid rgba(139, 123, 242, 0.28);
  border-radius: 12px;
  box-shadow: 0 8px 28px -8px rgba(0, 0, 0, 0.7);
  backdrop-filter: blur(12px);
  animation: fadeIn 0.2s ease;
  user-select: none;
  -webkit-user-select: none;
}
.indicator.done { border-color: rgba(52, 211, 153, 0.4); }

.wave { display: inline-flex; align-items: center; gap: 3px; height: 18px; flex-shrink: 0; }
.wave i {
  width: 3px;
  height: 100%;
  border-radius: 2px;
  background: linear-gradient(180deg, #a99bff, #8b7bf2);
  transform: scaleY(0.3);
  transform-origin: center;
}
.recording .wave i { animation: iwave 0.9s ease-in-out infinite; }
.transcribing .wave i { animation: iwave 1.5s ease-in-out infinite; opacity: 0.6; }
.wave i:nth-child(2) { animation-delay: 0.10s; }
.wave i:nth-child(3) { animation-delay: 0.20s; }
.wave i:nth-child(4) { animation-delay: 0.30s; }
.wave i:nth-child(5) { animation-delay: 0.20s; }
.wave i:nth-child(6) { animation-delay: 0.10s; }
.wave i:nth-child(7) { animation-delay: 0.05s; }
@keyframes iwave {
  0%, 100% { transform: scaleY(0.3); }
  50% { transform: scaleY(1); }
}

.check { color: #34d399; flex-shrink: 0; }

.text {
  color: #ececf4;
  font-size: 13px;
  font-weight: 500;
  letter-spacing: 0.2px;
  line-height: 1.4;
  font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', 'PingFang SC', 'Microsoft YaHei', sans-serif;
  overflow: hidden;
  text-overflow: ellipsis;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  word-break: break-word;
}
.done .text { color: #34d399; }

@keyframes fadeIn {
  from { opacity: 0; transform: translateY(8px); }
  to { opacity: 1; transform: translateY(0); }
}

@media (prefers-reduced-motion: reduce) {
  .indicator { animation: none; }
  .wave i { animation: none !important; transform: scaleY(0.6); }
}
</style>
