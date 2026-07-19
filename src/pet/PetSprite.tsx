import { memo, useEffect, useRef, useState } from "react";
import type { CSSProperties, SyntheticEvent } from "react";
import { petMotionProfiles } from "../petProfile";
import type { EvolutionPath, EvolutionStage, PetMotionMode, PetSpecies } from "../petProfile";
import blenderMotionMap from "./blenderMotionMap.json";
import {
  getPetActionDefinition,
  getPetFrameSrc,
  getPetManifestSnapshot,
  loadPetManifests,
} from "./petManifest";
import type {
  PetActionDefinition,
  PetManifestIndex,
  PetSpriteAction,
} from "./petManifest";

export type PetAnimationMarkerEvent = Readonly<{
  actionInstanceId: string;
  actionId: PetSpriteAction;
  marker: string;
  frame: number;
  eventId: string;
  transactionId?: string;
  atEpochMs: number;
}>;

export type PetAnimationLoopBoundaryEvent = Readonly<{
  actionInstanceId: string;
  actionId: PetSpriteAction;
  loopCount: number;
  atEpochMs: number;
}>;

export type PetPlaybackState = Readonly<{
  actionInstanceId: string;
  actionId: PetSpriteAction;
  sequenceIndex: number;
  frameNumber: number;
  loopCount: number;
  startedAtEpochMs: number;
  phase: "entry" | "main" | "hold" | "maintenance";
  completed: boolean;
}>;

export type PetSpriteProps = {
  species: PetSpecies;
  action: PetSpriteAction;
  evolutionStage?: EvolutionStage;
  evolutionPath?: EvolutionPath;
  className?: string;
  lookX?: number;
  lookY?: number;
  decorative?: boolean;
  gaze?: boolean;
  /** Stable IDs and start times keep separate Tauri windows on one timeline. */
  actionInstanceId?: string;
  startedAtEpochMs?: number;
  /** Logical pixels per second. Used only by velocity_driven actions. */
  velocity?: number;
  /** App-level reduced motion. The system preference is always OR-ed with it. */
  reducedMotion?: boolean;
  /** Controls both ambient breathing cadence and the speed of deliberate actions. */
  motionMode?: PetMotionMode;
  transactionId?: string;
  onMarker?: (event: PetAnimationMarkerEvent) => void;
  onLoopBoundary?: (event: PetAnimationLoopBoundaryEvent) => void;
  onAnimationEnd?: () => void;
};

type TimelinePosition = PetPlaybackState & {
  playbackKey: string;
  timelineStep: number;
  hasFutureFrame: boolean;
};

type InstanceClock = {
  identityKey: string;
  actionInstanceId: string;
  startedAtEpochMs: number;
};

type ImageFallback = {
  imageKey: string;
  stage: number;
};

const TRANSPARENT_PIXEL = "data:image/gif;base64,R0lGODlhAQABAAD/ACwAAAAAAQABAAACADs=";
const blenderPetAssetRoot = `${import.meta.env.BASE_URL.replace(/\/?$/, "/")}pets/blender-rendered`;
const blenderSpeciesAliases = blenderMotionMap.speciesAliases as Readonly<Record<string, string>>;
const blenderMotionFamilyByAction = blenderMotionMap.actions as Readonly<Record<PetSpriteAction, string>>;
let generatedIdCounter = 0;

export function getBlenderPetFrameSrc(
  species: PetSpecies,
  evolutionStage: EvolutionStage,
  evolutionPath: EvolutionPath,
  action: PetSpriteAction,
  frame: number,
) {
  const renderedSpecies = blenderSpeciesAliases[species] ?? species;
  const motionFamily = blenderMotionFamilyByAction[action] ?? "idle";
  const renderedFrame = String(((Math.max(1, frame) - 1) % blenderMotionMap.render.frameCount) + 1).padStart(2, "0");
  return `${blenderPetAssetRoot}/${renderedSpecies}/${evolutionStage}/${evolutionPath}/${motionFamily}/${renderedFrame}.png`;
}

