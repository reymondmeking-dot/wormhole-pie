export type PetBehaviorMode = "gentle" | "quiet" | "lively";

export type PetActionSource =
  | "system"
  | "file"
  | "agent"
  | "voice"
  | "pointer"
  | "autonomy";

export type PetSourceWindow = "main" | "pet" | "pet-dialogue" | "rust";
export type PetRuntimeLayer = "normal" | "top" | "bottom";
export type PetDirection = "left" | "right";

export type PetAnimationMode =
  | "once"
  | "loop"
  | "ping_pong"
  | "hold_last"
  | "hold_last_ping_pong"
  | "velocity_driven";

export type PetInterruptRule = "immediate" | "loop_boundary" | "key_marker";

export type PetNeedValues = Record<string, number>;

export type PetActionDefinition = {
  group?: string;
  priority: number;
  mode: PetAnimationMode;
  frames?: number;
  sequence?: readonly number[];
  entrySequence?: readonly number[];
  fps: number;
  interrupt: PetInterruptRule;
  cooldownMs?: number;
  randomCooldownMs?: readonly number[];
  markers?: Readonly<Record<string, number>>;
  autonomous?: boolean;
  weight?: number;
  needBias?: Readonly<Record<string, number>>;
  needEffect?: Readonly<Record<string, number>>;
  longWaitFallback?: string;
  requiresConfirmation?: boolean;
  requiresPointerHold?: boolean;
  directional?: boolean;
  mirrorSafe?: boolean;
  holdLastMs?: number;
};

export type PetActionsManifest = {
  version?: number;
  defaults?: {
    frames?: number;
    sequence?: readonly number[];
  };
  actions: Readonly<Record<string, PetActionDefinition>>;
};

export type PetAutonomyPool = {
  id: string;
  when: {
    idleForMs: readonly number[];
  };
  actions: Readonly<Record<string, number>>;
};

export type PetAutonomyManifest = {
  enabled?: boolean;
  schedulerTickMs?: number;
  decisionIntervalMs?: readonly number[];
  noRepeatWindow?: number;
  maxAutonomousPriority?: number;
  minimumIdleBeforeAutonomyMs?: number;
  pools?: readonly PetAutonomyPool[];
  circadian?: Readonly<
    Record<
      string,
      {
        hours: readonly number[];
        prefer?: readonly string[];
        [key: string]: unknown;
      }
    >
  >;
  memory?: {
    recentEvents?: number;
    [key: string]: unknown;
  };
};

export type PetNeedDefinition = {
  min: number;
  max: number;
  initial: number;
  [key: string]: number;
};

export type PetStateMachineManifest = {
  version?: number;
  default: string;
  signals: Readonly<Record<string, readonly string[]>>;
  globalRules?: readonly string[];
  autonomy?: PetAutonomyManifest;
  needs?: Readonly<Record<string, PetNeedDefinition>>;
};

export type PetRuntimeConstraints = {
  focused: boolean;
  doNotDisturb: boolean;
  fullscreen: boolean;
  meeting: boolean;
  presentation: boolean;
  recording: boolean;
  dragging: boolean;
  voiceCapturing: boolean;
  permissionPending: boolean;
  fileTransactionActive: boolean;
};

export type PetBehaviorRuntimeConfig = {
  actions: PetActionsManifest;
  stateMachine: PetStateMachineManifest;
  petId: string;
  initialAtEpochMs?: number;
  behaviorMode?: PetBehaviorMode;
  reducedMotion?: boolean;
  layer?: PetRuntimeLayer;
  direction?: PetDirection;
  velocity?: number;
  needs?: Readonly<PetNeedValues>;
  actionWeightModifiers?: Readonly<Record<string, number>>;
  queueLimit?: number;
  eventDedupeLimit?: number;
  randomSeed?: number;
  quietSuppressedSignals?: readonly string[];
  coalescedSignals?: readonly string[];
  returnableActionIds?: readonly string[];
  chainAdvanceMarkers?: Readonly<Record<string, readonly string[]>>;
  transactionLockMarkers?: Readonly<Record<string, string>>;
  transactionResultSignals?: readonly string[];
};

export type PetSignalEvent = {
  eventId: string;
  signal: string;
  sourceWindow: PetSourceWindow;
  atEpochMs: number;
  petId: string;
  payload?: Record<string, unknown>;
  transactionId?: string;
  source?: PetActionSource;
  expiresAtEpochMs?: number;
  essentialInQuietMode?: boolean;
  invalidateReturnActionIds?: readonly string[];
};

export type QueuedPetAction = {
  requestId: string;
  sourceEventId: string;
  actionId: string;
  priority: number;
  source: PetActionSource;
  enqueuedAtEpochMs: number;
  expiresAtEpochMs?: number;
  payload?: Record<string, unknown>;
  transactionId?: string;
  signal?: string;
  chainId?: string;
  chainIndex?: number;
  chainLength?: number;
  chainActionIds?: readonly string[];
};

