import type {
  PetActionCooldown,
  PetActionDefinition,
  PetActionEndReason,
  PetActionInstance,
  PetActionSource,
  PetBehaviorEvent,
  PetBehaviorMode,
  PetBehaviorRuntimeConfig,
  PetBehaviorState,
  PetBehaviorTransition,
  PetMarkerEvent,
  PetRequestDropReason,
  PetReturnAction,
  PetRuntimeEffect,
  PetRuntimeSnapshot,
  PetSignalEvent,
  QueuedPetAction,
} from "./petBehaviorTypes";
import { petMotionProfiles } from "../petProfile";

export * from "./petBehaviorTypes";

export const PET_BEHAVIOR_QUEUE_LIMIT = 8;
export const PET_AUTONOMY_INTERVALS_MS = {
  gentle: petMotionProfiles.gentle.autonomyIntervalMs!,
  lively: petMotionProfiles.lively.autonomyIntervalMs!,
} as const;

export const DEFAULT_COALESCED_SIGNALS = [
  "pointer_enter",
  "pointer_hover",
  "typing_started",
] as const;

export const DEFAULT_QUIET_SUPPRESSED_SIGNALS = [
  "pointer_enter",
  "pointer_hover",
  "typing_started",
  "pleasant_event",
  "unexpected_event",
  "repeated_poking",
  "long_ignored",
  "idle_timeout",
  "screen_edge_reached",
  "small_obstacle",
] as const;

export const DEFAULT_CHAIN_ADVANCE_MARKERS: Readonly<Record<string, readonly string[]>> = {
  voice_wake: ["ready"],
};

export const DEFAULT_TRANSACTION_LOCK_MARKERS: Readonly<Record<string, string>> = {
  file_eat: "commitToRecycleBin",
};

export const DEFAULT_TRANSACTION_RESULT_SIGNALS = ["recycle_completed", "recycle_failed"] as const;

const DEFAULT_EVENT_DEDUPE_LIMIT = 128;
const DEFAULT_RANDOM_SEED = 0x6d2b_79f5;
const RETURN_STACK_LIMIT = 8;

type InterruptBoundary = "signal" | "loop" | "marker";

function clamp(value: number, min: number, max: number) {
  return Math.min(max, Math.max(min, value));
}

function finiteOr(value: number | undefined, fallback: number) {
  return typeof value === "number" && Number.isFinite(value) ? value : fallback;
}

function clonePayload(payload?: Record<string, unknown>) {
  return payload ? { ...payload } : undefined;
}

function cloneRequest(request: QueuedPetAction): QueuedPetAction {
  return {
    ...request,
    payload: clonePayload(request.payload),
    chainActionIds: request.chainActionIds ? [...request.chainActionIds] : undefined,
  };
}

function cloneInstance(instance: PetActionInstance): PetActionInstance {
  return {
    ...instance,
    firedMarkers: [...instance.firedMarkers],
    payload: clonePayload(instance.payload),
    chainActionIds: instance.chainActionIds ? [...instance.chainActionIds] : undefined,
  };
}

function cloneReturnAction(action: PetReturnAction): PetReturnAction {
  return { ...action, payload: clonePayload(action.payload) };
}

export function clonePetBehaviorState(state: PetBehaviorState): PetBehaviorState {
  return {
    ...state,
    current: cloneInstance(state.current),
    queue: state.queue.map(cloneRequest),
    returnStack: state.returnStack.map(cloneReturnAction),
    cooldowns: Object.fromEntries(
      Object.entries(state.cooldowns).map(([key, cooldown]) => [key, { ...cooldown }]),
    ),
    recentEventIds: [...state.recentEventIds],
    recentAutonomousActions: [...state.recentAutonomousActions],
    constraints: { ...state.constraints },
    needs: { ...state.needs },
  };
}

function normalizeSeed(seed?: number) {
  const normalized = finiteOr(seed, DEFAULT_RANDOM_SEED) >>> 0;
  return normalized === 0 ? DEFAULT_RANDOM_SEED : normalized;
}

function nextRandom(state: PetBehaviorState) {
  let value = state.randomState >>> 0;
  value ^= value << 13;
  value ^= value >>> 17;
  value ^= value << 5;
  state.randomState = (value >>> 0) || DEFAULT_RANDOM_SEED;
  return state.randomState / 0x1_0000_0000;
}

function randomBetween(state: PetBehaviorState, first: number, second: number) {
  const min = Math.min(first, second);
  const max = Math.max(first, second);
  if (max <= min) return min;
  return min + Math.floor(nextRandom(state) * (max - min + 1));
}

function allocateId(state: PetBehaviorState, prefix: string, atEpochMs: number) {
  const ordinal = state.nextOrdinal;
  state.nextOrdinal += 1;
  return `${prefix}-${Math.max(0, Math.floor(atEpochMs)).toString(36)}-${ordinal.toString(36)}`;
}

function getQueueLimit(config: PetBehaviorRuntimeConfig) {
  return clamp(Math.floor(finiteOr(config.queueLimit, PET_BEHAVIOR_QUEUE_LIMIT)), 1, PET_BEHAVIOR_QUEUE_LIMIT);
}

function getEventDedupeLimit(config: PetBehaviorRuntimeConfig) {
  return Math.max(8, Math.floor(finiteOr(config.eventDedupeLimit, DEFAULT_EVENT_DEDUPE_LIMIT)));
}

function getAction(config: PetBehaviorRuntimeConfig, actionId: string) {
  return config.actions.actions[actionId];
}

function getDefaultActionId(config: PetBehaviorRuntimeConfig) {
  if (getAction(config, config.stateMachine.default)) return config.stateMachine.default;
  if (getAction(config, "idle_breathe")) return "idle_breathe";
  const first = Object.keys(config.actions.actions)[0];
  if (!first) throw new Error("PetBehaviorRuntime requires at least one action definition.");
  return first;
}

function getActionSequence(config: PetBehaviorRuntimeConfig, actionId: string) {
  const action = getAction(config, actionId);
  const entry = action?.entrySequence?.filter((frame) => Number.isInteger(frame) && frame > 0);
  if (entry?.length) return entry;
  const sequence = action?.sequence?.filter((frame) => Number.isInteger(frame) && frame > 0);
  if (sequence?.length) return sequence;
  const defaults = config.actions.defaults?.sequence?.filter((frame) => Number.isInteger(frame) && frame > 0);
  if (defaults?.length) return defaults;
  const frameCount = Math.max(1, Math.floor(finiteOr(action?.frames, finiteOr(config.actions.defaults?.frames, 1))));
  return Array.from({ length: frameCount }, (_, index) => index + 1);
}

