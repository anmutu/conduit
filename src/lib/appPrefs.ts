import type { AppType } from "@/types";

/**
 * 界面偏好(localStorage):布局模式 + CLI 分组的显示与顺序。
 * 与后端无关,纯前端偏好,损坏时静默回退默认值。
 */

export type LayoutMode = "side" | "top" | "right" | "bottom";

const LAYOUT_KEY = "conduit-layout";
const APPS_KEY = "conduit-apps";

export const ALL_LAYOUTS: LayoutMode[] = ["side", "top", "right", "bottom"];

/** 全部分组(含扩充;顺序即默认排序) */
export const ALL_APPS: AppType[] = [
  "claude",
  "codex",
  "gemini",
  "opencode",
  "openclaw",
  "qwen",
  "iflow",
  "crush",
  "droid",
];

/** 默认显示的分组(扩充组默认隐藏,可在设置中开启) */
export const DEFAULT_APPS: AppType[] = ALL_APPS.slice(0, 5);

export function loadLayout(): LayoutMode {
  try {
    const v = localStorage.getItem(LAYOUT_KEY) as LayoutMode | null;
    return v && ALL_LAYOUTS.includes(v) ? v : "side";
  } catch {
    return "side";
  }
}

export function saveLayout(mode: LayoutMode) {
  try {
    localStorage.setItem(LAYOUT_KEY, mode);
  } catch {
    /* ignore */
  }
}

/** 可见分组(有序);存储缺失/非法时回退默认 5 个 */
export function loadApps(): AppType[] {
  try {
    const raw = localStorage.getItem(APPS_KEY);
    if (!raw) return DEFAULT_APPS;
    const list = JSON.parse(raw) as string[];
    const valid = ALL_APPS.filter((a) => list.includes(a));
    return valid.length > 0 ? valid : DEFAULT_APPS;
  } catch {
    return DEFAULT_APPS;
  }
}

export function saveApps(apps: AppType[]) {
  try {
    localStorage.setItem(APPS_KEY, JSON.stringify(apps));
  } catch {
    /* ignore */
  }
}

/** 预设选择器排序偏好:类别顺序 + 各分组内平台顺序(按名称) */
export interface PresetOrderPrefs {
  /** 类别(大版块)顺序 */
  categories: string[];
  /** appId → 平台名称顺序 */
  presets: Record<string, string[]>;
}

const PRESET_KEY = "conduit-preset-order";

export function loadPresetOrder(): PresetOrderPrefs {
  try {
    const raw = localStorage.getItem(PRESET_KEY);
    if (!raw) return { categories: [], presets: {} };
    const v = JSON.parse(raw) as Partial<PresetOrderPrefs>;
    return {
      categories: Array.isArray(v.categories) ? v.categories : [],
      presets: v.presets && typeof v.presets === "object" ? v.presets : {},
    };
  } catch {
    return { categories: [], presets: {} };
  }
}

export function savePresetOrder(prefs: PresetOrderPrefs) {
  try {
    localStorage.setItem(PRESET_KEY, JSON.stringify(prefs));
  } catch {
    /* ignore */
  }
}