export function getBlenderPetPreviewSrc(
  species: PetSpecies,
  evolutionStage: EvolutionStage,
  evolutionPath: EvolutionPath,
) {
  return getBlenderPetFrameSrc(species, evolutionStage, evolutionPath, "idle_breathe", 1);
}

function createRuntimeId(prefix: string) {
  const randomUuid = globalThis.crypto?.randomUUID?.();
  if (randomUuid) return randomUuid;
  generatedIdCounter += 1;
  return `${prefix}-${Date.now().toString(36)}-${generatedIdCounter.toString(36)}`;
}

function effectiveFramesPerSecond(
  definition: PetActionDefinition,
  reducedMotion: boolean,
  motionMode: PetMotionMode,
  velocity?: number,
) {
  if (motionMode === "quiet" && definition.id === "idle_breathe") return 0;
  let fps = definition.fps * petMotionProfiles[motionMode].actionRate;
  if (definition.mode === "velocity_driven" && velocity !== undefined) {
    const speed = Math.abs(velocity);
    if (speed < 1) return 0;
    // 900 logical px/s reaches the manifest FPS ceiling; slower movement
    // advances proportionally without inventing motion while stationary.
    fps *= Math.min(1, speed / 900);
  }
  if (reducedMotion) {
    const reducedCeiling = definition.mode === "once" || definition.mode === "hold_last" ? 6 : 2;
    fps = Math.min(fps, reducedCeiling);
  }
  return Math.max(0.1, fps);
}

function positionInAmbientIdleLoop(
  definition: PetActionDefinition,
  clock: InstanceClock,
  playbackKey: string,
  elapsedMs: number,
  frameDurationMs: number,
  pauseMs: number,
): TimelinePosition {
  const entryDurationMs = definition.entrySequence.length * frameDurationMs;
  if (elapsedMs < entryDurationMs) {
    const entryStep = Math.min(definition.entrySequence.length - 1, Math.floor(elapsedMs / frameDurationMs));
    return {
      playbackKey,
      actionInstanceId: clock.actionInstanceId,
      actionId: definition.id,
      sequenceIndex: entryStep,
      frameNumber: definition.entrySequence[entryStep] ?? 1,
      loopCount: 0,
      startedAtEpochMs: clock.startedAtEpochMs,
      phase: "entry",
      completed: false,
      timelineStep: entryStep,
      hasFutureFrame: true,
    };
  }

  const mainElapsedMs = elapsedMs - entryDurationMs;
  const motionDurationMs = definition.sequence.length * frameDurationMs;
  const cycleDurationMs = motionDurationMs + pauseMs;
  const loopCount = Math.floor(mainElapsedMs / cycleDurationMs);
  const cycleElapsedMs = mainElapsedMs % cycleDurationMs;
  const timelineBase = definition.entrySequence.length + loopCount * definition.sequence.length;
  if (cycleElapsedMs >= motionDurationMs) {
    return {
      playbackKey,
      actionInstanceId: clock.actionInstanceId,
      actionId: definition.id,
      sequenceIndex: 0,
      frameNumber: definition.sequence[0] ?? 1,
      loopCount,
      startedAtEpochMs: clock.startedAtEpochMs,
      phase: "hold",
      completed: false,
      timelineStep: timelineBase + definition.sequence.length,
      hasFutureFrame: true,
    };
  }

  const sequenceIndex = Math.min(definition.sequence.length - 1, Math.floor(cycleElapsedMs / frameDurationMs));
  return {
    playbackKey,
    actionInstanceId: clock.actionInstanceId,
    actionId: definition.id,
    sequenceIndex,
    frameNumber: definition.sequence[sequenceIndex] ?? 1,
    loopCount,
    startedAtEpochMs: clock.startedAtEpochMs,
    phase: "main",
    completed: false,
    timelineStep: timelineBase + sequenceIndex,
    hasFutureFrame: true,
  };
}