function getInitialNeeds(config: PetBehaviorRuntimeConfig) {
  const needs: Record<string, number> = {};
  for (const [need, definition] of Object.entries(config.stateMachine.needs ?? {})) {
    needs[need] = clamp(finiteOr(config.needs?.[need], definition.initial), definition.min, definition.max);
  }
  for (const [need, value] of Object.entries(config.needs ?? {})) {
    if (!(need in needs)) needs[need] = finiteOr(value, 0);
  }
  return needs;
}

function createInitialInstance(config: PetBehaviorRuntimeConfig, atEpochMs: number): PetActionInstance {
  const actionId = getDefaultActionId(config);
  const action = getAction(config, actionId);
  const sequence = getActionSequence(config, actionId);
  return {
    actionInstanceId: `action-${Math.max(0, Math.floor(atEpochMs)).toString(36)}-2`,
    actionId,
    requestId: `request-${Math.max(0, Math.floor(atEpochMs)).toString(36)}-1`,
    sourceEventId: "runtime-initialized",
    priority: action.priority,
    source: "system",
    startedAtEpochMs: atEpochMs,
    sequenceIndex: 0,
    frameNumber: sequence[0] ?? 1,
    loopCount: 0,
    firedMarkers: [],
    transactionLocked: false,
  };
}

export function getPetAutonomyDecisionRange(mode: PetBehaviorMode) {
  return petMotionProfiles[mode].autonomyIntervalMs ?? undefined;
}

function scheduleNextAutonomyDecision(
  state: PetBehaviorState,
  config: PetBehaviorRuntimeConfig,
  atEpochMs: number,
) {
  if (state.behaviorMode === "quiet" || config.stateMachine.autonomy?.enabled === false) {
    state.nextAutonomyDecisionAtEpochMs = undefined;
    return undefined;
  }
  const range = getPetAutonomyDecisionRange(state.behaviorMode);
  if (!range) {
    state.nextAutonomyDecisionAtEpochMs = undefined;
    return undefined;
  }
  const [min, max] = range;
  const nextAt = atEpochMs + randomBetween(state, min, max);
  state.nextAutonomyDecisionAtEpochMs = nextAt;
  return nextAt;
}

export function createInitialPetBehaviorState(config: PetBehaviorRuntimeConfig): PetBehaviorState {
  const atEpochMs = finiteOr(config.initialAtEpochMs, 0);
  const state: PetBehaviorState = {
    revision: 1,
    petId: config.petId,
    behaviorMode: config.behaviorMode ?? "gentle",
    reducedMotion: config.reducedMotion ?? false,
    layer: config.layer ?? "normal",
    direction: config.direction,
    velocity: config.velocity,
    current: createInitialInstance(config, atEpochMs),
    queue: [],
    returnStack: [],
    cooldowns: {},
    recentEventIds: [],
    recentAutonomousActions: [],
    lastInteractionAtEpochMs: atEpochMs,
    constraints: {
      focused: true,
      doNotDisturb: false,
      fullscreen: false,
      meeting: false,
      presentation: false,
      recording: false,
      dragging: false,
      voiceCapturing: false,
      permissionPending: false,
      fileTransactionActive: false,
    },
    needs: getInitialNeeds(config),
    randomState: normalizeSeed(config.randomSeed),
    nextOrdinal: 3,
  };
  scheduleNextAutonomyDecision(state, config, atEpochMs);
  return state;
}

export function createPetRuntimeSnapshot(state: PetBehaviorState): PetRuntimeSnapshot {
  return {
    revision: state.revision,
    petId: state.petId,
    actionInstanceId: state.current.actionInstanceId,
    actionId: state.current.actionId,
    transactionId: state.current.transactionId,
    startedAtEpochMs: state.current.startedAtEpochMs,
    direction: state.direction,
    velocity: state.velocity,
    reducedMotion: state.reducedMotion,
    layer: state.layer,
    needs: { ...state.needs },
    behaviorMode: state.behaviorMode,
    priority: state.current.priority,
    sequenceIndex: state.current.sequenceIndex,
    frameNumber: state.current.frameNumber,
    loopCount: state.current.loopCount,
    queueDepth: state.queue.length,
  };
}

function cooldownKey(petId: string, actionId: string) {
  return `${petId}::${actionId}`;
}

function isOnCooldown(state: PetBehaviorState, actionId: string, atEpochMs: number) {
  return (state.cooldowns[cooldownKey(state.petId, actionId)]?.untilEpochMs ?? 0) > atEpochMs;
}

function cleanupCooldowns(state: PetBehaviorState, atEpochMs: number) {
  const retained = Object.fromEntries(
    Object.entries(state.cooldowns).filter(([, cooldown]) => cooldown.untilEpochMs > atEpochMs),
  );
  if (Object.keys(retained).length === Object.keys(state.cooldowns).length) return false;
  state.cooldowns = retained;
  return true;
}

function sortQueue(queue: readonly QueuedPetAction[]) {
  return [...queue].sort(
    (left, right) =>
      right.priority - left.priority ||
      left.enqueuedAtEpochMs - right.enqueuedAtEpochMs ||
      left.requestId.localeCompare(right.requestId),
  );
}

function dropEffect(request: QueuedPetAction, reason: PetRequestDropReason): PetRuntimeEffect {
  return { type: "request_dropped", request: cloneRequest(request), reason };
}

function pruneQueue(
  state: PetBehaviorState,
  atEpochMs: number,
  effects: PetRuntimeEffect[],
) {
  let changed = false;
  const retained: QueuedPetAction[] = [];
  for (const request of state.queue) {
    if (request.expiresAtEpochMs !== undefined && request.expiresAtEpochMs <= atEpochMs) {
      effects.push(dropEffect(request, "expired"));
      changed = true;
      continue;
    }
    if (isOnCooldown(state, request.actionId, atEpochMs)) {
      effects.push(dropEffect(request, "cooldown"));
      changed = true;
      continue;
    }
    retained.push(request);
  }
  if (changed) state.queue = sortQueue(retained);
  return changed;
}

