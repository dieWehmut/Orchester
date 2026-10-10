import { onUnmounted, ref } from 'vue'

interface Recognition {
  lang: string
  continuous: boolean
  interimResults: boolean
  onresult: ((event: { results: ArrayLike<ArrayLike<{ transcript: string }>> }) => void) | null
  onend: (() => void) | null
  onerror: (() => void) | null
  start(): void
  stop(): void
  abort(): void
}
type SpeechWindow = Window & { SpeechRecognition?: new () => Recognition; webkitSpeechRecognition?: new () => Recognition }

/** Start only from a user gesture; unsupported webviews keep the mic disabled. */
export function useVoiceInput(accept: (text: string) => void, language: () => string) {
  const host = typeof window === 'undefined' ? undefined : window as SpeechWindow
  const Factory = host?.SpeechRecognition ?? host?.webkitSpeechRecognition
  const supported = Boolean(Factory)
  const listening = ref(false)
  const failed = ref(false)
  let recognition: Recognition | null = null
  function stop(): void {
    recognition?.abort()
    recognition = null
    listening.value = false
  }
  function toggle(): void {
    if (listening.value) { recognition?.stop(); return }
    if (!Factory) return
    failed.value = false
    recognition = new Factory()
    recognition.lang = language()
    recognition.continuous = false
    recognition.interimResults = false
    recognition.onresult = event => {
      const text = Array.from(event.results).map(result => result[0]?.transcript ?? '').join(' ').trim()
      if (text) accept(text)
    }
    recognition.onend = () => { listening.value = false }
    recognition.onerror = () => { failed.value = true; listening.value = false }
    try { recognition.start(); listening.value = true } catch { failed.value = true }
  }
  onUnmounted(stop)
  return { supported, listening, failed, toggle, stop }
}
