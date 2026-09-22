export { default as PetCompanion } from './PetCompanion.vue'
export {
  PET_ANIMATION_NAMES,
  animationFrameAt,
  animationFrameDelay,
  createPetAnimations,
  createPetWalkTrack,
  petPoseHoldMs,
  petStateCycleMs,
  petWalkCycleMs,
  resolveAnimation,
  type PetAnimation,
  type PetAnimationFrame,
  type PetAnimationName,
} from './pet-animations'
export {
  PET_LOOK_DEADZONE_PX,
  PET_LOOK_DIRECTIONS,
  lookDirectionFor,
  petLookFor,
  type PetLook,
} from './pet-look'
export {
  PET_ATLAS_HEIGHT,
  PET_ATLAS_WIDTH,
  PET_COLUMNS,
  PET_FRAME_HEIGHT,
  PET_FRAME_WIDTH,
  PET_SPRITE_VERSION,
  frameIndex,
  frameOffset,
  parsePetManifest,
  petGridFor,
  resolveSpritesheetUrl,
  type PetGrid,
  type PetManifest,
} from './pet-manifest'
export {
  petStateFor,
  type PetNotificationKind,
  type PetState,
  type PetStateInput,
} from './pet-state'
export {
  PET_ROAM_FIRST_PAUSE_MS,
  PET_ROAM_MAX_CYCLES,
  PET_ROAM_PAUSE_MAX_MS,
  PET_ROAM_PAUSE_MIN_MS,
  PET_ROAM_STRIDE_PX,
  petRoamAllowed,
  petRoamPositionAt,
  petWalkAnimation,
  planPetRoamLeg,
  planPetRoamPause,
  type PetRoamBounds,
  type PetRoamDirection,
  type PetRoamLeg,
} from './pet-roam'
export { PET_PACK_URL, loadPetPack, resetPetPackForTests, usePetPack, type PetPack } from './use-pet-pack'
export {
  PET_VISIBILITY_STORAGE_KEY,
  resetPetVisibilityForTests,
  usePetVisibility,
  type PetVisibilityApi,
} from './use-pet-visibility'