function removeQueuedRequests(
  state: PetBehaviorState,
  predicate: (request: QueuedPetAction) => boolean,
  reason: PetRequestDropReason,
  effects: PetRuntimeEffect[],
) {
  const retained: QueuedPetAction[] = [];
  let changed = false;
  for (const request of state.queue) {
    if (predicate(request)) {
      effects.push(dropEffect(request, reason));
      changed = true;
    } else {
      retained.push(request);
    }
  }
  if (changed) state.queue = sortQueue(retained);
  return changed;
}

function enqueueRequest(
  state: PetBehaviorState,
  config: PetBehaviorRuntimeConfig,
  request: QueuedPetAction,
  atEpochMs: number,
  effects: PetRuntimeEffect[],
) {
  pruneQueue(state, atEpochMs, effects);
  if (request.expiresAtEpochMs !== undefined && request.expiresAtEpochMs <= atEpochMs) {
    effects.push(dropEffect(request, "expired"));
    return false;
  }
  if (isOnCooldown(state, request.actionId, atEpochMs)) {
    effects.push(dropEffect(request, "cooldown"));
    return false;
  }
  const duplicate =
    (state.current.sourceEventId === request.sourceEventId && state.current.actionId === request.actionId) ||
    state.queue.some(
      (queued) =>
        queued.sourceEventId === request.sourceEventId && queued.actionId === request.actionId,
    );
  if (duplicate) {
    effects.push(dropEffect(request, "duplicate"));
    return false;
  }

  const coalescedSignals = new Set(config.coalescedSignals ?? DEFAULT_COALESCED_SIGNALS);
  if (request.signal && coalescedSignals.has(request.signal)) {
    removeQueuedRequests(
      state,
      (queued) => queued.signal === request.signal && queued.source === request.source,
      "duplicate",
      effects,
    );
  }
  if (request.source === "autonomy") {
    removeQueuedRequests(state, (queued) => queued.source === "autonomy", "duplicate", effects);
  }

  const sorted = sortQueue([...state.queue, cloneRequest(request)]);
  const retained = sorted.slice(0, getQueueLimit(config));
  const dropped = sorted.slice(getQueueLimit(config));
  state.queue = retained;
  for (const item of dropped) effects.push(dropEffect(item, "queue_full"));
  const wasRetained = retained.some((item) => item.requestId === request.requestId);
  if (wasRetained) effects.push({ type: "request_queued", request: cloneRequest(request) });
  return wasRetained;
}

function makeRequest(
  state: PetBehaviorState,
  config: PetBehaviorRuntimeConfig,
  actionId: string,
  sourceEventId: string,
  source: PetActionSource,
  atEpochMs: number,
  options: {
    signal?: string;
    payload?: Record<string, unknown>;
    transactionId?: string;
    expiresAtEpochMs?: number;
    chainId?: string;
    chainIndex?: number;
    chainActionIds?: readonly string[];
  } = {},
): QueuedPetAction {
  const action = getAction(config, actionId);
  return {
    requestId: allocateId(state, "request", atEpochMs),
    sourceEventId,
    actionId,
    priority: action?.priority ?? 0,
    source,
    enqueuedAtEpochMs: atEpochMs,
    expiresAtEpochMs: options.expiresAtEpochMs,
    payload: clonePayload(options.payload),
    transactionId: options.transactionId,
    signal: options.signal,
    chainId: options.chainId,
    chainIndex: options.chainIndex,
    chainLength: options.chainActionIds?.length,
    chainActionIds: options.chainActionIds ? [...options.chainActionIds] : undefined,
  };
}

function makeInstance(
  state: PetBehaviorState,
  config: PetBehaviorRuntimeConfig,
  request: QueuedPetAction,
  atEpochMs: number,
): PetActionInstance {
  const sequence = getActionSequence(config, request.actionId);
  return {
    actionInstanceId: allocateId(state, "action", atEpochMs),
    actionId: request.actionId,
    requestId: request.requestId,
    sourceEventId: request.sourceEventId,
    priority: request.priority,
    source: request.source,
    startedAtEpochMs: atEpochMs,
    sequenceIndex: 0,
    frameNumber: sequence[0] ?? 1,
    loopCount: 0,
    firedMarkers: [],
    payload: clonePayload(request.payload),
    transactionId: request.transactionId,
    expiresAtEpochMs: request.expiresAtEpochMs,
    signal: request.signal,
    chainId: request.chainId,
    chainIndex: request.chainIndex,
    chainLength: request.chainLength,
    chainActionIds: request.chainActionIds ? [...request.chainActionIds] : undefined,
    transactionLocked: false,
  };
}

function isReturnableAction(
  config: PetBehaviorRuntimeConfig,
  instance: PetActionInstance,
) {
  if (config.returnableActionIds) return config.returnableActionIds.includes(instance.actionId);
  if (instance.source === "autonomy" || instance.source === "pointer" || instance.source === "file") {
    return false;
  }
  const action = getAction(config, instance.actionId);
  if (!action || action.requiresPointerHold || action.mode === "once" || action.mode === "velocity_driven") {
    return false;
  }
  return true;
}

function pushReturnAction(
  state: PetBehaviorState,
  config: PetBehaviorRuntimeConfig,
  instance: PetActionInstance,
) {
  if (!isReturnableAction(config, instance)) return false;
  const item: PetReturnAction = {
    actionId: instance.actionId,
    source: instance.source,
    sourceEventId: instance.sourceEventId,
    payload: clonePayload(instance.payload),
    transactionId: instance.transactionId,
  };
  const withoutDuplicate = state.returnStack.filter((entry) => entry.actionId !== item.actionId);
  state.returnStack = [...withoutDuplicate, item].slice(-RETURN_STACK_LIMIT);
  return true;
}

function applyNeedEffect(
  state: PetBehaviorState,
  config: PetBehaviorRuntimeConfig,
  action: PetActionDefinition,
) {
  if (!action.needEffect) return false;
  const next = { ...state.needs };
  let changed = false;
  for (const [need, delta] of Object.entries(action.needEffect)) {
    const definition = config.stateMachine.needs?.[need];
    const current = finiteOr(next[need], definition?.initial ?? 0);
    const value = definition
      ? clamp(current + finiteOr(delta, 0), definition.min, definition.max)
      : current + finiteOr(delta, 0);
    if (value !== current) {
      next[need] = value;
      changed = true;
    }
  }
  if (changed) state.needs = next;
  return changed;
}

