import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useState,
} from "react";
import { invoke } from "@tauri-apps/api/core";
import { en, zh, type Dict, type DictKey } from "@/i18n/dict";

export type LocaleSetting = "zh" | "en" | "system";
const STORAGE_KEY = "keyway-locale";
const LEGACY_STORAGE_KEY = "conduit-locale";

/** 解析 system 的实际语言 */
function systemLocale(): "zh" | "en" {
  return navigator.language.toLowerCase().startsWith("zh") ? "zh" : "en";
}

interface I18nValue {
  locale: "zh" | "en";
  setting: LocaleSetting;
  setSetting: (s: LocaleSetting) => void;
  t: (key: DictKey, vars?: Record<string, string | number>) => string;
}

const I18nContext = createContext<I18nValue>({
  locale: "zh",
  setting: "system",
  setSetting: () => {},
  t: (k) => zh[k],
});

const DICTS: Record<"zh" | "en", Dict> = { zh, en };

export function I18nProvider({ children }: { children: React.ReactNode }) {
  const [setting, setSettingState] = useState<LocaleSetting>(() => {
    // URL ?lang=zh|en 强制(演示/截图);?lang=system 恢复跟随
    const q = new URLSearchParams(location.search).get("lang");
    if (q === "zh" || q === "en") return q;
    let saved = localStorage.getItem(STORAGE_KEY) as LocaleSetting | null;
    if (saved === null) {
      // 旧键迁移:老版本用户的语言偏好不丢
      saved = localStorage.getItem(LEGACY_STORAGE_KEY) as LocaleSetting | null;
    }
    return saved === "zh" || saved === "en" || saved === "system"
      ? saved
      : "system";
  });

  const locale = setting === "system" ? systemLocale() : setting;

  useEffect(() => {
    document.documentElement.lang = locale;
    localStorage.setItem(STORAGE_KEY, setting);
  }, [setting, locale]);

  const t = useCallback(
    (key: DictKey, vars?: Record<string, string | number>) => {
      let s = DICTS[locale][key] ?? zh[key] ?? key;
      if (vars) {
        for (const [k, v] of Object.entries(vars)) {
          s = s.split(`{${k}}`).join(String(v));
        }
      }
      return s;
    },
    [locale],
  );

  const setSetting = useCallback((s: LocaleSetting) => {
    setSettingState(s);
    // 同步 Rust 侧(托盘菜单语言);演示/浏览器模式静默失败
    void invoke("set_locale", { locale: s }).catch(() => {});
  }, []);

  return (
    <I18nContext.Provider value={{ locale, setting, setSetting, t }}>
      {children}
    </I18nContext.Provider>
  );
}

export function useI18n() {
  return useContext(I18nContext);
}
