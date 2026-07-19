export type PetSpecies = "star-cat" | "moon-rabbit" | "corgi" | "river-otter" | "bear-cub" | "cloud-fox";
export type PetTheme = "lilac" | "mint" | "peach";
export type EvolutionPath = "companion" | "creator" | "guardian";
export type EvolutionStage = "seedling" | "growing" | "evolved";
export type PetMotionMode = "gentle" | "quiet" | "lively";

export type PetEvolutionMetrics = {
  completedTodos: number;
  organizedFiles: number;
  agentSuccesses: number;
  interactions: number;
  feedCount: number;
  activeDays: string[];
};

export const defaultPetEvolutionMetrics: PetEvolutionMetrics = {
  completedTodos: 0,
  organizedFiles: 0,
  agentSuccesses: 0,
  interactions: 0,
  feedCount: 0,
  activeDays: [],
};

export const evolutionStageThresholds = {
  growing: 35,
  evolved: 120,
} as const;

function safeMetric(value: number) {
  return Number.isFinite(value) ? Math.max(0, Math.floor(value)) : 0;
}

export function normalizePetEvolutionMetrics(value: unknown): PetEvolutionMetrics {
  const saved = typeof value === "object" && value !== null ? value as Partial<PetEvolutionMetrics> : {};
  return {
    completedTodos: safeMetric(saved.completedTodos ?? 0),
    organizedFiles: safeMetric(saved.organizedFiles ?? 0),
    agentSuccesses: safeMetric(saved.agentSuccesses ?? 0),
    interactions: safeMetric(saved.interactions ?? 0),
    feedCount: safeMetric(saved.feedCount ?? 0),
    activeDays: Array.isArray(saved.activeDays)
      ? [...new Set(saved.activeDays.filter((day): day is string => typeof day === "string" && Boolean(day.trim())).map((day) => day.trim()))]
      : [],
  };
}

export function mergePetEvolutionMetrics(_current: PetEvolutionMetrics, incoming: PetEvolutionMetrics) {
  return normalizePetEvolutionMetrics(incoming);
}

export function calculatePetEvolution(metrics: PetEvolutionMetrics) {
  const normalized = normalizePetEvolutionMetrics(metrics);
  const completedTodos = normalized.completedTodos;
  const organizedFiles = normalized.organizedFiles;
  const agentSuccesses = normalized.agentSuccesses;
  const interactions = normalized.interactions;
  const feedCount = normalized.feedCount;
  const activeDays = new Set(normalized.activeDays).size;
  const scores: Record<EvolutionPath, number> = {
    companion: interactions * 2 + feedCount * 3 + activeDays * 2,
    creator: agentSuccesses * 5 + interactions + activeDays,
    guardian: organizedFiles + completedTodos * 4 + activeDays * 2,
  };
  const path = (Object.entries(scores) as Array<[EvolutionPath, number]>).sort((left, right) => right[1] - left[1])[0][0];
  const points = completedTodos * 4 + organizedFiles + agentSuccesses * 5
    + interactions + feedCount * 3 + activeDays * 2;
  const stage: EvolutionStage = points >= evolutionStageThresholds.evolved
    ? "evolved"
    : points >= evolutionStageThresholds.growing
      ? "growing"
      : "seedling";
  const stageStart = stage === "seedling" ? 0 : stage === "growing" ? evolutionStageThresholds.growing : evolutionStageThresholds.evolved;
  const nextTarget = stage === "seedling"
    ? evolutionStageThresholds.growing
    : evolutionStageThresholds.evolved;
  const stageSpan = stage === "evolved" ? 1 : nextTarget - stageStart;
  const stagePoints = stage === "evolved" ? 1 : Math.min(stageSpan, Math.max(0, points - stageStart));
  return {
    path,
    stage,
    points,
    nextTarget,
    stageStart,
    stagePoints,
    stageSpan,
    stageProgress: stagePoints / stageSpan,
    scores,
  };
}