function establishCooldown(
  state: PetBehaviorState,
  config: PetBehaviorRuntimeConfig,
  instance: PetActionInstance,
  atEpochMs: number,
) {
  const action = getAction(config, instance.actionId);
  if (!action) return undefined;
  let durationMs = Math.max(0, finiteOr(action.cooldownMs, 0));
  if (action.randomCooldownMs && action.randomCooldownMs.length >= 2) {
    const min = Math.max(0, finiteOr(action.randomCooldownMs[0], 0));
    const max = Math.max(0, finiteOr(action.randomCooldownMs[1], min));
    durationMs = randomBetween(state, min, max);
  }
  if (durationMs <= 0) return undefined;
  const cooldown: PetActionCooldown = {
    petId: state.petId,
    actionId: instance.actionId,
    durationMs,
    startedAtEpochMs: atEpochMs,
    untilEpochMs: atEpochMs + durationMs,
  };
  state.cooldowns = {
    ...state.cooldowns,
    [cooldownKey(state.petId, instance.actionId)]: cooldown,
  };
  return cooldown;
}

function rememberAutonomousAction(
  state: PetBehaviorState,
  config: PetBehaviorRuntimeConfig,
  instance: PetActionInstance,
) {
  if (instance.source !== "autonomy") return false;
  const windowSize = Math.max(2, Math.floor(finiteOr(config.stateMachine.autonomy?.noRepeatWindow, 2)));
  state.recentAutonomousActions = [...state.recentAutonomousActions, instance.actionId].slice(-windowSize);
  return true;
}

function finishCurrent(
  state: PetBehaviorState,
  config: PetBehaviorRuntimeConfig,
  atEpochMs: number,
  reason: PetActionEndReason,
  effects: PetRuntimeEffect[],
) {
  const ended = cloneInstance(state.current);
  const cooldown = establishCooldown(state, config, ended, atEpochMs);
  if (reason === "completed" || reason === "marker_advanced") {
    const action = getAction(config, ended.actionId);
    if (action) applyNeedEffect(state, config, action);
  }
  rememberAutonomousAction(state, config, ended);
  effects.push({
    type: "action_ended",
    instance: ended,
    reason,
    cooldown: cooldown ? { ...cooldown } : undefined,
  });
  pruneQueue(state, atEpochMs, effects);
  return ended;
}

function startRequest(
  state: PetBehaviorState,
  config: PetBehaviorRuntimeConfig,
  request: QueuedPetAction,
  atEpochMs: number,
  effects: PetRuntimeEffect[],
) {
  const instance = makeInstance(state, config, request, atEpochMs);
  state.current = instance;
  effects.push({ type: "action_started", instance: cloneInstance(instance) });
}

function enqueueNextChainAction(
  state: PetBehaviorState,
  config: PetBehaviorRuntimeConfig,
  ended: PetActionInstance,
  atEpochMs: number,
  effects: PetRuntimeEffect[],
) {
  const chain = ended.chainActionIds;
  if (!chain?.length) return false;
  const currentIndex = ended.chainIndex ?? 0;
  for (let index = currentIndex + 1; index < chain.length; index += 1) {
    const actionId = chain[index];
    const request = makeRequest(state, config, actionId, ended.sourceEventId, ended.source, atEpochMs, {
      signal: ended.signal,
      payload: ended.payload,
      transactionId: ended.transactionId,
      expiresAtEpochMs: ended.expiresAtEpochMs,
      chainId: ended.chainId,
      chainIndex: index,
      chainActionIds: chain,
    });
    if (!getAction(config, actionId)) {
      effects.push(dropEffect(request, "unknown_action"));
      continue;
    }
    if (isOnCooldown(state, actionId, atEpochMs)) {
      effects.push(dropEffect(request, "cooldown"));
      continue;
    }
    return enqueueRequest(state, config, request, atEpochMs, effects);
  }
  return false;
}

function takeNextQueuedRequest(
  state: PetBehaviorState,
  atEpochMs: number,
  effects: PetRuntimeEffect[],
) {
  pruneQueue(state, atEpochMs, effects);
  const [first, ...rest] = state.queue;
  if (!first) return undefined;
  state.queue = rest;
  return first;
}

function takeReturnRequest(
  state: PetBehaviorState,
  config: PetBehaviorRuntimeConfig,
  atEpochMs: number,
) {
  const stack = [...state.returnStack];
  while (stack.length) {
    const item = stack.pop();
    if (!item || !getAction(config, item.actionId) || isOnCooldown(state, item.actionId, atEpochMs)) continue;
    state.returnStack = stack;
    return makeRequest(state, config, item.actionId, item.sourceEventId, item.source, atEpochMs, {
      payload: item.payload,
      transactionId: item.transactionId,
    });
  }
  state.returnStack = [];
  return undefined;
}

function makeFallbackRequest(
  state: PetBehaviorState,
  config: PetBehaviorRuntimeConfig,
  atEpochMs: number,
) {
  const actionId = getDefaultActionId(config);
  return makeRequest(
    state,
    config,
    actionId,
    allocateId(state, "fallback", atEpochMs),
    "system",
    atEpochMs,
  );
}

function advanceAfterEnd(
  state: PetBehaviorState,
  config: PetBehaviorRuntimeConfig,
  ended: PetActionInstance,
  atEpochMs: number,
  effects: PetRuntimeEffect[],
  advanceChain: boolean,
) {
  if (advanceChain) enqueueNextChainAction(state, config, ended, atEpochMs, effects);
  const next =
    takeNextQueuedRequest(state, atEpochMs, effects) ??
    takeReturnRequest(state, config, atEpochMs) ??
    makeFallbackRequest(state, config, atEpochMs);
  startRequest(state, config, next, atEpochMs, effects);
}

function finishAndAdvance(
  state: PetBehaviorState,
  config: PetBehaviorRuntimeConfig,
  atEpochMs: number,
  reason: PetActionEndReason,
  effects: PetRuntimeEffect[],
  advanceChain: boolean,
) {
  const ended = finishCurrent(state, config, atEpochMs, reason, effects);
  advanceAfterEnd(state, config, ended, atEpochMs, effects, advanceChain);
}

