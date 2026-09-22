<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'

import {
  animationFrameAt,
  animationFrameDelay,
  createPetAnimations,
  createPetWalkTrack,
  petPoseHoldMs,
  resolveAnimation,
  type PetAnimationName,
} from './pet-animations'
import { frameOffset } from './pet-manifest'
import { petLookFor, type PetLook } from './pet-look'
import {
  PET_ROAM_FIRST_PAUSE_MS,
  petNotices,
  petRoamAllowed,
  petRoamPositionAt,
  petWalkAnimation,
  planPetRoamLeg,
  planPetRoamPause,
  type PetRoamLeg,
} from './pet-roam'
import { usePetPack } from './use-pet-pack'

/**
 * The ambient companion: one sprite cell from the fixed xiaoxuan pack, advanced
 * frame by frame on the durations the pack declares.
 *
 * Rendering is a CSS background sprite rather than a canvas: the atlas is a
 * static asset, so the browser can decode it once and the frame change is a
 * single property write instead of a per-frame draw call.
 *
 * The companion also paces the stage its host measured for it, because the
 * reference companion is ambient rather than seated. The sprite cannot know how
 * much room it was given - it is a decoration inside somebody else's layout -
 * so the host measures its band and hands the span down, and the walk stays off
 * until one arrives.
 */
const props = withDefaults(
  defineProps<{
    animation?: PetAnimationName
    label?: string
    size?: number
    /** Follow the pointer with the look rows; off for reduced motion. */
    trackPointer?: boolean
    reducedMotion?: boolean
    /** Whether the companion may pace the stage its host measured. */
    roam?: boolean
    /** How much travel the host measured for it, in CSS pixels. */
    roamSpan?: number
  }>(),
  {
    animation: 'idle',
    label: '',
    size: 96,
    trackPointer: true,
    reducedMotion: false,
    roam: false,
    roamSpan: 0,
  },
)

const pack = usePetPack()
const tracks = computed(() => (pack.value ? createPetAnimations(pack.value.grid) : null))
const active = computed<PetAnimationName>(() => props.animation)
const look = ref<PetLook | null>(null)
const elapsedMs = ref(0)
const root = ref<HTMLElement | null>(null)

/**
 * The leg being walked, and where the companion stands.
 *
 * Position is its own state rather than a CSS-only offset because the sprite
 * has to survive an interruption: when the run asks for a pose mid-step, the
 * walk stops where it had got to instead of finishing or snapping back.
 */
const leg = ref<PetRoamLeg | null>(null)
const offsetX = ref(0)

/** The walk row to draw, or null while the companion is standing still. */
const walking = computed<PetAnimationName | null>(() =>
  leg.value === null ? null : petWalkAnimation(leg.value.direction),
)

/** The pose in force: the walk wins while a leg is under way. */
const pose = computed<PetAnimationName>(() => walking.value ?? active.value)

const animation = computed(() => {
  if (!tracks.value || !pack.value) return null
  // The walk is a looping row, not a state track that hands off after three
  // cycles: a leg is shorter than that, so it needs the step to repeat for as
  // long as the leg lasts.
  if (walking.value !== null) {
    return (
      createPetWalkTrack(pack.value.grid, walking.value as 'running-left' | 'running-right') ??
      resolveAnimation(tracks.value, walking.value)
    )
  }
  return resolveAnimation(tracks.value, active.value)
})

/**
 * A tracked look holds a single pose, so it suspends the timeline entirely and
 * the companion resumes its animation from the start once the pointer leaves.
 */
/**
 * A look is the pointer's, and it outranks both the walk and the timeline: a
 * companion that kept padding along while it was being pointed at would read
 * as ignoring the gesture. So a look stops the walk where it stands and holds
 * one pose until the pointer leaves.
 */
const listening = computed(
  () =>
    props.trackPointer &&
    !props.reducedMotion &&
    look.value?.frame !== null &&
    look.value?.frame !== undefined,
)

