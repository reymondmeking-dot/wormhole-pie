export type PetSpecies = "star-cat" | "moon-rabbit" | "corgi" | "river-otter" | "bear-cub" | "cloud-fox";
export type PetTheme = "lilac" | "mint" | "peach";
export type EvolutionPath = "companion" | "creator" | "guardian";
export type PetMotionMode = "gentle" | "quiet" | "lively";

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
  motionMode: "wormhole-pie.pet.motionMode.v1",
  dialogueEnabled: "wormhole-pie.dialogue.enabled.v1",
  voiceEnabled: "wormhole-pie.voice.enabled.v1",
} as const;