function canInterruptAtBoundary(
  state: PetBehaviorState,
  config: PetBehaviorRuntimeConfig,
  request: QueuedPetAction,
  boundary: InterruptBoundary,
) {
  const current = state.current;
  const currentAction = getAction(config, current.actionId);
  if (!currentAction || request.priority <= current.priority) return false;
  const maxAutonomousPriority = finiteOr(config.stateMachine.autonomy?.maxAutonomousPriority, 30);
  if (request.source === "autonomy" && current.priority > maxAutonomousPriority) return false;
  if (current.transactionLocked && request.source !== "file" && request.source !== "system") return false;
  if (currentAction.interrupt === "immediate") return true;
  if (currentAction.interrupt === "loop_boundary") return boundary === "loop";
  return currentAction.interrupt === "key_marker" && boundary === "marker";
}

function tryInterruptFromQueue(
  state: PetBehaviorState,
  config: PetBehaviorRuntimeConfig,
  atEpochMs: number,
  boundary: InterruptBoundary,
  effects: PetRuntimeEffect[],
) {
  pruneQueue(state, atEpochMs, effects);
  const request = state.queue[0];
  if (!request || !canInterruptAtBoundary(state, config, request, boundary)) return false;
  state.queue = state.queue.slice(1);
  pushReturnAction(state, config, state.current);
  finishCurrent(state, config, atEpochMs, "interrupted", effects);
  startRequest(state, config, request, atEpochMs, effects);
  return true;
}

function markerAdvanceMap(config: PetBehaviorRuntimeConfig) {
  return config.chainAdvanceMarkers ?? DEFAULT_CHAIN_ADVANCE_MARKERS;
}

function transactionLockMap(config: PetBehaviorRuntimeConfig) {
  return config.transactionLockMarkers ?? DEFAULT_TRANSACTION_LOCK_MARKERS;
}

function isChainAdvanceMarker(
  config: PetBehaviorRuntimeConfig,
  actionId: string,
  marker: string,
) {
  return markerAdvanceMap(config)[actionId]?.includes(marker) ?? false;
}

function emitMarker(
  state: PetBehaviorState,
  config: PetBehaviorRuntimeConfig,
  marker: string,
  frame: number,
  atEpochMs: number,
  effects: PetRuntimeEffect[],
) {
  const current = state.current;
  const expectedFrame = getAction(config, current.actionId)?.markers?.[marker];
  if (expectedFrame === undefined || expectedFrame !== frame || current.firedMarkers.includes(marker)) {
    return false;
  }
  current.firedMarkers = [...current.firedMarkers, marker];
  current.frameNumber = frame;
  const lockMarker = transactionLockMap(config)[current.actionId];
  if (lockMarker === marker) current.transactionLocked = true;
  const markerEvent: PetMarkerEvent = {
    actionInstanceId: current.actionInstanceId,
    actionId: current.actionId,
    marker,
    frame,
    eventId: allocateId(state, "marker", atEpochMs),
    transactionId: current.transactionId,
    atEpochMs,
  };
  effects.push({ type: "marker", marker: markerEvent });
  return true;
}

function firePendingMarkers(
  state: PetBehaviorState,
  config: PetBehaviorRuntimeConfig,
  atEpochMs: number,
  effects: PetRuntimeEffect[],
) {
  const action = getAction(config, state.current.actionId);
  let chainShouldAdvance = false;
  const markers = Object.entries(action?.markers ?? {}).sort(
    ([leftName, leftFrame], [rightName, rightFrame]) =>
      leftFrame - rightFrame || leftName.localeCompare(rightName),
  );
  for (const [marker, frame] of markers) {
    if (emitMarker(state, config, marker, frame, atEpochMs, effects)) {
      chainShouldAdvance ||= isChainAdvanceMarker(config, state.current.actionId, marker);
    }
  }
  return chainShouldAdvance;
}

function sourceForSignal(event: PetSignalEvent): PetActionSource {
  if (event.source) return event.source;
  if (
    event.signal.startsWith("file_") ||
    event.signal.startsWith("recycle_") ||
    event.signal === "confirmed_file_drop"
  ) {
    return "file";
  }
  if (event.signal.startsWith("voice_") || event.signal === "speech_started") return "voice";
  if (
    event.signal === "request_submitted" ||
    event.signal === "permission_required" ||
    event.signal.startsWith("task_")
  ) {
    return "agent";
  }
  if (
    event.signal.startsWith("pointer_") ||
    event.signal.includes("click") ||
    event.signal === "high_five_gesture" ||
    event.signal === "user_affection"
  ) {
    return "pointer";
  }
  return "system";
}

function isAttentionSuppressed(state: PetBehaviorState) {
  const constraints = state.constraints;
  return (
    constraints.doNotDisturb ||
    constraints.fullscreen ||
    constraints.meeting ||
    constraints.presentation ||
    constraints.recording
  );
}

function isAutonomyPaused(state: PetBehaviorState) {
  const constraints = state.constraints;
  return (
    state.behaviorMode === "quiet" ||
    isAttentionSuppressed(state) ||
    constraints.dragging ||
    constraints.voiceCapturing ||
    constraints.permissionPending ||
    constraints.fileTransactionActive ||
    state.current.transactionLocked
  );
}

function isTransactionResultSignal(config: PetBehaviorRuntimeConfig, signal: string) {
  return (config.transactionResultSignals ?? DEFAULT_TRANSACTION_RESULT_SIGNALS).includes(signal);
}

function transactionResultMatches(current: PetActionInstance, event: PetSignalEvent) {
  if (!current.transactionLocked) return false;
  if (current.transactionId && event.transactionId && current.transactionId !== event.transactionId) return false;
  return true;
}

function rememberEvent(state: PetBehaviorState, config: PetBehaviorRuntimeConfig, eventId: string) {
  state.recentEventIds = [...state.recentEventIds, eventId].slice(-getEventDedupeLimit(config));
}

function makeSignalChainRequests(
  state: PetBehaviorState,
  config: PetBehaviorRuntimeConfig,
  event: PetSignalEvent,
  source: PetActionSource,
  effects: PetRuntimeEffect[],
) {
  const mapped = config.stateMachine.signals[event.signal];
  if (!mapped?.length) return undefined;
  const eligible: string[] = [];
  for (const actionId of mapped) {
    const probe = makeRequest(state, config, actionId, event.eventId, source, event.atEpochMs, {
      signal: event.signal,
      payload: event.payload,
      transactionId: event.transactionId,
      expiresAtEpochMs: event.expiresAtEpochMs,
    });
    if (!getAction(config, actionId)) {
      effects.push(dropEffect(probe, "unknown_action"));
      continue;
    }
    if (isOnCooldown(state, actionId, event.atEpochMs)) {
      effects.push(dropEffect(probe, "cooldown"));
      continue;
    }
    eligible.push(actionId);
  }
  if (!eligible.length) return undefined;
  const chainId = allocateId(state, "chain", event.atEpochMs);
  return makeRequest(state, config, eligible[0], event.eventId, source, event.atEpochMs, {
    signal: event.signal,
    payload: event.payload,
    transactionId: event.transactionId,
    expiresAtEpochMs: event.expiresAtEpochMs,
    chainId,
    chainIndex: 0,
    chainActionIds: eligible,
  });
}