const frameIndex = computed(() => {
  const current = animation.value
  if (!current) return null
  if (props.reducedMotion) return current.frames.at(0)?.index ?? null
  if (listening.value) return look.value?.frame ?? null
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
    'data-pet-animation': pose.value,
  }
  if (frameIndex.value !== null) attributes['data-pet-frame'] = frameIndex.value
  if (look.value?.direction !== null && look.value?.direction !== undefined) {
    attributes['data-pet-look'] = look.value.direction
  }
  // What the roam is doing and where it has carried the companion, so tests
  // and devtools can read the walk without re-implementing its clock.
  attributes['data-pet-roam'] = leg.value === null ? 'resting' : 'walking'
  attributes['data-pet-position'] = Math.round(offsetX.value)
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
    transform: `translateX(${Math.round(offsetX.value)}px)`,
    // The glide belongs to the leg, so its duration is the leg's own: a CSS
    // animation here would have to guess the distance the planner chose.
    // While resting it is zero, so an interruption places the companion where
    // its leg had reached instead of sliding there afterwards.
    transitionDuration: `${leg.value?.durationMs ?? 0}ms`,
  }
})

/**
 * Two clocks, because the companion has two things to keep time with: the
 * frame it is holding and the leg it is walking. They run on unrelated
 * durations, so one timer cannot carry both - whichever was scheduled last
 * would cancel the other.
 */
let frameTimer: ReturnType<typeof setTimeout> | null = null
let legTimer: ReturnType<typeof setTimeout> | null = null
let startedAt = 0
let legStartedAt = 0

function clearFrameTimer(): void {
  if (frameTimer === null) return
  clearTimeout(frameTimer)
  frameTimer = null
}

function clearLegTimer(): void {
  if (legTimer === null) return
  clearTimeout(legTimer)
  legTimer = null
}

function clearTimers(): void {
  clearFrameTimer()
  clearLegTimer()
}

/**
 * Whether the companion may walk right now.
 *
 * Three things have to agree: the host has to have opened a stage, the motion
 * preference has to allow it, and the run has to have nothing to say. A stage
 * narrower than one stride is refused by the planner itself rather than here.
 */
const roamReady = computed(
  () => props.roam && !props.reducedMotion && props.roamSpan > 0 && petRoamAllowed(active.value),
)

/**
 * Schedule exactly the next frame change instead of polling.
 *
 * The tracks are held for seconds at a time, so a fixed 16 ms tick would wake
 * the main thread hundreds of times per visible frame.
 */
function schedule(): void {
  clearFrameTimer()
  const current = animation.value
  if (!current || props.reducedMotion) return
  if (listening.value) return
  const delay = animationFrameDelay(current, elapsedMs.value)
  if (delay === null) return
  frameTimer = setTimeout(() => {
    frameTimer = null
    elapsedMs.value = Date.now() - startedAt
    schedule()
  }, delay)
}

/**
 * Start the next leg, from wherever the companion is standing.
 *
 * The leg is held open for exactly its own duration: the sprite glides on a
 * transition of that length while the walk row loops, and the timer that ends
 * it is the same number, so the step and the travel finish together.
 */
function startLeg(): void {
  clearLegTimer()
  if (!roamReady.value) return
  // A look outranks the walk, so the beat stands down while the pointer holds
  // the companion's attention and is picked up again when it leaves.
  if (listening.value) return
  const next = planPetRoamLeg({ span: props.roamSpan }, offsetX.value)
  // A stage with no stride of room yet is not a stage the companion abandons:
  // it waits out a beat and looks again, so widening the window later starts
  // the walk rather than requiring a reload.
  if (next === null) {
    legTimer = setTimeout(startLeg, planPetRoamPause())
    return
  }
  leg.value = next
  offsetX.value = next.to
  legStartedAt = Date.now()
  resetTimeline()
  schedule()
  legTimer = setTimeout(finishLeg, next.durationMs)
}

