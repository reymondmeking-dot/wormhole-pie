import type { PetSpecies } from "../petProfile";

export const petSpriteActions = [
  "idle_breathe",
  "walk",
  "sleep",
  "click_feedback",
  "listen",
  "think",
  "task_success",
  "file_eat",
  "drag",
  "rest_reminder",
  "affection",
  "hide_peek",
  "blink",
  "ear_twitch",
  "tail_flick",
  "yawn",
  "idle_lookaround",
  "cursor_notice",
  "hover_interest",
  "typing_watch",
  "run",
  "hop",
  "edge_balance",
  "release_bounce",
  "double_click_excited",
  "pet_loop",
  "tickle_laugh",
  "high_five",
  "emotion_happy",
  "emotion_surprised",
  "emotion_sad",
  "emotion_angry",
  "stretch",
  "groom",
  "snack_eat",
  "dance",
  "voice_wake",
  "task_failure",
  "permission_wait",
  "speak_loop",
  "file_notice",
  "file_reject",
  "trash_success",
  "trash_fail_cough",
] as const;

export type PetSpriteAction = (typeof petSpriteActions)[number];
export type PetAnimationMode = "loop" | "ping_pong" | "once" | "hold_last" | "hold_last_ping_pong" | "velocity_driven";
export type PetInterruptMode = "immediate" | "loop_boundary" | "key_marker";

export type PetActionDefinition = Readonly<{
  id: PetSpriteAction;
  group: string;
  frames: number;
  fps: number;
  mode: PetAnimationMode;
  sequence: readonly number[];
  entrySequence: readonly number[];
  priority: number;
  interrupt: PetInterruptMode;
  markers: Readonly<Record<string, number>>;
  cooldownMs: number;
  randomCooldownMs?: readonly [number, number];
  holdLastMs?: number;
  longWaitFallback?: PetSpriteAction;
  autonomous: boolean;
  weight: number;
  directional: boolean;
  mirrorSafe: boolean;
  requiresConfirmation: boolean;
  requiresPointerHold: boolean;
  needBias: Readonly<Record<string, number>>;
  needEffect: Readonly<Record<string, number>>;
}>;

export type PetDescriptor = Readonly<{
  id: string;
  name: string;
  species: string;
  accent: string;
  canonicalMax: number;
  personality: string;
  needModifiers: Readonly<Record<string, number>>;
  actionWeightModifiers: Readonly<Partial<Record<PetSpriteAction, number>>>;
}>;

export type PetActionsManifest = Readonly<{
  version: number;
  framePattern: string;
  defaultFrames: number;
  defaultSequence: readonly number[];
  actions: Readonly<Record<PetSpriteAction, PetActionDefinition>>;
  proceduralLayers: Readonly<Record<string, unknown>>;
}>;

export type PetStateMachineManifest = Readonly<{
  version: number;
  defaultAction: PetSpriteAction;
  signals: Readonly<Record<string, readonly PetSpriteAction[]>>;
  globalRules: readonly string[];
  autonomy: Readonly<Record<string, unknown>>;
  needs: Readonly<Record<string, unknown>>;
}>;

export type PetManifestIndex = Readonly<{
  source: "manifest" | "fallback";
  revision: string;
  actionsManifest: PetActionsManifest;
  stateMachine: PetStateMachineManifest;
  pets: readonly PetDescriptor[];
  petsById: ReadonlyMap<string, PetDescriptor>;
}>;

const petSpriteActionSet = new Set<string>(petSpriteActions);
const animationModeSet = new Set<PetAnimationMode>([
  "loop",
  "ping_pong",
  "once",
  "hold_last",
  "hold_last_ping_pong",
  "velocity_driven",
]);
const interruptModeSet = new Set<PetInterruptMode>(["immediate", "loop_boundary", "key_marker"]);
const expectedPetIds = new Set(["nuonuo-corgi", "xingxing-cat", "taotao-bunny", "bobo-otter", "tangtang-bear"]);

