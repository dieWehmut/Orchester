<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from 'vue'
import brandIcon from '../assets/brand-icon.png'

const emit = defineEmits<{ finish: [] }>()
const leaving = ref(false)
let holdTimer: ReturnType<typeof setTimeout> | undefined
let exitTimer: ReturnType<typeof setTimeout> | undefined

onMounted(() => {
  const reducedMotion = window.matchMedia('(prefers-reduced-motion: reduce)').matches
  holdTimer = setTimeout(() => {
    leaving.value = true
    exitTimer = setTimeout(() => emit('finish'), reducedMotion ? 100 : 420)
  }, reducedMotion ? 80 : 820)
})

onBeforeUnmount(() => {
  clearTimeout(holdTimer)
  clearTimeout(exitTimer)
})
</script>

<template>
  <div class="launch-screen" :class="{ 'is-leaving': leaving }" aria-hidden="true">
    <div class="launch-halo" />
    <div class="launch-brand">
      <div class="launch-portrait">
        <span class="launch-orbit" />
        <img :src="brandIcon" alt="" width="112" height="112" />
        <span class="launch-spark launch-spark-one" />
        <span class="launch-spark launch-spark-two" />
      </div>
      <p class="launch-wordmark">Orchester</p>
      <p class="launch-caption">连接你的电脑，继续创造</p>
      <div class="launch-rhythm"><i /><i /><i /><i /><i /></div>
    </div>
  </div>
</template>

<style scoped>
.launch-screen {
  position: fixed;
  inset: 0;
  z-index: 1000;
  display: grid;
  place-items: center;
  overflow: hidden;
  background: var(--bg, #1b110e);
  color: var(--text, #f4e0d7);
  opacity: 1;
  transition: opacity 420ms ease, visibility 420ms;
}
.launch-screen.is-leaving { opacity: 0; visibility: hidden; }
.launch-halo {
  position: absolute;
  width: 420px;
  height: 420px;
  border-radius: 50%;
  background: radial-gradient(circle, color-mix(in srgb, var(--primary, #ffb79a) 13%, transparent), transparent 67%);
  animation: launch-halo-in 1.2s ease-out both;
}
.launch-brand { position: relative; display: grid; justify-items: center; transform: translateY(-18px); }
.launch-portrait { position: relative; padding: 18px; animation: launch-portrait-in 650ms cubic-bezier(.2,.7,.2,1) both; }
.launch-portrait img {
  display: block;
  width: 112px;
  height: 112px;
  border-radius: 28px;
  box-shadow: 0 12px 40px #0003, 0 0 0 1px #fff2;
}
.launch-orbit {
  position: absolute;
  inset: 0;
  border: 1px solid color-mix(in srgb, var(--primary, #ffb79a) 22%, transparent);
  border-radius: 46px;
  transform: rotate(-9deg);
}
.launch-spark { position: absolute; width: 5px; height: 5px; border-radius: 50%; background: var(--primary, #ffb79a); }
.launch-spark-one { top: 4px; right: 28px; }
.launch-spark-two { bottom: 15px; left: 5px; width: 3px; height: 3px; background: var(--secondary, #9ccac5); }
.launch-wordmark { margin: 26px 0 0; font-size: 32px; font-weight: 650; letter-spacing: -.8px; animation: launch-copy-in 500ms 140ms ease-out both; }
.launch-caption { margin: 12px 0 0; font-size: 13px; letter-spacing: 1.4px; color: var(--muted, #b8a297); animation: launch-copy-in 500ms 230ms ease-out both; }
.launch-rhythm { display: flex; align-items: center; gap: 5px; height: 22px; margin-top: 32px; }
.launch-rhythm i { display: block; width: 3px; height: 8px; border-radius: 3px; background: var(--primary, #ffb79a); animation: launch-note 800ms ease-in-out infinite alternate; }
.launch-rhythm i:nth-child(2), .launch-rhythm i:nth-child(4) { animation-delay: 120ms; height: 13px; }
.launch-rhythm i:nth-child(3) { animation-delay: 240ms; height: 19px; }
@keyframes launch-portrait-in { from { opacity: 0; transform: scale(.91) translateY(8px); } to { opacity: 1; transform: scale(1) translateY(0); } }
@keyframes launch-copy-in { from { opacity: 0; transform: translateY(6px); } to { opacity: 1; transform: translateY(0); } }
@keyframes launch-halo-in { from { opacity: 0; transform: scale(.75); } to { opacity: 1; transform: scale(1); } }
@keyframes launch-note { from { opacity: .3; transform: scaleY(.55); } to { opacity: .9; transform: scaleY(1); } }
@media (prefers-reduced-motion: reduce) {
  .launch-screen { transition-duration: 100ms; }
  .launch-halo, .launch-portrait, .launch-wordmark, .launch-caption, .launch-rhythm i { animation: none; }
}
</style>