function positionInFiniteSequence(
  definition: PetActionDefinition,
  step: number,
  actionInstanceId: string,
  startedAtEpochMs: number,
  playbackKey: string,
): TimelinePosition {
  const entryLength = definition.entrySequence.length;
  const totalLength = entryLength + definition.sequence.length;
  const finalIndex = Math.max(0, totalLength - 1);
  const clampedIndex = Math.min(step, finalIndex);
  const inEntry = clampedIndex < entryLength;
  const sequenceIndex = inEntry ? clampedIndex : clampedIndex - entryLength;
  const frameNumber = inEntry
    ? definition.entrySequence[sequenceIndex]
    : definition.sequence[sequenceIndex];
  const isOnce = definition.mode === "once";
  const isHoldLast = definition.mode === "hold_last";
  const completed = isOnce && step >= totalLength;
  const holding = isHoldLast && step >= totalLength;
  return {
    playbackKey,
    actionInstanceId,
    actionId: definition.id,
    sequenceIndex,
    frameNumber,
    loopCount: holding ? 1 : 0,
    startedAtEpochMs,
    phase: inEntry ? "entry" : holding || step >= totalLength ? "hold" : "main",
    completed,
    timelineStep: step,
    hasFutureFrame: isOnce ? !completed : isHoldLast ? !holding : step < finalIndex,
  };
}

function positionInLoop(
  definition: PetActionDefinition,
  step: number,
  actionInstanceId: string,
  startedAtEpochMs: number,
  playbackKey: string,
): TimelinePosition {
  const entryLength = definition.entrySequence.length;
  if (step < entryLength) {
    return {
      playbackKey,
      actionInstanceId,
      actionId: definition.id,
      sequenceIndex: step,
      frameNumber: definition.entrySequence[step],
      loopCount: 0,
      startedAtEpochMs,
      phase: "entry",
      completed: false,
      timelineStep: step,
      hasFutureFrame: true,
    };
  }
  const mainStep = step - entryLength;
  const sequenceIndex = mainStep % definition.sequence.length;
  return {
    playbackKey,
    actionInstanceId,
    actionId: definition.id,
    sequenceIndex,
    frameNumber: definition.sequence[sequenceIndex],
    loopCount: Math.floor(mainStep / definition.sequence.length),
    startedAtEpochMs,
    phase: "main",
    completed: false,
    timelineStep: step,
    hasFutureFrame: true,
  };
}

function positionInHeldPingPong(
  definition: PetActionDefinition,
  step: number,
  frameDurationMs: number,
  actionInstanceId: string,
  startedAtEpochMs: number,
  playbackKey: string,
): TimelinePosition {
  const entryLength = definition.entrySequence.length;
  if (step < entryLength) {
    return {
      playbackKey,
      actionInstanceId,
      actionId: definition.id,
      sequenceIndex: step,
      frameNumber: definition.entrySequence[step],
      loopCount: 0,
      startedAtEpochMs,
      phase: "entry",
      completed: false,
      timelineStep: step,
      hasFutureFrame: true,
    };
  }

  const mainStep = step - entryLength;
  if (mainStep < definition.sequence.length) {
    return {
      playbackKey,
      actionInstanceId,
      actionId: definition.id,
      sequenceIndex: mainStep,
      frameNumber: definition.sequence[mainStep],
      loopCount: 0,
      startedAtEpochMs,
      phase: "main",
      completed: false,
      timelineStep: step,
      hasFutureFrame: true,
    };
  }

  const holdSteps = Math.max(0, Math.ceil((definition.holdLastMs ?? 0) / frameDurationMs));
  const afterFirstPass = mainStep - definition.sequence.length;
  if (afterFirstPass < holdSteps) {
    return {
      playbackKey,
      actionInstanceId,
      actionId: definition.id,
      sequenceIndex: definition.sequence.length - 1,
      frameNumber: definition.sequence.at(-1) ?? 1,
      loopCount: 0,
      startedAtEpochMs,
      phase: "hold",
      completed: false,
      timelineStep: step,
      hasFutureFrame: true,
    };
  }

  // Frame 1 is normally the neutral entry pose. Avoid an unconditional jump
  // back to it when maintaining a held ping-pong action.
  const maintenanceStart = definition.sequence[0] === 1 && definition.sequence.length > 1 ? 1 : 0;
  const maintenanceSequence = definition.sequence.slice(maintenanceStart);
  const maintenanceStep = afterFirstPass - holdSteps;
  const maintenanceIndex = maintenanceStep % maintenanceSequence.length;
  return {
    playbackKey,
    actionInstanceId,
    actionId: definition.id,
    sequenceIndex: maintenanceStart + maintenanceIndex,
    frameNumber: maintenanceSequence[maintenanceIndex],
    loopCount: 1 + Math.floor(maintenanceStep / maintenanceSequence.length),
    startedAtEpochMs,
    phase: "maintenance",
    completed: false,
    timelineStep: step,
    hasFutureFrame: true,
  };
}

