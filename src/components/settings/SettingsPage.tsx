import { useEffect, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { invoke } from "@tauri-apps/api/core";
import { Database, Languages, Monitor, Moon, Power, Sun } from "lucide-react";
import { Switch } from "@/components/ui/switch";
import { Button } from "@/components/ui/button";
import { useTheme, type Theme } from "@/components/theme-provider";
import { useI18n, type LocaleSetting } from "@/i18n";
import { cn } from "@/lib/utils";

interface AppSettings {
  autostart: boolean;
  db_path: string;
  proxy_addr: string;
}

function Row({
  icon,
  title,
  desc,
  children,
}: {
  icon: React.ReactNode;
  title: string;
  desc: string;
  children: React.ReactNode;
}) {
  return (
    <div className="flex items-center justify-between gap-4 rounded-xl border border-border p-4 bg-card">
      <div className="flex items-start gap-3 min-w-0">
        <div className="mt-0.5 text-muted-foreground">{icon}</div>
        <div className="min-w-0">
          <div className="text-sm font-medium">{title}</div>
          <div className="text-xs text-muted-foreground break-all">{desc}</div>
        </div>
      </div>
      <div className="shrink-0">{children}</div>
    </div>
  );
}

export function SettingsPage({
  onError,
  onSuccess,
}: {
  onError: (msg: string) => void;
  onSuccess: (msg: string) => void;
}) {
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [version, setVersion] = useState("…");
  const { theme, setTheme } = useTheme();
  const { t, setting: localeSetting, setSetting: setLocale } = useI18n();

  useEffect(() => {
    invoke<AppSettings>("get_app_settings")
      .then(setSettings)
      .catch((e) => onError(String(e)));
    getVersion()
      .then(setVersion)
      .catch(() => setVersion(t("about.demoVersion")));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const toggleAutostart = async (enabled: boolean) => {
    try {
      await invoke("set_autostart", { enabled });
      setSettings((s) => (s ? { ...s, autostart: enabled } : s));
      onSuccess(enabled ? t("settings.autostartOn") : t("settings.autostartOff"));
    } catch (e) {
      onError(String(e));
    }
  };

  const themeOptions: { value: Theme; label: string; icon: typeof Sun }[] = [
    { value: "light", label: t("settings.themeLight"), icon: Sun },
    { value: "dark", label: t("settings.themeDark"), icon: Moon },
    { value: "system", label: t("settings.themeSystem"), icon: Monitor },
  ];

  return (
    <div className="space-y-4 max-w-2xl">
      <Row
        icon={<Power className="w-4 h-4" />}
        title={t("settings.autostart")}
        desc={t("settings.autostartDesc")}
      >
        <Switch
          checked={settings?.autostart ?? false}
          disabled={!settings}
          onCheckedChange={(v) => void toggleAutostart(v)}
        />
      </Row>

      <Row
        icon={<Sun className="w-4 h-4" />}
        title={t("settings.appearance")}
        desc={t("settings.appearanceDesc")}
      >
        <div className="flex items-center gap-1 p-1 bg-muted rounded-lg">
          {themeOptions.map(({ value, label, icon: Icon }) => (
            <button
              key={value}
              type="button"
              onClick={() => setTheme(value)}
              className={cn(
                "inline-flex items-center gap-1 px-2.5 h-7 rounded-md text-xs font-medium transition-all",
                theme === value
                  ? "bg-background text-foreground shadow-sm"
                  : "text-muted-foreground hover:text-foreground",
              )}
            >
              <Icon className="w-3.5 h-3.5" />
              {label}
            </button>
          ))}
        </div>
      </Row>

      <Row
        icon={<Languages className="w-4 h-4" />}
        title={t("settings.language")}
        desc={t("settings.languageDesc")}
      >
        <div className="flex items-center gap-1 p-1 bg-muted rounded-lg">
          {([
            { value: "zh", label: t("settings.langZh") },
            { value: "en", label: t("settings.langEn") },
            { value: "system", label: t("settings.langSystem") },
          ] as { value: LocaleSetting; label: string }[]).map(({ value, label }) => (
            <button
              key={value}
              type="button"
              onClick={() => setLocale(value)}
              className={cn(
                "px-2.5 h-7 rounded-md text-xs font-medium transition-all",
                localeSetting === value
                  ? "bg-background text-foreground shadow-sm"
                  : "text-muted-foreground hover:text-foreground",
              )}
            >
              {label}
            </button>
          ))}
        </div>
      </Row>

      <Row
        icon={<Database className="w-4 h-4" />}
        title={t("settings.data")}
        desc={settings ? t("settings.dataDesc", { path: settings.db_path }) : "…"}
      >
        <span className="inline-flex items-center px-2 py-0.5 rounded-md text-xs font-medium text-emerald-600 bg-emerald-500/10 dark:text-emerald-400">
          已加密
        </span>
      </Row>

      <Row
        icon={<Monitor className="w-4 h-4" />}
        title={t("settings.proxyRow")}
        desc={settings ? t("settings.proxyRowDesc", { addr: settings.proxy_addr }) : "…"}
      >
        <span className="text-xs text-muted-foreground">v{version}</span>
      </Row>

      <div className="flex justify-end pt-2">
        <Button
          variant="link"
          className="text-xs"
          onClick={() =>
            window.open("https://github.com/anmutu/conduit", "_blank")
          }
        >
          {t("github.repo")}
        </Button>
      </div>
    </div>
  );
}