function handleSignal(
  state: PetBehaviorState,
  config: PetBehaviorRuntimeConfig,
  event: PetSignalEvent,
  effects: PetRuntimeEffect[],
) {
  if (event.petId !== state.petId) {
    effects.push({ type: "signal_ignored", event, reason: "pet_mismatch" });
    return false;
  }
  if (state.recentEventIds.includes(event.eventId)) {
    effects.push({ type: "signal_ignored", event, reason: "duplicate_event" });
    return false;
  }
  rememberEvent(state, config, event.eventId);
  const source = sourceForSignal(event);
  const isTransactionResult = isTransactionResultSignal(config, event.signal);
  const matchingTransactionResult = isTransactionResult && transactionResultMatches(state.current, event);

  if (isTransactionResult && state.current.transactionLocked && !matchingTransactionResult) {
    effects.push({ type: "signal_ignored", event, reason: "transaction_mismatch" });
    return true;
  }
  const quietSuppressed = new Set(config.quietSuppressedSignals ?? DEFAULT_QUIET_SUPPRESSED_SIGNALS);
  if (
    state.behaviorMode === "quiet" &&
    !event.essentialInQuietMode &&
    quietSuppressed.has(event.signal)
  ) {
    effects.push({ type: "signal_ignored", event, reason: "quiet_mode" });
    return true;
  }
  const interactionSuppressed =
    state.layer === "bottom" || !state.constraints.focused || state.constraints.doNotDisturb;
  if (!matchingTransactionResult && interactionSuppressed && (source === "pointer" || source === "file")) {
    effects.push({
      type: "signal_ignored",
      event,
      reason: state.layer === "bottom" ? "bottom_layer" : "context_suppressed",
    });
    return true;
  }
  if (event.signal === "rest_threshold_reached" && isAttentionSuppressed(state)) {
    effects.push({ type: "signal_ignored", event, reason: "attention_suppressed" });
    return true;
  }
  if (event.invalidateReturnActionIds?.length) {
    const invalidated = new Set(event.invalidateReturnActionIds);
    state.returnStack = state.returnStack.filter((item) => !invalidated.has(item.actionId));
  }
  if (source === "pointer" || source === "voice" || source === "agent" || source === "file") {
    state.lastInteractionAtEpochMs = event.atEpochMs;
  }
  const request = makeSignalChainRequests(state, config, event, source, effects);
  if (!config.stateMachine.signals[event.signal]) {
    effects.push({ type: "signal_ignored", event, reason: "unknown_signal" });
    return true;
  }
  if (request) enqueueRequest(state, config, request, event.atEpochMs, effects);

  if (matchingTransactionResult) {
    finishAndAdvance(state, config, event.atEpochMs, "external", effects, false);
    return true;
  }
  tryInterruptFromQueue(state, config, event.atEpochMs, "signal", effects);
  return true;
}

function findAutonomyPool(config: PetBehaviorRuntimeConfig, idleForMs: number) {
  const pools = config.stateMachine.autonomy?.pools ?? [];
  const eligible = pools.filter((pool) => {
    const min = finiteOr(pool.when.idleForMs[0], 0);
    const max = finiteOr(pool.when.idleForMs[1], Number.POSITIVE_INFINITY);
    return idleForMs >= Math.min(min, max) && idleForMs <= Math.max(min, max);
  });
  if (eligible.length) return eligible[eligible.length - 1];
  return [...pools]
    .sort((left, right) => finiteOr(left.when.idleForMs[0], 0) - finiteOr(right.when.idleForMs[0], 0))
    .filter((pool) => idleForMs >= finiteOr(pool.when.idleForMs[0], 0))
    .at(-1);
}

function normalizedNeedValue(
  state: PetBehaviorState,
  config: PetBehaviorRuntimeConfig,
  need: string,
) {
  const definition = config.stateMachine.needs?.[need];
  const value = finiteOr(state.needs[need], definition?.initial ?? 0);
  if (!definition || definition.max <= definition.min) return clamp(value / 100, 0, 1);
  return clamp((value - definition.min) / (definition.max - definition.min), 0, 1);
}

function needBiasMultiplier(
  state: PetBehaviorState,
  config: PetBehaviorRuntimeConfig,
  action: PetActionDefinition,
) {
  let multiplier = 1;
  for (const [need, coefficient] of Object.entries(action.needBias ?? {})) {
    const normalized = normalizedNeedValue(state, config, need);
    const amount = Math.abs(finiteOr(coefficient, 0));
    multiplier *= coefficient >= 0 ? 1 + amount * normalized : 1 + amount * (1 - normalized);
  }
  return multiplier;
}

function circadianMultiplier(
  config: PetBehaviorRuntimeConfig,
  actionId: string,
  hour: number,
) {
  for (const band of Object.values(config.stateMachine.autonomy?.circadian ?? {})) {
    const start = finiteOr(band.hours[0], 0);
    const end = finiteOr(band.hours[1], 24);
    const inBand = start <= end ? hour >= start && hour < end : hour >= start || hour < end;
    if (inBand && band.prefer?.includes(actionId)) return 1.35;
  }
  return 1;
}