const legacySpeciesToManifestSpecies: Record<PetSpecies, string> = {
  "star-cat": "cat",
  "moon-rabbit": "lop_bunny",
  corgi: "corgi",
  "river-otter": "river_otter",
  "bear-cub": "bear_cub",
  "cloud-fox": "corgi",
};

const fallbackAssetBySpecies: Record<PetSpecies, string> = {
  "star-cat": "xingxing-cat",
  "moon-rabbit": "taotao-bunny",
  corgi: "nuonuo-corgi",
  "river-otter": "bobo-otter",
  "bear-cub": "tangtang-bear",
  "cloud-fox": "nuonuo-corgi",
};

const baseUrl = import.meta.env.BASE_URL.replace(/\/?$/, "/");
export const petAssetRoot = `${baseUrl}pets/pai`;

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function asRecord(value: unknown, label: string): Record<string, unknown> {
  if (!isRecord(value)) throw new Error(`${label} 必须是对象`);
  return value;
}

function asString(value: unknown, label: string, fallback?: string) {
  if (typeof value === "string") return value;
  if (fallback !== undefined) return fallback;
  throw new Error(`${label} 必须是字符串`);
}

function asFiniteNumber(value: unknown, label: string, fallback?: number) {
  if (typeof value === "number" && Number.isFinite(value)) return value;
  if (fallback !== undefined) return fallback;
  throw new Error(`${label} 必须是有限数字`);
}

function asPositiveInteger(value: unknown, label: string, fallback?: number) {
  const number = asFiniteNumber(value, label, fallback);
  if (!Number.isInteger(number) || number <= 0) throw new Error(`${label} 必须是正整数`);
  return number;
}

function asNumberRecord(value: unknown, label: string): Readonly<Record<string, number>> {
  if (value === undefined) return Object.freeze({});
  const record = asRecord(value, label);
  const result: Record<string, number> = {};
  for (const [key, item] of Object.entries(record)) {
    result[key] = asFiniteNumber(item, `${label}.${key}`);
  }
  return Object.freeze(result);
}

function asFrameSequence(value: unknown, label: string): readonly number[] {
  if (!Array.isArray(value)) throw new Error(`${label} 必须是帧号数组`);
  const sequence = value.map((frame, index) => asPositiveInteger(frame, `${label}[${index}]`));
  if (sequence.length === 0) throw new Error(`${label} 不能为空`);
  return Object.freeze(sequence);
}

function isPetSpriteAction(value: unknown): value is PetSpriteAction {
  return typeof value === "string" && petSpriteActionSet.has(value);
}

export { isPetSpriteAction };

function maxReferencedFrame(action: Record<string, unknown>) {
  let maximum = 0;
  for (const key of ["sequence", "entrySequence"] as const) {
    const value = action[key];
    if (!Array.isArray(value)) continue;
    for (const frame of value) {
      if (typeof frame === "number" && Number.isInteger(frame) && frame > maximum) maximum = frame;
    }
  }
  if (isRecord(action.markers)) {
    for (const frame of Object.values(action.markers)) {
      if (typeof frame === "number" && Number.isInteger(frame) && frame > maximum) maximum = frame;
    }
  }
  return maximum;
}

function validateFrames(sequence: readonly number[], frames: number, label: string) {
  for (const frame of sequence) {
    if (frame > frames) throw new Error(`${label} 引用了不存在的第 ${frame} 帧（有效帧数 ${frames}）`);
  }
}