function deriveTimelinePosition(
  definition: PetActionDefinition,
  clock: InstanceClock,
  playbackKey: string,
  now: number,
  reducedMotion: boolean,
  motionMode: PetMotionMode,
  velocity?: number,
): TimelinePosition {
  const fps = effectiveFramesPerSecond(definition, reducedMotion, motionMode, velocity);
  const frameDurationMs = fps > 0 ? 1000 / fps : Number.POSITIVE_INFINITY;
  const elapsedMs = Math.max(0, now - clock.startedAtEpochMs);
  const step = Number.isFinite(frameDurationMs) ? Math.floor(elapsedMs / frameDurationMs) : 0;

  if (definition.id === "idle_breathe" && Number.isFinite(frameDurationMs)) {
    return positionInAmbientIdleLoop(
      definition,
      clock,
      playbackKey,
      elapsedMs,
      frameDurationMs,
      petMotionProfiles[motionMode].idlePauseMs,
    );
  }

  if (definition.mode === "once" || definition.mode === "hold_last") {
    return positionInFiniteSequence(definition, step, clock.actionInstanceId, clock.startedAtEpochMs, playbackKey);
  }
  if (definition.mode === "hold_last_ping_pong") {
    return positionInHeldPingPong(
      definition,
      step,
      frameDurationMs,
      clock.actionInstanceId,
      clock.startedAtEpochMs,
      playbackKey,
    );
  }
  const position = positionInLoop(definition, step, clock.actionInstanceId, clock.startedAtEpochMs, playbackKey);
  if (!Number.isFinite(frameDurationMs)) return { ...position, hasFutureFrame: false };
  return position;
}

function firstTimelineStepForFrame(definition: PetActionDefinition, frame: number) {
  const entryIndex = definition.entrySequence.indexOf(frame);
  if (entryIndex >= 0) return entryIndex;
  const sequenceIndex = definition.sequence.indexOf(frame);
  return sequenceIndex >= 0 ? definition.entrySequence.length + sequenceIndex : Number.POSITIVE_INFINITY;
}

function samePlayback(left: TimelinePosition, right: TimelinePosition) {
  return left.playbackKey === right.playbackKey
    && left.sequenceIndex === right.sequenceIndex
    && left.frameNumber === right.frameNumber
    && left.loopCount === right.loopCount
    && left.phase === right.phase
    && left.completed === right.completed
    && left.hasFutureFrame === right.hasFutureFrame;
}

function nextFrameDelay(
  definition: PetActionDefinition,
  clock: InstanceClock,
  now: number,
  reducedMotion: boolean,
  motionMode: PetMotionMode,
  velocity?: number,
) {
  const fps = effectiveFramesPerSecond(definition, reducedMotion, motionMode, velocity);
  if (fps <= 0) return Number.POSITIVE_INFINITY;
  const frameDurationMs = 1000 / fps;
  const elapsedMs = Math.max(0, now - clock.startedAtEpochMs);
  if (definition.id === "idle_breathe") {
    const entryDurationMs = definition.entrySequence.length * frameDurationMs;
    if (elapsedMs < entryDurationMs) {
      const entryStep = Math.floor(elapsedMs / frameDurationMs);
      const nextEntryBoundaryMs = (entryStep + 1) * frameDurationMs;
      return Math.max(16, Math.min(60_000, nextEntryBoundaryMs - elapsedMs + 1));
    }
    const mainElapsedMs = Math.max(0, elapsedMs - entryDurationMs);
    const motionDurationMs = definition.sequence.length * frameDurationMs;
    const cycleDurationMs = motionDurationMs + petMotionProfiles[motionMode].idlePauseMs;
    const cycleElapsedMs = mainElapsedMs % cycleDurationMs;
    if (cycleElapsedMs >= motionDurationMs) {
      return Math.max(16, Math.min(60_000, cycleDurationMs - cycleElapsedMs + 1));
    }
    const sequenceStep = Math.floor(cycleElapsedMs / frameDurationMs);
    const nextSequenceBoundaryMs = (sequenceStep + 1) * frameDurationMs;
    return Math.max(16, Math.min(60_000, nextSequenceBoundaryMs - cycleElapsedMs + 1));
  }
  const currentStep = Math.floor(elapsedMs / frameDurationMs);
  const nextBoundary = clock.startedAtEpochMs + (currentStep + 1) * frameDurationMs;
  return Math.max(16, Math.min(60_000, nextBoundary - now + 1));
}