function chooseAutonomousAction(
  state: PetBehaviorState,
  config: PetBehaviorRuntimeConfig,
  idleForMs: number,
  hour: number,
  atEpochMs: number,
) {
  const pool = findAutonomyPool(config, idleForMs);
  if (!pool) return undefined;
  const maxPriority = finiteOr(config.stateMachine.autonomy?.maxAutonomousPriority, 30);
  const recent = new Set(state.recentAutonomousActions);
  const weighted: { actionId: string; weight: number }[] = [];
  for (const [actionId, poolWeight] of Object.entries(pool.actions)) {
    const action = getAction(config, actionId);
    if (!action || action.priority > maxPriority || isOnCooldown(state, actionId, atEpochMs)) {
      continue;
    }
    if (recent.has(actionId) || state.current.actionId === actionId) continue;
    if (state.queue.some((request) => request.actionId === actionId)) continue;
    const manifestWeight = Math.max(0, finiteOr(action.weight, 1));
    const personality = Math.max(0, finiteOr(config.actionWeightModifiers?.[actionId], 1));
    const weight =
      Math.max(0, finiteOr(poolWeight, 0)) *
      manifestWeight *
      personality *
      needBiasMultiplier(state, config, action) *
      circadianMultiplier(config, actionId, hour);
    if (weight > 0) weighted.push({ actionId, weight });
  }
  const total = weighted.reduce((sum, item) => sum + item.weight, 0);
  if (total <= 0) return undefined;
  let roll = nextRandom(state) * total;
  for (const item of weighted) {
    roll -= item.weight;
    if (roll <= 0) return item.actionId;
  }
  return weighted.at(-1)?.actionId;
}

function handleTick(
  state: PetBehaviorState,
  config: PetBehaviorRuntimeConfig,
  atEpochMs: number,
  hour: number | undefined,
  effects: PetRuntimeEffect[],
) {
  let changed = cleanupCooldowns(state, atEpochMs);
  changed = pruneQueue(state, atEpochMs, effects) || changed;
  if (state.behaviorMode === "quiet" || config.stateMachine.autonomy?.enabled === false) {
    if (state.nextAutonomyDecisionAtEpochMs !== undefined) {
      state.nextAutonomyDecisionAtEpochMs = undefined;
      changed = true;
    }
    return changed;
  }
  if (state.nextAutonomyDecisionAtEpochMs === undefined) {
    scheduleNextAutonomyDecision(state, config, atEpochMs);
    return true;
  }
  if (atEpochMs < state.nextAutonomyDecisionAtEpochMs) return changed;

  const nextDecisionAt = scheduleNextAutonomyDecision(state, config, atEpochMs) ?? atEpochMs;
  changed = true;
  const idleForMs = Math.max(0, atEpochMs - state.lastInteractionAtEpochMs);
  const minimumIdle = Math.max(
    0,
    finiteOr(config.stateMachine.autonomy?.minimumIdleBeforeAutonomyMs, 10_000),
  );
  const maxPriority = finiteOr(config.stateMachine.autonomy?.maxAutonomousPriority, 30);
  let reason: string | undefined;
  if (idleForMs < minimumIdle) reason = "not_idle_long_enough";
  else if (isAutonomyPaused(state)) reason = "autonomy_paused";
  else if (state.current.priority > maxPriority) reason = "priority_blocked";
  else if (state.current.source === "autonomy" || state.queue.some((item) => item.source === "autonomy")) {
    reason = "autonomy_already_pending";
  }
  if (reason) {
    effects.push({ type: "autonomy_scheduled", nextDecisionAtEpochMs: nextDecisionAt, reason });
    return changed;
  }

  const localHour = hour ?? new Date(atEpochMs).getHours();
  const actionId = chooseAutonomousAction(state, config, idleForMs, localHour, atEpochMs);
  if (!actionId) {
    effects.push({
      type: "autonomy_scheduled",
      nextDecisionAtEpochMs: nextDecisionAt,
      reason: "no_candidate",
    });
    return changed;
  }
  const sourceEventId = allocateId(state, "autonomy", atEpochMs);
  const request = makeRequest(state, config, actionId, sourceEventId, "autonomy", atEpochMs, {
    signal: "autonomy_decision",
  });
  enqueueRequest(state, config, request, atEpochMs, effects);
  tryInterruptFromQueue(state, config, atEpochMs, "signal", effects);
  effects.push({ type: "autonomy_scheduled", actionId, nextDecisionAtEpochMs: nextDecisionAt });
  return true;
}

function handleContext(
  state: PetBehaviorState,
  config: PetBehaviorRuntimeConfig,
  event: Extract<PetBehaviorEvent, { type: "context" }>,
  effects: PetRuntimeEffect[],
) {
  const previousMode = state.behaviorMode;
  let changed = false;
  if (event.behaviorMode !== undefined && event.behaviorMode !== state.behaviorMode) {
    state.behaviorMode = event.behaviorMode;
    changed = true;
  }
  if (event.reducedMotion !== undefined && event.reducedMotion !== state.reducedMotion) {
    state.reducedMotion = event.reducedMotion;
    changed = true;
  }
  if (event.layer !== undefined && event.layer !== state.layer) {
    state.layer = event.layer;
    changed = true;
  }
  if (event.direction !== undefined && event.direction !== state.direction) {
    state.direction = event.direction;
    changed = true;
  }
  if (event.velocity !== undefined && event.velocity !== state.velocity) {
    state.velocity = event.velocity;
    changed = true;
  }
  if (event.constraints) {
    const constraints = { ...state.constraints, ...event.constraints };
    if (Object.keys(constraints).some((key) => constraints[key as keyof typeof constraints] !== state.constraints[key as keyof typeof constraints])) {
      state.constraints = constraints;
      changed = true;
    }
  }

  const autonomyMustStop = state.behaviorMode === "quiet" || isAutonomyPaused(state);
  if (autonomyMustStop) {
    changed =
      removeQueuedRequests(state, (request) => request.source === "autonomy", "context_suppressed", effects) ||
      changed;
  }
  const interactionsMustStop =
    state.layer === "bottom" || !state.constraints.focused || state.constraints.doNotDisturb;
  if (interactionsMustStop) {
    changed =
      removeQueuedRequests(
        state,
        (request) => request.source === "pointer" || request.source === "file",
        state.layer === "bottom" ? "bottom_layer" : "context_suppressed",
        effects,
      ) || changed;
  }

  const currentMustStop =
    (state.current.source === "autonomy" && autonomyMustStop) ||
    ((state.current.source === "pointer" || state.current.source === "file") && interactionsMustStop);
  if (currentMustStop && !state.current.transactionLocked) {
    finishAndAdvance(
      state,
      config,
      event.atEpochMs,
      previousMode !== state.behaviorMode ? "mode_changed" : "context_changed",
      effects,
      false,
    );
    changed = true;
  }

  if (state.behaviorMode === "quiet") {
    if (state.nextAutonomyDecisionAtEpochMs !== undefined) changed = true;
    state.nextAutonomyDecisionAtEpochMs = undefined;
  } else if (previousMode !== state.behaviorMode || state.nextAutonomyDecisionAtEpochMs === undefined) {
    scheduleNextAutonomyDecision(state, config, event.atEpochMs);
    changed = true;
  }
  return changed;
}