function normalizeActionsManifest(value: unknown): PetActionsManifest {
  const manifest = asRecord(value, "actions.json");
  const defaults = asRecord(manifest.defaults, "actions.json.defaults");
  const rawActions = asRecord(manifest.actions, "actions.json.actions");
  const defaultFramesValue = defaults.frames === undefined ? undefined : asPositiveInteger(defaults.frames, "actions.json.defaults.frames");
  const defaultSequenceValue = defaults.sequence === undefined
    ? undefined
    : asFrameSequence(defaults.sequence, "actions.json.defaults.sequence");

  const missingActions = petSpriteActions.filter((action) => !(action in rawActions));
  const unknownActions = Object.keys(rawActions).filter((action) => !petSpriteActionSet.has(action));
  if (missingActions.length > 0 || unknownActions.length > 0) {
    throw new Error(`动作清单与运行时类型不一致；缺少: ${missingActions.join(", ") || "无"}；未知: ${unknownActions.join(", ") || "无"}`);
  }

  const definitions = {} as Record<PetSpriteAction, PetActionDefinition>;
  for (const actionId of petSpriteActions) {
    const rawAction = asRecord(rawActions[actionId], `actions.json.actions.${actionId}`);
    const derivedFrames = maxReferencedFrame(rawAction);
    const frames = rawAction.frames === undefined
      ? defaultFramesValue ?? derivedFrames
      : asPositiveInteger(rawAction.frames, `actions.json.actions.${actionId}.frames`);
    if (frames <= 0) throw new Error(`${actionId} 无法推导有效帧数`);

    const sequence = rawAction.sequence === undefined
      ? defaultSequenceValue ?? Object.freeze(Array.from({ length: frames }, (_, index) => index + 1))
      : asFrameSequence(rawAction.sequence, `actions.json.actions.${actionId}.sequence`);
    const entrySequence = rawAction.entrySequence === undefined
      ? Object.freeze([] as number[])
      : asFrameSequence(rawAction.entrySequence, `actions.json.actions.${actionId}.entrySequence`);
    validateFrames(sequence, frames, `${actionId}.sequence`);
    validateFrames(entrySequence, frames, `${actionId}.entrySequence`);

    const mode = asString(rawAction.mode, `${actionId}.mode`) as PetAnimationMode;
    if (!animationModeSet.has(mode)) throw new Error(`${actionId}.mode 不受支持: ${mode}`);
    const interrupt = asString(rawAction.interrupt, `${actionId}.interrupt`, "immediate") as PetInterruptMode;
    if (!interruptModeSet.has(interrupt)) throw new Error(`${actionId}.interrupt 不受支持: ${interrupt}`);

    const markerRecord = rawAction.markers === undefined ? {} : asRecord(rawAction.markers, `${actionId}.markers`);
    const markers: Record<string, number> = {};
    for (const [marker, frameValue] of Object.entries(markerRecord)) {
      const frame = asPositiveInteger(frameValue, `${actionId}.markers.${marker}`);
      if (frame > frames) throw new Error(`${actionId}.markers.${marker} 引用了不存在的第 ${frame} 帧`);
      markers[marker] = frame;
    }

    let randomCooldownMs: readonly [number, number] | undefined;
    if (rawAction.randomCooldownMs !== undefined) {
      if (!Array.isArray(rawAction.randomCooldownMs) || rawAction.randomCooldownMs.length !== 2) {
        throw new Error(`${actionId}.randomCooldownMs 必须是 [min, max]`);
      }
      const minimum = asFiniteNumber(rawAction.randomCooldownMs[0], `${actionId}.randomCooldownMs[0]`);
      const maximum = asFiniteNumber(rawAction.randomCooldownMs[1], `${actionId}.randomCooldownMs[1]`);
      if (minimum < 0 || maximum < minimum) throw new Error(`${actionId}.randomCooldownMs 范围无效`);
      randomCooldownMs = Object.freeze([minimum, maximum] as const);
    }

    const fallback = rawAction.longWaitFallback;
    if (fallback !== undefined && !isPetSpriteAction(fallback)) {
      throw new Error(`${actionId}.longWaitFallback 引用了未知动作 ${String(fallback)}`);
    }

    definitions[actionId] = Object.freeze({
      id: actionId,
      group: asString(rawAction.group, `${actionId}.group`, "life"),
      frames,
      fps: Math.max(0.1, asFiniteNumber(rawAction.fps, `${actionId}.fps`, 6)),
      mode,
      sequence,
      entrySequence,
      priority: asFiniteNumber(rawAction.priority, `${actionId}.priority`, 0),
      interrupt,
      markers: Object.freeze(markers),
      cooldownMs: Math.max(0, asFiniteNumber(rawAction.cooldownMs, `${actionId}.cooldownMs`, 0)),
      randomCooldownMs,
      holdLastMs: rawAction.holdLastMs === undefined
        ? undefined
        : Math.max(0, asFiniteNumber(rawAction.holdLastMs, `${actionId}.holdLastMs`)),
      longWaitFallback: fallback,
      autonomous: rawAction.autonomous === true,
      weight: Math.max(0, asFiniteNumber(rawAction.weight, `${actionId}.weight`, 0)),
      directional: rawAction.directional === true,
      mirrorSafe: rawAction.mirrorSafe === true,
      requiresConfirmation: rawAction.requiresConfirmation === true,
      requiresPointerHold: rawAction.requiresPointerHold === true,
      needBias: asNumberRecord(rawAction.needBias, `${actionId}.needBias`),
      needEffect: asNumberRecord(rawAction.needEffect, `${actionId}.needEffect`),
    });
  }

  const framePattern = asString(manifest.framePattern, "actions.json.framePattern");
  for (const placeholder of ["{size}", "{pet}", "{action}", "{frame}"]) {
    if (!framePattern.includes(placeholder)) throw new Error(`actions.json.framePattern 缺少 ${placeholder}`);
  }

  const proceduralLayers: Readonly<Record<string, unknown>> = manifest.proceduralLayers === undefined
    ? Object.freeze({})
    : Object.freeze({ ...asRecord(manifest.proceduralLayers, "actions.json.proceduralLayers") });
  const blinkLayer = isRecord(proceduralLayers.blink_overlay) ? proceduralLayers.blink_overlay : undefined;
  if (blinkLayer?.fallbackAction !== undefined && !isPetSpriteAction(blinkLayer.fallbackAction)) {
    throw new Error(`blink_overlay.fallbackAction 引用了未知动作 ${String(blinkLayer.fallbackAction)}`);
  }

  const defaultFrames = defaultFramesValue ?? Math.max(...Object.values(definitions).map((definition) => definition.frames));
  const defaultSequence = defaultSequenceValue ?? Object.freeze(Array.from({ length: defaultFrames }, (_, index) => index + 1));
  validateFrames(defaultSequence, defaultFrames, "actions.json.defaults.sequence");

  return Object.freeze({
    version: asFiniteNumber(manifest.version, "actions.json.version", 1),
    framePattern,
    defaultFrames,
    defaultSequence,
    actions: Object.freeze(definitions),
    proceduralLayers,
  });
}