export const PetSprite = memo(function PetSprite({
  species,
  action,
  evolutionStage = "seedling",
  evolutionPath = "companion",
  className = "",
  lookX = 0,
  lookY = 0,
  decorative = true,
  gaze = false,
  actionInstanceId,
  startedAtEpochMs,
  velocity,
  reducedMotion = false,
  motionMode = "gentle",
  transactionId,
  onMarker,
  onLoopBoundary,
  onAnimationEnd,
}: PetSpriteProps) {
  const [manifestIndex, setManifestIndex] = useState<PetManifestIndex>(getPetManifestSnapshot);
  const [systemReducedMotion, setSystemReducedMotion] = useState(
    () => typeof window !== "undefined" && (window.matchMedia?.("(prefers-reduced-motion: reduce)").matches ?? false),
  );
  const effectiveReducedMotion = reducedMotion || systemReducedMotion;
  const instanceClockRef = useRef<InstanceClock | null>(null);
  const onAnimationEndRef = useRef(onAnimationEnd);
  const onMarkerRef = useRef(onMarker);
  const onLoopBoundaryRef = useRef(onLoopBoundary);
  const firedMarkersRef = useRef(new Map<string, Set<string>>());
  const emittedLoopCountRef = useRef(new Map<string, number>());
  const completedInstancesRef = useRef(new Set<string>());
  const [imageFallback, setImageFallback] = useState<ImageFallback>({ imageKey: "", stage: 0 });

  onAnimationEndRef.current = onAnimationEnd;
  onMarkerRef.current = onMarker;
  onLoopBoundaryRef.current = onLoopBoundary;

  const requestedStartedAt = Number.isFinite(startedAtEpochMs) ? Number(startedAtEpochMs) : undefined;
  const identityKey = `${species}\u0000${action}\u0000${actionInstanceId ?? "internal"}\u0000${requestedStartedAt ?? "now"}`;
  if (instanceClockRef.current?.identityKey !== identityKey) {
    instanceClockRef.current = {
      identityKey,
      actionInstanceId: actionInstanceId || createRuntimeId("pet-action"),
      startedAtEpochMs: requestedStartedAt ?? Date.now(),
    };
  }
  const clock = instanceClockRef.current;
  const resolvedAction = manifestIndex.source === "manifest" ? action : "idle_breathe";
  const definition = getPetActionDefinition(resolvedAction, manifestIndex);
  const playbackKey = `${clock.actionInstanceId}:${clock.startedAtEpochMs}:${manifestIndex.revision}:${resolvedAction}:${motionMode}`;
  const immediatePlayback = deriveTimelinePosition(
    definition,
    clock,
    playbackKey,
    Date.now(),
    effectiveReducedMotion,
    motionMode,
    velocity,
  );
  const [playback, setPlayback] = useState<TimelinePosition>(immediatePlayback);
  // A prop change can render before effects run. Deriving the new instance's
  // first/current frame here prevents the new action from ever committing an
  // old action's sequence index.
  const activePlayback = playback.playbackKey === playbackKey ? playback : immediatePlayback;

  useEffect(() => {
    let active = true;
    void loadPetManifests().then((loaded) => {
      if (active) setManifestIndex(loaded);
    }).catch(() => {
      // loadPetManifests already reports validation diagnostics. Development
      // keeps the rejected promise visible while the renderer stays safe.
    });
    return () => {
      active = false;
    };
  }, []);

  useEffect(() => {
    if (typeof window === "undefined") return;
    const media = window.matchMedia?.("(prefers-reduced-motion: reduce)");
    if (!media) return;
    const update = () => setSystemReducedMotion(media.matches);
    media.addEventListener("change", update);
    return () => media.removeEventListener("change", update);
  }, []);

  useEffect(() => {
    const frames = new Set([...definition.entrySequence, ...definition.sequence]);
    for (const frame of frames) {
      const image = new Image();
      image.src = getBlenderPetFrameSrc(species, evolutionStage, evolutionPath, resolvedAction, frame);
      const fallbackImage = new Image();
      fallbackImage.src = getPetFrameSrc(species, resolvedAction, frame, manifestIndex);
    }
    const idleImage = new Image();
    idleImage.src = getBlenderPetFrameSrc(species, evolutionStage, evolutionPath, "idle_breathe", 1);
  }, [definition, evolutionPath, evolutionStage, manifestIndex, resolvedAction, species]);

  useEffect(() => {
    let disposed = false;
    let timer = 0;

    const emitDueMarkers = (position: TimelinePosition, now: number) => {
      if (!onMarkerRef.current) return;
      let fired = firedMarkersRef.current.get(position.actionInstanceId);
      if (!fired) {
        fired = new Set<string>();
        firedMarkersRef.current.set(position.actionInstanceId, fired);
        if (firedMarkersRef.current.size > 32) {
          const oldest = firedMarkersRef.current.keys().next().value as string | undefined;
          if (oldest) firedMarkersRef.current.delete(oldest);
        }
      }
      const dueMarkers = Object.entries(definition.markers)
        .map(([marker, frame]) => ({ marker, frame, step: firstTimelineStepForFrame(definition, frame) }))
        .filter(({ marker, step }) => !fired?.has(marker) && step <= position.timelineStep)
        .sort((left, right) => left.step - right.step || left.marker.localeCompare(right.marker));
      for (const { marker, frame } of dueMarkers) {
        fired.add(marker);
        onMarkerRef.current?.({
          actionInstanceId: position.actionInstanceId,
          actionId: definition.id,
          marker,
          frame,
          eventId: createRuntimeId("pet-marker"),
          transactionId,
          atEpochMs: now,
        });
      }
    };

    const updatePlayback = () => {
      if (disposed) return;
      const now = Date.now();
      const next = deriveTimelinePosition(
        definition,
        clock,
        playbackKey,
        now,
        effectiveReducedMotion,
        motionMode,
        velocity,
      );
      setPlayback((current) => samePlayback(current, next) ? current : next);
      emitDueMarkers(next, now);

      const lastLoopCount = emittedLoopCountRef.current.get(next.actionInstanceId) ?? 0;
      if (next.loopCount > lastLoopCount) {
        emittedLoopCountRef.current.set(next.actionInstanceId, next.loopCount);
        if (emittedLoopCountRef.current.size > 32) {
          const oldest = emittedLoopCountRef.current.keys().next().value as string | undefined;
          if (oldest) emittedLoopCountRef.current.delete(oldest);
        }
        onLoopBoundaryRef.current?.({
          actionInstanceId: next.actionInstanceId,
          actionId: definition.id,
          loopCount: next.loopCount,
          atEpochMs: now,
        });
      }

      const completionKey = `${next.actionInstanceId}:${definition.id}`;
      if (next.completed && !completedInstancesRef.current.has(completionKey)) {
        completedInstancesRef.current.add(completionKey);
        if (completedInstancesRef.current.size > 64) {
          const oldest = completedInstancesRef.current.values().next().value as string | undefined;
          if (oldest) completedInstancesRef.current.delete(oldest);
        }
        onAnimationEndRef.current?.();
      }

      if (!next.hasFutureFrame) return;
      const delay = nextFrameDelay(definition, clock, now, effectiveReducedMotion, motionMode, velocity);
      if (Number.isFinite(delay)) timer = window.setTimeout(updatePlayback, delay);
    };

    updatePlayback();
    return () => {
      disposed = true;
      window.clearTimeout(timer);
    };
  }, [clock, definition, effectiveReducedMotion, motionMode, playbackKey, transactionId, velocity]);

  const clampedLookX = Math.max(-1, Math.min(1, lookX));
  const clampedLookY = Math.max(-1, Math.min(1, lookY));
  const showGaze = gaze && resolvedAction === "idle_breathe" && activePlayback.frameNumber !== 3;
  const style = {
    "--pet-look-x": `${clampedLookX * 3}px`,
    "--pet-look-y": `${clampedLookY * 2}px`,
    "--pet-look-rotate": `${clampedLookX * 2.5}deg`,
    "--pet-gaze-x": `${clampedLookX * 2.4}px`,
    "--pet-gaze-y": `${clampedLookY * 1.8}px`,
  } as CSSProperties;

  const imageKey = `${playbackKey}:${species}:${evolutionStage}:${evolutionPath}`;
  const fallbackStage = imageFallback.imageKey === imageKey ? imageFallback.stage : 0;
  const renderedFrame = String(((activePlayback.frameNumber - 1) % 4) + 1).padStart(2, "0");
  const renderedSpecies = blenderSpeciesAliases[species] ?? species;
  const motionFamily = blenderMotionFamilyByAction[resolvedAction] ?? "idle";
  const candidateSources = Array.from(new Set([
    getBlenderPetFrameSrc(species, evolutionStage, evolutionPath, resolvedAction, activePlayback.frameNumber),
    getBlenderPetFrameSrc(species, evolutionStage, "companion", resolvedAction, activePlayback.frameNumber),
    getBlenderPetFrameSrc(species, evolutionStage, "companion", "idle_breathe", activePlayback.frameNumber),
    `${blenderPetAssetRoot}/${renderedSpecies}/${evolutionStage}/${renderedFrame}.png`,
    getPetFrameSrc(species, resolvedAction, activePlayback.frameNumber, manifestIndex),
    getPetFrameSrc(species, "idle_breathe", activePlayback.frameNumber, manifestIndex),
    getPetFrameSrc("star-cat", "idle_breathe", activePlayback.frameNumber, manifestIndex),
    TRANSPARENT_PIXEL,
  ]));
  const imageSrc = candidateSources[Math.min(fallbackStage, candidateSources.length - 1)];

  const handleImageError = (event: SyntheticEvent<HTMLImageElement>) => {
    if (fallbackStage >= candidateSources.length - 1) {
      event.currentTarget.onerror = null;
      return;
    }
    setImageFallback({ imageKey, stage: fallbackStage + 1 });
  };

  return (
    <span
      className={`pet-sprite ${className}`}
      data-action={action}
      data-rendered-action={resolvedAction}
      data-frame={activePlayback.frameNumber}
      data-species={species}
      data-motion-family={motionFamily}
      data-evolution-stage={evolutionStage}
      data-evolution-path={evolutionPath}
      data-action-instance-id={activePlayback.actionInstanceId}
      data-sequence-index={activePlayback.sequenceIndex}
      data-loop-count={activePlayback.loopCount}
      data-started-at={activePlayback.startedAtEpochMs}
      data-reduced-motion={effectiveReducedMotion ? "true" : undefined}
      data-motion-mode={motionMode}
      data-gaze={showGaze ? "true" : undefined}
      data-image-fallback={fallbackStage > 0 ? fallbackStage : undefined}
      style={style}
      aria-hidden={decorative || undefined}
    >
      <span className="pet-sprite-shadow" />
      <img
        key={imageKey}
        alt={decorative ? "" : `${species} ${resolvedAction}`}
        draggable={false}
        src={imageSrc}
        onError={handleImageError}
      />
      {showGaze ? (
        <span className="pet-sprite-gaze" aria-hidden="true">
          <i className="pet-sprite-pupil pet-sprite-pupil-left" />
          <i className="pet-sprite-pupil pet-sprite-pupil-right" />
        </span>
      ) : null}
    </span>
  );
});
