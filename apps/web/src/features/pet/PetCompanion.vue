<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'

import {
  animationFrameAt,
  animationFrameDelay,
  createPetAnimations,
  resolveAnimation,
  type PetAnimationName,
} from './pet-animations'
import { frameOffset } from './pet-manifest'
import { petLookFor, type PetLook } from './pet-look'
import { usePetPack } from './use-pet-pack'

/**
 * The ambient companion: one sprite cell from the fixed xiaoxuan pack, advanced
 * frame by frame on the durations the pack declares.
 *
 * Rendering is a CSS background sprite rather than a canvas: the atlas is a
 * static asset, so the browser can decode it once and the frame change is a
 * single property write instead of a per-frame draw call.
 */
const props = withDefaults(
  defineProps<{
    animation?: PetAnimationName
    label?: string
    size?: number
    /** Follow the pointer with the look rows; off for reduced motion. */
    trackPointer?: boolean
    reducedMotion?: boolean
  }>(),
  {
    animation: 'idle',
    label: '',
    size: 96,
    trackPointer: true,
    reducedMotion: false,
  },
)

const pack = usePetPack()
const tracks = computed(() => (pack.value ? createPetAnimations(pack.value.grid) : null))
const active = computed<PetAnimationName>(() => props.animation)
const look = ref<PetLook | null>(null)
const elapsedMs = ref(0)
const root = ref<HTMLElement | null>(null)

const animation = computed(() => {
  if (!tracks.value) return null
  return resolveAnimation(tracks.value, active.value)
})

/**
 * A tracked look holds a single pose, so it suspends the timeline entirely and
 * the companion resumes its animation from the start once the pointer leaves.
 */
const frameIndex = computed(() => {
  const current = animation.value
  if (!current) return null
  if (props.reducedMotion) return current.frames.at(0)?.index ?? null
  if (props.trackPointer && look.value?.frame !== null && look.value?.frame !== undefined) {
    return look.value.frame
  }
  return animationFrameAt(current, elapsedMs.value)?.index ?? null
})

const offset = computed(() => {
  if (!pack.value || frameIndex.value === null) return null
  return frameOffset(pack.value.grid, frameIndex.value)
})

/**
 * The decoration exposes its frame and look index as data attributes so tests
 * and devtools can read the current pose without re-implementing the timeline.
 */
const rootAttributes = computed<Record<string, string | number>>(() => {
  const attributes: Record<string, string | number> = {
    'data-pet-animation': active.value,
  }
  if (frameIndex.value !== null) attributes['data-pet-frame'] = frameIndex.value
  if (look.value?.direction !== null && look.value?.direction !== undefined) {
    attributes['data-pet-look'] = look.value.direction
  }
  if (props.label) {
    attributes.role = 'img'
    attributes['aria-label'] = props.label
  } else {
    attributes.role = 'presentation'
    attributes['aria-hidden'] = 'true'
  }
  return attributes
})

const style = computed(() => {
  if (!pack.value || !offset.value) return {}
  return {
    backgroundImage: `url(${pack.value.spritesheetUrl})`,
    backgroundPosition: `${offset.value.x}% ${offset.value.y}%`,
    backgroundSize: `${pack.value.grid.columns * 100}% ${pack.value.grid.rows * 100}%`,
    inlineSize: `${props.size}px`,
    blockSize: `${props.size}px`,
  }
})

let timer: ReturnType<typeof setTimeout> | null = null
let startedAt = 0

function clearTimer(): void {
  if (timer === null) return
  clearTimeout(timer)
  timer = null
}

/**
 * Schedule exactly the next frame change instead of polling.
 *
 * The tracks are held for seconds at a time, so a fixed 16 ms tick would wake
 * the main thread hundreds of times per visible frame.
 */
function schedule(): void {
  clearTimer()
  const current = animation.value
  if (!current || props.reducedMotion) return
  if (props.trackPointer && look.value !== null) return
  const delay = animationFrameDelay(current, elapsedMs.value)
  if (delay === null) return
  timer = setTimeout(() => {
    timer = null
    elapsedMs.value = Date.now() - startedAt
    schedule()
  }, delay)
}

function restart(): void {
  startedAt = Date.now()
  elapsedMs.value = 0
  schedule()
}

watch(() => active.value, restart)
watch(() => props.reducedMotion, restart)
watch(() => pack.value, restart)

function handlePointerMove(event: PointerEvent): void {
  if (!props.trackPointer || props.reducedMotion || !pack.value) return
  const element = root.value
  if (!element) return
  const bounds = element.getBoundingClientRect()
  const centreX = bounds.left + bounds.width / 2
  const centreY = bounds.top + bounds.height / 2
  const next = petLookFor(pack.value.grid, event.clientX - centreX, event.clientY - centreY)
  const changed = next.direction !== (look.value?.direction ?? null)
  look.value = next.direction === null ? null : next
  if (changed) schedule()
}

onMounted(() => {
  restart()
  if (typeof window !== 'undefined') {
    window.addEventListener('pointermove', handlePointerMove)
  }
})

onUnmounted(() => {
  clearTimer()
  if (typeof window !== 'undefined') {
    window.removeEventListener('pointermove', handlePointerMove)
  }
})
</script>

<template>
  <div ref="root" v-bind="rootAttributes" class="pet-companion" data-pet-companion>
    <span class="pet-companion__sprite" data-pet-sprite :style="style" aria-hidden="true" />
  </div>
</template>

<style scoped>
.pet-companion {
  display: grid;
  place-items: center;
  pointer-events: none;
}

.pet-companion__sprite {
  display: block;
  background-repeat: no-repeat;
  image-rendering: auto;
}

@media (prefers-reduced-motion: reduce) {
  .pet-companion__sprite {
    transition: none;
  }
}

@keyframes pet-companion-in {
  from {
    opacity: 0;
    transform: translateY(6px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}

@media (prefers-reduced-motion: no-preference) {
  .pet-companion {
    animation: pet-companion-in 320ms ease-out both;
  }
}
</style>