function normalizePetsManifest(value: unknown): readonly PetDescriptor[] {
  const manifest = asRecord(value, "pets.json");
  if (!Array.isArray(manifest.pets)) throw new Error("pets.json.pets 必须是数组");
  const pets = manifest.pets.map((item, index): PetDescriptor => {
    const pet = asRecord(item, `pets.json.pets[${index}]`);
    const id = asString(pet.id, `pets.json.pets[${index}].id`);
    const rawWeights = asNumberRecord(pet.actionWeightModifiers, `${id}.actionWeightModifiers`);
    const actionWeightModifiers: Partial<Record<PetSpriteAction, number>> = {};
    for (const [action, weight] of Object.entries(rawWeights)) {
      if (!isPetSpriteAction(action)) throw new Error(`${id}.actionWeightModifiers 引用了未知动作 ${action}`);
      actionWeightModifiers[action] = weight;
    }
    return Object.freeze({
      id,
      name: asString(pet.name, `${id}.name`, id),
      species: asString(pet.species, `${id}.species`),
      accent: asString(pet.accent, `${id}.accent`, "#8067ff"),
      canonicalMax: asFiniteNumber(pet.canonicalMax, `${id}.canonicalMax`, 360),
      personality: asString(pet.personality, `${id}.personality`, ""),
      needModifiers: asNumberRecord(pet.needModifiers, `${id}.needModifiers`),
      actionWeightModifiers: Object.freeze(actionWeightModifiers),
    });
  });

  const ids = new Set(pets.map((pet) => pet.id));
  const missing = [...expectedPetIds].filter((id) => !ids.has(id));
  const unexpected = [...ids].filter((id) => !expectedPetIds.has(id));
  if (pets.length !== expectedPetIds.size || missing.length > 0 || unexpected.length > 0) {
    throw new Error(`pets.json 角色集合无效；缺少: ${missing.join(", ") || "无"}；未知: ${unexpected.join(", ") || "无"}`);
  }
  return Object.freeze(pets);
}

