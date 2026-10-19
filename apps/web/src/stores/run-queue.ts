import { computed, ref, shallowRef, watch } from 'vue'
import type { RunView } from '@orchester/ereignis'
import type { RunStore } from './run'

export interface QueuedPrompt { id: string; prompt: string; status: 'waiting' | 'sending' | 'failed' }

/** Acknowledged drafts live here, independently of the mounted composer. */
export function createRunQueue(run: RunStore) {
  const items = ref<QueuedPrompt[]>([])
  const paused = ref(false)
  const history = shallowRef<readonly RunView[]>([])
  const view = computed<RunView>(() => ({ ...run.view.value, timeline: [...history.value.flatMap(part => part.timeline), ...run.view.value.timeline] }))
  let sending = false
  let disposed = false
  let serial = 0
  const busy = () => ['submitting', 'running', 'cancelling'].includes(run.lifecycle.value)

  async function drain(): Promise<void> {
    if (disposed || sending || paused.value || busy() || !items.value.length) return
    const item = items.value[0]!
    const previous = run.view.value
    sending = true
    item.status = 'sending'
    try {
      const response = await run.submit(item.prompt)
      if (disposed) return
      if (response) {
        if (previous.runId && previous.timeline.length && !history.value.some(part => part.runId === previous.runId)) history.value = [...history.value, previous]
        items.value = items.value.filter(value => value.id !== item.id)
      } else {
        item.status = 'failed'
        paused.value = true
      }
    } finally {
      sending = false
      if (!disposed && !paused.value && run.lifecycle.value === 'completed') queueMicrotask(() => { void drain() })
    }
  }

  const unwatch = watch(run.lifecycle, value => {
    if (['failed', 'cancelled', 'paused', 'cancelling'].includes(value)) paused.value = true
    else if (value === 'completed') void drain()
  }, { flush: 'post' })

  function enqueue(prompt: string): boolean {
    if (disposed || !prompt.trim()) return false
    if (!items.value.length && !busy()) paused.value = false
    items.value.push({ id: `queued-${Date.now()}-${++serial}`, prompt, status: 'waiting' })
    void drain()
    return true
  }
  function remove(id: string): string | null {
    const item = items.value.find(item => item.id === id && item.status !== 'sending')
    if (!item) return null
    items.value = items.value.filter(item => item.id !== id)
    return item.prompt
  }
  function resume(): void { paused.value = false; void drain() }
  function dispose(): void { disposed = true; unwatch() }
  return { items, paused, view, enqueue, remove, resume, dispose }
}
