export interface IndicatorState {
  state: 'idle' | 'recording' | 'transcribing' | 'done'
}

export interface VoiceTranscribed {
  text: string
}

export interface VoiceError {
  message: string
}

export interface DownloadProgress {
  phase: string
  progress: number
}

export interface AppConfig {
  voiceEngine: string
  voiceHotkey: string
  voiceCloud: {
    volcAppId: string
    volcAccessToken: string
  }
  downloadProxy: string
  firstRun: boolean
}