export type PetActionInstance = {
  actionInstanceId: string;
  actionId: string;
  requestId: string;
  sourceEventId: string;
  priority: number;
  source: PetActionSource;
  startedAtEpochMs: number;
  sequenceIndex: number;
  frameNumber: number;
  loopCount: number;
  firedMarkers: readonly string[];
  payload?: Record<string, unknown>;
  transactionId?: string;
  expiresAtEpochMs?: number;
  signal?: string;
  chainId?: string;
  chainIndex?: number;
  chainLength?: number;
  chainActionIds?: readonly string[];
  transactionLocked: boolean;
};

export type PetReturnAction = {
  actionId: string;
  source: PetActionSource;
  sourceEventId: string;
  payload?: Record<string, unknown>;
  transactionId?: string;
};

export type PetActionCooldown = {
  actionId: string;
  petId: string;
  durationMs: number;
  startedAtEpochMs: number;
  untilEpochMs: number;
};

export type PetBehaviorState = {
  revision: number;
  petId: string;
  behaviorMode: PetBehaviorMode;
  reducedMotion: boolean;
  layer: PetRuntimeLayer;
  direction?: PetDirection;
  velocity?: number;
  current: PetActionInstance;
  queue: readonly QueuedPetAction[];
  returnStack: readonly PetReturnAction[];
  cooldowns: Readonly<Record<string, PetActionCooldown>>;
  recentEventIds: readonly string[];
  recentAutonomousActions: readonly string[];
  lastInteractionAtEpochMs: number;
  nextAutonomyDecisionAtEpochMs?: number;
  constraints: PetRuntimeConstraints;
  needs: Readonly<PetNeedValues>;
  randomState: number;
  nextOrdinal: number;
};

export type PetMarkerEvent = {
  actionInstanceId: string;
  actionId: string;
  marker: string;
  frame: number;
  eventId: string;
  transactionId?: string;
  atEpochMs: number;
};

export type PetRuntimeSnapshot = {
  revision: number;
  petId: string;
  actionInstanceId: string;
  actionId: string;
  transactionId?: string;
  startedAtEpochMs: number;
  direction?: PetDirection;
  velocity?: number;
  reducedMotion: boolean;
  layer: PetRuntimeLayer;
  needs: Readonly<PetNeedValues>;
  behaviorMode: PetBehaviorMode;
  priority: number;
  sequenceIndex: number;
  frameNumber: number;
  loopCount: number;
  queueDepth: number;
};

export type PetActionEndReason =
  | "completed"
  | "interrupted"
  | "marker_advanced"
  | "external"
  | "mode_changed"
  | "context_changed";

export type PetRequestDropReason =
  | "cooldown"
  | "duplicate"
  | "expired"
  | "queue_full"
  | "unknown_action"
  | "quiet_mode"
  | "bottom_layer"
  | "context_suppressed"
  | "no_repeat"
  | "priority_blocked";

export type PetRuntimeEffect =
  | { type: "action_started"; instance: PetActionInstance }
  | {
      type: "action_ended";
      instance: PetActionInstance;
      reason: PetActionEndReason;
      cooldown?: PetActionCooldown;
    }
  | { type: "marker"; marker: PetMarkerEvent }
  | { type: "request_queued"; request: QueuedPetAction }
  | { type: "request_dropped"; request: QueuedPetAction; reason: PetRequestDropReason }
  | { type: "signal_ignored"; event: PetSignalEvent; reason: string }
  | {
      type: "autonomy_scheduled";
      actionId?: string;
      nextDecisionAtEpochMs: number;
      reason?: string;
    };

export type PetBehaviorTransition = {
  state: PetBehaviorState;
  snapshot: PetRuntimeSnapshot;
  effects: readonly PetRuntimeEffect[];
};

export type PetBehaviorEvent =
  | { type: "signal"; event: PetSignalEvent }
  | { type: "tick"; atEpochMs: number; hour?: number }
  | {
      type: "frame";
      actionInstanceId: string;
      sequenceIndex: number;
      frameNumber: number;
      loopCount?: number;
      atEpochMs: number;
    }
  | {
      type: "marker";
      actionInstanceId: string;
      marker: string;
      frame: number;
      atEpochMs: number;
    }
  | { type: "animation_end"; actionInstanceId: string; atEpochMs: number }
  | {
      type: "loop_boundary";
      actionInstanceId: string;
      atEpochMs: number;
      loopCount?: number;
    }
  | {
      type: "terminate";
      actionInstanceId: string;
      atEpochMs: number;
      force?: boolean;
      invalidateChain?: boolean;
    }
  | {
      type: "context";
      atEpochMs: number;
      behaviorMode?: PetBehaviorMode;
      reducedMotion?: boolean;
      layer?: PetRuntimeLayer;
      direction?: PetDirection;
      velocity?: number;
      constraints?: Partial<PetRuntimeConstraints>;
    }
  | { type: "needs"; atEpochMs: number; values: Readonly<PetNeedValues> }
  | { type: "invalidate_returns"; atEpochMs: number; actionIds: readonly string[] };