function handleNeeds(
  state: PetBehaviorState,
  config: PetBehaviorRuntimeConfig,
  values: Readonly<Record<string, number>>,
) {
  const next = { ...state.needs };
  let changed = false;
  for (const [need, rawValue] of Object.entries(values)) {
    const definition = config.stateMachine.needs?.[need];
    const value = definition
      ? clamp(finiteOr(rawValue, next[need] ?? definition.initial), definition.min, definition.max)
      : finiteOr(rawValue, next[need] ?? 0);
    if (next[need] !== value) {
      next[need] = value;
      changed = true;
    }
  }
  if (changed) state.needs = next;
  return changed;
}

function reduceInto(
  state: PetBehaviorState,
  config: PetBehaviorRuntimeConfig,
  event: PetBehaviorEvent,
  effects: PetRuntimeEffect[],
) {
  switch (event.type) {
    case "signal":
      return handleSignal(state, config, event.event, effects);
    case "tick":
      return handleTick(state, config, event.atEpochMs, event.hour, effects);
    case "frame": {
      if (event.actionInstanceId !== state.current.actionInstanceId) return false;
      const nextLoopCount = event.loopCount ?? state.current.loopCount;
      if (
        state.current.sequenceIndex === event.sequenceIndex &&
        state.current.frameNumber === event.frameNumber &&
        state.current.loopCount === nextLoopCount
      ) {
        return false;
      }
      state.current.sequenceIndex = Math.max(0, Math.floor(event.sequenceIndex));
      state.current.frameNumber = Math.max(1, Math.floor(event.frameNumber));
      state.current.loopCount = Math.max(0, Math.floor(nextLoopCount));
      return true;
    }
    case "marker": {
      if (event.actionInstanceId !== state.current.actionInstanceId) return false;
      const emitted = emitMarker(state, config, event.marker, event.frame, event.atEpochMs, effects);
      if (!emitted) return false;
      if (
        isChainAdvanceMarker(config, state.current.actionId, event.marker) &&
        (state.current.chainIndex ?? 0) + 1 < (state.current.chainActionIds?.length ?? 0)
      ) {
        finishAndAdvance(state, config, event.atEpochMs, "marker_advanced", effects, true);
      } else {
        tryInterruptFromQueue(state, config, event.atEpochMs, "marker", effects);
      }
      return true;
    }
    case "animation_end": {
      if (event.actionInstanceId !== state.current.actionInstanceId) return false;
      const chainShouldAdvance = firePendingMarkers(state, config, event.atEpochMs, effects);
      if (state.current.transactionLocked) return true;
      finishAndAdvance(
        state,
        config,
        event.atEpochMs,
        chainShouldAdvance ? "marker_advanced" : "completed",
        effects,
        true,
      );
      return true;
    }
    case "loop_boundary": {
      if (event.actionInstanceId !== state.current.actionInstanceId) return false;
      const nextLoopCount = Math.floor(event.loopCount ?? state.current.loopCount + 1);
      if (nextLoopCount <= state.current.loopCount) return false;
      state.current.loopCount = nextLoopCount;
      tryInterruptFromQueue(state, config, event.atEpochMs, "loop", effects);
      return true;
    }
    case "terminate": {
      if (event.actionInstanceId !== state.current.actionInstanceId) return false;
      if (state.current.transactionLocked && !event.force) return false;
      finishAndAdvance(
        state,
        config,
        event.atEpochMs,
        "external",
        effects,
        !event.invalidateChain,
      );
      return true;
    }
    case "context":
      return handleContext(state, config, event, effects);
    case "needs":
      return handleNeeds(state, config, event.values);
    case "invalidate_returns": {
      const invalidated = new Set(event.actionIds);
      const retained = state.returnStack.filter((item) => !invalidated.has(item.actionId));
      if (retained.length === state.returnStack.length) return false;
      state.returnStack = retained;
      return true;
    }
  }
}

export function reducePetBehavior(
  state: PetBehaviorState,
  event: PetBehaviorEvent,
  config: PetBehaviorRuntimeConfig,
): PetBehaviorTransition {
  const next = clonePetBehaviorState(state);
  const effects: PetRuntimeEffect[] = [];
  const changed = reduceInto(next, config, event, effects);
  if (!changed) {
    return { state, snapshot: createPetRuntimeSnapshot(state), effects };
  }
  next.revision = state.revision + 1;
  return { state: next, snapshot: createPetRuntimeSnapshot(next), effects };
}

export class PetBehaviorRuntime {
  readonly config: PetBehaviorRuntimeConfig;
  #state: PetBehaviorState;

  constructor(config: PetBehaviorRuntimeConfig, initialState?: PetBehaviorState) {
    this.config = config;
    this.#state = initialState
      ? clonePetBehaviorState(initialState)
      : createInitialPetBehaviorState(config);
  }

  getState() {
    return clonePetBehaviorState(this.#state);
  }

  getSnapshot() {
    return createPetRuntimeSnapshot(this.#state);
  }

  dispatch(event: PetBehaviorEvent) {
    const transition = reducePetBehavior(this.#state, event, this.config);
    this.#state = transition.state;
    return transition;
  }

  receiveSignal(event: PetSignalEvent) {
    return this.dispatch({ type: "signal", event });
  }

  tick(atEpochMs: number, hour?: number) {
    return this.dispatch({ type: "tick", atEpochMs, hour });
  }

  onFrame(
    actionInstanceId: string,
    sequenceIndex: number,
    frameNumber: number,
    atEpochMs: number,
    loopCount?: number,
  ) {
    return this.dispatch({
      type: "frame",
      actionInstanceId,
      sequenceIndex,
      frameNumber,
      loopCount,
      atEpochMs,
    });
  }

  onMarker(
    actionInstanceId: string,
    marker: string,
    frame: number,
    atEpochMs: number,
  ) {
    return this.dispatch({ type: "marker", actionInstanceId, marker, frame, atEpochMs });
  }

  onAnimationEnd(actionInstanceId: string, atEpochMs: number) {
    return this.dispatch({ type: "animation_end", actionInstanceId, atEpochMs });
  }

  onLoopBoundary(actionInstanceId: string, atEpochMs: number, loopCount?: number) {
    return this.dispatch({ type: "loop_boundary", actionInstanceId, atEpochMs, loopCount });
  }
}