/** Start the visible track over, so the pose that takes the row begins at its first frame. */
function resetTimeline(): void {
  startedAt = Date.now()
  elapsedMs.value = 0
}

/**
 * End the leg where it was interrupted, or at its end if it ran its course.
 *
 * Interruption is the ordinary case - a run turning busy mid-step - so the
 * companion is left standing at the position the leg had reached rather than
 * either finishing the walk or jumping back to where it started.
 */
function finishLeg(): void {
  const walked = leg.value
  if (walked !== null) offsetX.value = petRoamPositionAt(walked, Date.now() - legStartedAt)
  leg.value = null
  clearLegTimer()
  resetTimeline()
  schedule()
  legTimer = setTimeout(startLeg, planPetRoamPause())
}

/** Restart the timeline; the walk is stopped first because its row changed. */
function restart(): void {
  clearLegTimer()
  if (leg.value !== null) {
    offsetX.value = petRoamPositionAt(leg.value, Date.now() - legStartedAt)
    leg.value = null
  }
  resetTimeline()
  schedule()
  if (roamReady.value) {
    legTimer = setTimeout(startLeg, firstStepDelay())
  }
}

/**
 * How long the companion holds before it starts padding about.
 *
 * A pose the pack carries is a message, and a companion that walked off the
 * moment a run settled would cut its own gesture short. So the wait is the
 * longer of the companion's usual beat and whatever the pose in force still has
 * to play - which is nothing at all when the run is simply resting.
 */
function firstStepDelay(): number {
  return Math.max(PET_ROAM_FIRST_PAUSE_MS, petPoseHoldMs(active.value))
}

watch(() => active.value, restart)
watch(() => props.reducedMotion, restart)
watch(() => pack.value, restart)
watch([() => props.roam, () => props.roamSpan], restart)

function handlePointerMove(event: PointerEvent): void {
  if (!props.trackPointer || props.reducedMotion || !pack.value) return
  const element = root.value
  if (!element) return
  const bounds = element.getBoundingClientRect()
  const centreX = bounds.left + bounds.width / 2
  const centreY = bounds.top + bounds.height / 2
  const dx = event.clientX - centreX
  const dy = event.clientY - centreY
  // The pointer is somewhere on the page almost constantly, so attending to it
  // from any distance would leave the companion looking at a reader who is
  // working in the transcript and never taking a step. Only a pointer that has
  // come near is addressing the companion.
  const next = petNotices(dx, dy)
    ? petLookFor(pack.value.grid, dx, dy)
    : { direction: null, frame: null }
  const changed = next.direction !== (look.value?.direction ?? null)
  look.value = next.direction === null ? null : next
  if (changed) schedule()
}

/**
 * A look stops the walk where it stands: the pointer has the companion's
 * attention, and a leg that kept running would drag the sprite out from under
 * the pose it had just taken. When the pointer leaves, the beat picks up again
 * from wherever the companion was left standing.
 */
watch(listening, (held) => {
  if (!held) {
    restart()
    return
  }
  clearLegTimer()
  if (leg.value !== null) {
    offsetX.value = petRoamPositionAt(leg.value, Date.now() - legStartedAt)
    leg.value = null
  }
  resetTimeline()
  schedule()
})

onMounted(() => {
  restart()
  if (typeof window !== 'undefined') {
    window.addEventListener('pointermove', handlePointerMove)
  }
})

onUnmounted(() => {
  clearTimers()
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
  /* The glide is the leg's own duration, written by the component: the step is
     linear because the planner walks it at a steady pace, and the two have to
     agree exactly - an eased glide would put the sprite somewhere other than
     where the planner says it stands, and stopping mid-step would then jump. */
  transition-property: transform;
  transition-timing-function: linear;
}

@media (prefers-reduced-motion: reduce) {
  .pet-companion__sprite {
    transition: none;
  }
}

/*
 * The intro is a separate transform on the frame, not on the sprite: the
 * sprite's transform carries the walk, and an animation on the same element
 * would fight the inline translate the roam writes.
 */

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