function normalizeStateMachine(value: unknown): PetStateMachineManifest {
  const manifest = asRecord(value, "state-machine.json");
  const rawSignals = asRecord(manifest.signals, "state-machine.json.signals");
  const signals: Record<string, readonly PetSpriteAction[]> = {};
  for (const [signal, rawChain] of Object.entries(rawSignals)) {
    if (!Array.isArray(rawChain) || rawChain.length === 0) throw new Error(`signal ${signal} 必须映射到非空动作链`);
    signals[signal] = Object.freeze(rawChain.map((action, index) => {
      if (!isPetSpriteAction(action)) throw new Error(`signal ${signal}[${index}] 引用了未知动作 ${String(action)}`);
      return action;
    }));
  }
  const defaultAction = manifest.default;
  if (!isPetSpriteAction(defaultAction)) throw new Error(`state-machine.json.default 引用了未知动作 ${String(defaultAction)}`);
  const globalRules = Array.isArray(manifest.globalRules)
    ? Object.freeze(manifest.globalRules.map((rule, index) => asString(rule, `globalRules[${index}]`)))
    : Object.freeze([] as string[]);
  return Object.freeze({
    version: asFiniteNumber(manifest.version, "state-machine.json.version", 1),
    defaultAction,
    signals: Object.freeze(signals),
    globalRules,
    autonomy: Object.freeze({ ...asRecord(manifest.autonomy, "state-machine.json.autonomy") }),
    needs: Object.freeze({ ...asRecord(manifest.needs, "state-machine.json.needs") }),
  });
}

const fallbackDefinition = Object.freeze({
  id: "idle_breathe" as PetSpriteAction,
  group: "life",
  frames: 4,
  fps: 2,
  mode: "loop" as PetAnimationMode,
  sequence: Object.freeze([1, 2, 3, 2]),
  entrySequence: Object.freeze([] as number[]),
  priority: 10,
  interrupt: "immediate" as PetInterruptMode,
  markers: Object.freeze({}),
  cooldownMs: 0,
  autonomous: false,
  weight: 0,
  directional: false,
  mirrorSafe: false,
  requiresConfirmation: false,
  requiresPointerHold: false,
  needBias: Object.freeze({}),
  needEffect: Object.freeze({}),
});

const fallbackDefinitions = Object.freeze(Object.fromEntries(
  petSpriteActions.map((action) => [action, Object.freeze({ ...fallbackDefinition, id: action })]),
) as Record<PetSpriteAction, PetActionDefinition>);

// Kept as a live binding for existing scheduler imports. The authoritative
// definitions replace this safe fallback as soon as the manifests validate.
export let petActionDefinitions: Readonly<Record<PetSpriteAction, PetActionDefinition>> = fallbackDefinitions;

const fallbackActionsManifest: PetActionsManifest = Object.freeze({
  version: 0,
  framePattern: "{pet}/{action}/{frame}.png?size={size}",
  defaultFrames: 4,
  defaultSequence: fallbackDefinition.sequence,
  actions: fallbackDefinitions,
  proceduralLayers: Object.freeze({}),
});

const fallbackStateMachine: PetStateMachineManifest = Object.freeze({
  version: 0,
  defaultAction: "idle_breathe",
  signals: Object.freeze({}),
  globalRules: Object.freeze([]),
  autonomy: Object.freeze({}),
  needs: Object.freeze({}),
});

const fallbackManifestIndex: PetManifestIndex = Object.freeze({
  source: "fallback",
  revision: "fallback:0",
  actionsManifest: fallbackActionsManifest,
  stateMachine: fallbackStateMachine,
  pets: Object.freeze([]),
  petsById: new Map<string, PetDescriptor>(),
});

