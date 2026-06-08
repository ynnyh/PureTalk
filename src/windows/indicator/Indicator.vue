<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow'

const state = ref<'idle' | 'recording' | 'transcribing' | 'done'>('idle')

onMounted(async () => {
  const appWindow = getCurrentWebviewWindow()
  await appWindow.listen<{ state: string }>('indicator:state', (event) => {
    state.value = event.payload.state as any
  })
})
</script>

<template>
  <div class="indicator" :class="state" v-if="state !== 'idle'">
    <div class="dot" :class="state"></div>
    <span class="text">
      <template v-if="state === 'recording'">录音中...</template>
      <template v-else-if="state === 'transcribing'">转写中...</template>
      <template v-else-if="state === 'done'">完成</template>
    </span>
  </div>
</template>

<style scoped>
.indicator {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 20px;
  background: rgba(0, 0, 0, 0.75);
  border-radius: 20px;
  backdrop-filter: blur(10px);
  animation: fadeIn 0.2s ease;
  user-select: none;
  -webkit-user-select: none;
}

.dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  flex-shrink: 0;
}

.dot.recording {
  background: #ef4444;
  animation: pulse 1.2s ease-in-out infinite;
}

.dot.transcribing {
  background: #3b82f6;
  animation: spin 1s linear infinite;
  border-radius: 50%;
  width: 8px;
  height: 8px;
  position: relative;
}

.dot.transcribing::after {
  content: '';
  position: absolute;
  inset: -2px;
  border: 2px solid transparent;
  border-top-color: #3b82f6;
  border-radius: 50%;
}

.dot.done {
  background: #22c55e;
}

.text {
  color: #ffffff;
  font-size: 13px;
  font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif;
  white-space: nowrap;
}

@keyframes pulse {
  0%, 100% { opacity: 1; transform: scale(1); }
  50% { opacity: 0.5; transform: scale(1.3); }
}

@keyframes spin {
  from { transform: rotate(0deg); }
  to { transform: rotate(360deg); }
}

@keyframes fadeIn {
  from { opacity: 0; transform: translateY(8px); }
  to { opacity: 1; transform: translateY(0); }
}
</style>
