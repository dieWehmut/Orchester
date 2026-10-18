import { onMounted, onUnmounted, ref, watch, type Ref } from 'vue'

/** One clock for the visible conversation; never manufacture source timestamps. */
export function useActivityClock(active: Ref<boolean>) {
  const now = ref(Date.now())
  let timer: ReturnType<typeof setInterval> | undefined
  function sync(): void {
    clearInterval(timer)
    timer = undefined
    if (!active.value || document.visibilityState === 'hidden') return
    now.value = Date.now()
    timer = setInterval(() => { now.value = Date.now() }, 1000)
  }
  watch(active, sync, { immediate: true })
  onMounted(() => document.addEventListener('visibilitychange', sync))
  onUnmounted(() => { clearInterval(timer); document.removeEventListener('visibilitychange', sync) })
  return now
}