let currentManifestIndex = fallbackManifestIndex;
let manifestPromise: Promise<PetManifestIndex> | undefined;

async function fetchJson(name: "actions.json" | "pets.json" | "state-machine.json") {
  const response = await fetch(`${petAssetRoot}/${name}`, { cache: "force-cache" });
  if (!response.ok) throw new Error(`${name} 加载失败（HTTP ${response.status}）`);
  return response.json() as Promise<unknown>;
}

export function getPetManifestSnapshot() {
  return currentManifestIndex;
}

export function loadPetManifests(): Promise<PetManifestIndex> {
  if (manifestPromise) return manifestPromise;
  manifestPromise = Promise.all([
    fetchJson("actions.json"),
    fetchJson("pets.json"),
    fetchJson("state-machine.json"),
  ]).then(([actionsValue, petsValue, stateMachineValue]) => {
    const actionsManifest = normalizeActionsManifest(actionsValue);
    const pets = normalizePetsManifest(petsValue);
    const stateMachine = normalizeStateMachine(stateMachineValue);
    const petsById = new Map(pets.map((pet) => [pet.id, pet]));
    currentManifestIndex = Object.freeze({
      source: "manifest" as const,
      revision: `${actionsManifest.version}:${stateMachine.version}:${pets.length}`,
      actionsManifest,
      stateMachine,
      pets,
      petsById,
    });
    petActionDefinitions = actionsManifest.actions;
    return currentManifestIndex;
  }).catch((error: unknown) => {
    console.error("桌宠 manifest 校验失败，已安全回退到 idle_breathe。", error);
    if (import.meta.env.DEV) throw error;
    currentManifestIndex = fallbackManifestIndex;
    petActionDefinitions = fallbackDefinitions;
    return fallbackManifestIndex;
  });
  return manifestPromise;
}

export function getPetActionDefinition(action: PetSpriteAction, index = currentManifestIndex) {
  return index.actionsManifest.actions[action] ?? index.actionsManifest.actions.idle_breathe;
}

export function getPetAssetId(species: PetSpecies, index = currentManifestIndex) {
  const manifestSpecies = legacySpeciesToManifestSpecies[species];
  return index.pets.find((pet) => pet.species === manifestSpecies)?.id ?? fallbackAssetBySpecies[species];
}

function expandFramePattern(
  framePattern: string,
  replacements: { size: number; pet: string; action: PetSpriteAction; frame: number },
) {
  const expanded = framePattern
    .replaceAll("{size}", String(replacements.size))
    .replaceAll("{pet}", replacements.pet)
    .replaceAll("{action}", replacements.action)
    .replaceAll("{frame}", String(replacements.frame).padStart(2, "0"));
  if (/\{(?:size|pet|action|frame)\}/.test(expanded)) throw new Error(`framePattern 未完整替换: ${expanded}`);
  return expanded.replaceAll("\\", "/");
}

function packagedFramePath(expandedPattern: string, size: number) {
  const sourceFramesMarker = `/frames/${size}/`;
  const markerIndex = expandedPattern.indexOf(sourceFramesMarker);
  if (markerIndex >= 0) return expandedPattern.slice(markerIndex + sourceFramesMarker.length);
  return expandedPattern.replace(/^\.\.\/(?:frames\/\d+\/)?/, "").replace(/^\.\//, "").replace(/^\//, "");
}

export function getPetFrameSrc(
  species: PetSpecies,
  action: PetSpriteAction,
  frame: number,
  index = currentManifestIndex,
  size = 256,
) {
  const definition = getPetActionDefinition(action, index);
  const safeFrame = Math.max(1, Math.min(definition.frames, Math.round(frame)));
  const expanded = expandFramePattern(index.actionsManifest.framePattern, {
    size,
    pet: getPetAssetId(species, index),
    action,
    frame: safeFrame,
  });
  return `${petAssetRoot}/${packagedFramePath(expanded, size)}`;
}