export type PetMotionProfile = Readonly<{
  autonomyIntervalMs: readonly [number, number] | null;
  actionRate: number;
  idlePauseMs: number;
}>;

export const petMotionProfiles: Record<PetMotionMode, PetMotionProfile> = {
  gentle: { autonomyIntervalMs: [45_000, 90_000], actionRate: 0.9, idlePauseMs: 7_000 },
  quiet: { autonomyIntervalMs: null, actionRate: 0.8, idlePauseMs: 12_000 },
  lively: { autonomyIntervalMs: [25_000, 50_000], actionRate: 1, idlePauseMs: 3_000 },
};

export const petSpeciesOptions: Array<{ value: PetSpecies; label: string; detail: string }> = [
  { value: "corgi", label: "糯糯", detail: "热情、治愈" },
  { value: "star-cat", label: "星星", detail: "安静、黏人" },
  { value: "moon-rabbit", label: "桃桃", detail: "轻快、好奇" },
  { value: "river-otter", label: "波波", detail: "聪明、活泼" },
  { value: "bear-cub", label: "糖糖", detail: "温柔、可靠" },
];

export const petThemeOptions: Array<{ value: PetTheme; label: string }> = [
  { value: "lilac", label: "星雾紫" },
  { value: "mint", label: "薄荷青" },
  { value: "peach", label: "蜜桃橘" },
];

export const evolutionPathOptions: Array<{ value: EvolutionPath; label: string; detail: string }> = [
  { value: "companion", label: "陪伴系", detail: "更多回应与休息动作" },
  { value: "creator", label: "灵感系", detail: "更多创作与庆祝动作" },
  { value: "guardian", label: "守护系", detail: "更多整理与专注动作" },
];

export const evolutionStageOptions: Array<{ value: EvolutionStage; label: string; detail: string }> = [
  { value: "seedling", label: "幼苗", detail: "基础外观与轻量动作" },
  { value: "growing", label: "成长", detail: "轮廓、配饰与动作增强" },
  { value: "evolved", label: "进化", detail: "完整路线外观与动作表现" },
];

export const evolutionBadges: Record<EvolutionPath, string> = {
  companion: "伴",
  creator: "灵",
  guardian: "守",
};

export const petMotionOptions: Array<{ value: PetMotionMode; label: string; detail: string }> = [
  {
    value: "gentle",
    label: "温柔低频",
    detail: `身体动作约 ${petMotionProfiles.gentle.autonomyIntervalMs![0] / 1000}–${petMotionProfiles.gentle.autonomyIntervalMs![1] / 1000} 秒一次，待机呼吸约 8 秒一轮`,
  },
  { value: "quiet", label: "安静陪伴", detail: "不主动做身体动作，只保留视线与任务回应" },
  {
    value: "lively",
    label: "轻快陪伴",
    detail: `身体动作约 ${petMotionProfiles.lively.autonomyIntervalMs![0] / 1000}–${petMotionProfiles.lively.autonomyIntervalMs![1] / 1000} 秒一次，仍避免连续切换`,
  },
];

export const petClassName = (species: PetSpecies, theme: PetTheme, evolution: EvolutionPath) =>
  `pet-species-${species} pet-theme-${theme} pet-evolution-${evolution}`;

export const petStorageKeys = {
  name: "wormhole-pie.pet.name.v1",
  species: "wormhole-pie.pet.species.v1",
  theme: "wormhole-pie.pet.theme.v1",
  evolution: "wormhole-pie.pet.evolution.v1",
  autoEvolution: "wormhole-pie.pet.autoEvolution.v1",
  manualEvolutionStage: "wormhole-pie.pet.manualEvolutionStage.v1",
  evolutionMetrics: "wormhole-pie.pet.evolutionMetrics.v1",
  motionMode: "wormhole-pie.pet.motionMode.v1",
  dialogueEnabled: "wormhole-pie.dialogue.enabled.v1",
  voiceEnabled: "wormhole-pie.voice.enabled.v1",
} as const;
