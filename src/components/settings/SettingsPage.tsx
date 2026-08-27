import { useEffect, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { invoke } from "@tauri-apps/api/core";
import {
  ChevronDown,
  ChevronUp,
  CalendarClock,
  Database,
  Languages,
  LayoutGrid,
  Monitor,
  Moon,
  PanelLeft,
  PanelRight,
  PanelTop,
  PanelBottom,
  Power,
  Save,
  Layers,
  Sun,
} from "lucide-react";
import { Switch } from "@/components/ui/switch";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import { ProviderIcon } from "@/components/ProviderIcon";
import { RouteRulesCard } from "@/components/settings/RouteRulesCard";
import { useTheme, type Theme } from "@/components/theme-provider";
import { useI18n, type LocaleSetting } from "@/i18n";
import { FlagCN, FlagGB, GlobeAuto } from "@/components/LanguageBadges";
import { cn } from "@/lib/utils";
import { ALL_APPS, type LayoutMode } from "@/lib/appPrefs";
import type { AppType } from "@/types";

interface AppSettings {
  autostart: boolean;
  retention_days?: number;
  days_since_backup?: number | null;
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
  desc: React.ReactNode;
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
  layout,
  onLayoutChange,
  apps,
  onAppsChange,
}: {
  onError: (msg: string) => void;
  onSuccess: (msg: string) => void;
  /** 界面偏好(前端 localStorage) */
  layout: LayoutMode;
  onLayoutChange: (m: LayoutMode) => void;
  apps: AppType[];
  onAppsChange: (apps: AppType[]) => void;
}) {
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [version, setVersion] = useState("…");
  const [backupBusy, setBackupBusy] = useState(false);
  // Profile(供应商组合快照)
  const [profileList, setProfileList] = useState<string[]>([]);
  const [activeProfile, setActiveProfile] = useState("");
  const [profileName, setProfileName] = useState("");
  useEffect(() => {
    invoke<string[]>("list_profiles").then(setProfileList).catch(() => {});
  }, []);
  // 分组拖拽排序状态
  const [dragApp, setDragApp] = useState<AppType | null>(null);
  const [dragOver, setDragOver] = useState<AppType | null>(null);
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

  const saveRetention = async (days: number) => {
    if (!Number.isInteger(days) || days < 1 || days > 365) {
      onError(t("settings.retentionBad"));
      return;
    }
    try {
      await invoke("set_usage_retention", { days });
      setSettings((s) => (s ? { ...s, retention_days: days } : s));
      onSuccess(t("settings.retentionSaved", { n: days }));
    } catch (e) {
      onError(String(e));
    }
  };

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

  /** 分组管理:上移/下移(仅可见项之间)与显示开关 */
  const moveApp = (app: AppType, dir: -1 | 1) => {
    const idx = apps.indexOf(app);
    const target = idx + dir;
    if (target < 0 || target >= apps.length) return;
    const next = [...apps];
    [next[idx], next[target]] = [next[target], next[idx]];
    onAppsChange(next);
  };
  const toggleApp = (app: AppType, visible: boolean) => {
    if (visible) {
      // 恢复显示:插回默认顺序位置
      const next = ALL_APPS.filter((a) => a === app || apps.includes(a));
      onAppsChange(next);
    } else {
      if (apps.length <= 1) return; // 至少保留一个分组
      onAppsChange(apps.filter((a) => a !== app));
    }
  };

  const layoutOptions: { value: LayoutMode; label: string; icon: typeof PanelLeft }[] = [
    { value: "side", label: t("settings.layoutSide"), icon: PanelLeft },
    { value: "right", label: t("settings.layoutRight"), icon: PanelRight },
    { value: "top", label: t("settings.layoutTop"), icon: PanelTop },
    { value: "bottom", label: t("settings.layoutBottom"), icon: PanelBottom },
  ];

  return (
    <div className="space-y-4 w-full">
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
        icon={<CalendarClock className="w-4 h-4" />}
        title={t("settings.retention")}
        desc={t("settings.retentionDesc")}
      >
        <Input
          type="number"
          min={1}
          max={365}
          defaultValue={settings?.retention_days ?? 30}
          key={settings?.retention_days}
          onBlur={(e) => void saveRetention(Number(e.target.value))}
          onKeyDown={(e) => {
            if (e.key === "Enter") e.currentTarget.blur();
          }}
          className="w-20 h-8 text-xs tabular-nums text-right"
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
            { value: "zh", label: t("settings.langZh"), Flag: FlagCN },
            { value: "en", label: t("settings.langEn"), Flag: FlagGB },
            { value: "system", label: t("settings.langSystem"), Flag: GlobeAuto },
          ] as { value: LocaleSetting; label: string; Flag: () => React.JSX.Element }[]).map(({ value, label, Flag }) => (
            <button
              key={value}
              type="button"
              onClick={() => setLocale(value)}
              className={cn(
                "inline-flex items-center gap-1.5 px-2.5 h-7 rounded-md text-xs font-medium transition-all",
                localeSetting === value
                  ? "bg-background text-foreground shadow-sm"
                  : "text-muted-foreground hover:text-foreground",
              )}
            >
              <Flag />
              {label}
            </button>
          ))}
        </div>
      </Row>

      {/* 界面布局:左侧边栏 / 顶部横向 */}
      <Row
        icon={<PanelLeft className="w-4 h-4" />}
        title={t("settings.layout")}
        desc={t("settings.layoutDesc")}
      >
        <div className="flex items-center gap-1 p-1 bg-muted rounded-lg">
          {layoutOptions.map(({ value, label, icon: Icon }) => (
            <button
              key={value}
              type="button"
              onClick={() => onLayoutChange(value)}
              className={cn(
                "inline-flex items-center gap-1 px-2.5 h-7 rounded-md text-xs font-medium transition-all",
                layout === value
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

      {/* 分组管理:显示/隐藏 + 排序 */}
      <div className="rounded-xl border border-border bg-card p-4">
        <div className="flex items-start gap-3">
          <div className="mt-0.5 text-muted-foreground">
            <LayoutGrid className="w-4 h-4" />
          </div>
          <div className="flex-1 min-w-0">
            <div className="text-sm font-medium">{t("settings.appsRow")}</div>
            <div className="text-xs text-muted-foreground mb-3">
              {t("settings.appsRowDesc")}
            </div>
            <div className="space-y-1.5">
              {ALL_APPS.map((app) => {
                const visible = apps.includes(app);
                const vIdx = apps.indexOf(app);
                return (
                  <div
                    key={app}
                    draggable={visible}
                    onDragStart={() => setDragApp(app)}
                    onDragEnd={() => {
                      setDragApp(null);
                      setDragOver(null);
                    }}
                    onDragOver={(e) => {
                      if (!dragApp || !visible || dragApp === app) return;
                      e.preventDefault();
                      setDragOver(app);
                    }}
                    onDrop={(e) => {
                      e.preventDefault();
                      if (dragApp && visible && dragApp !== app) {
                        const next = apps.filter((a) => a !== dragApp);
                        next.splice(apps.indexOf(app), 0, dragApp);
                        onAppsChange(next);
                      }
                      setDragApp(null);
                      setDragOver(null);
                    }}
                    className={cn(
                      "flex items-center gap-2 rounded-lg border border-border px-3 py-1.5",
                      !visible && "opacity-50",
                      dragApp === app && "opacity-40",
                      dragOver === app && dragApp !== app && "border-blue-500/60 ring-1 ring-blue-500/40",
                      visible && "cursor-grab active:cursor-grabbing",
                    )}
                  >
                    <ProviderIcon icon={app} name={app} size={16} />
                    <span className="flex-1 text-sm capitalize">{app}</span>
                    <div className="flex items-center gap-0.5">
                      <Button
                        variant="ghost"
                        size="icon"
                        className="h-6 w-6"
                        disabled={!visible || vIdx === 0}
                        onClick={() => moveApp(app, -1)}
                        title={t("settings.appUp")}
                      >
                        <ChevronUp className="w-3.5 h-3.5" />
                      </Button>
                      <Button
                        variant="ghost"
                        size="icon"
                        className="h-6 w-6"
                        disabled={!visible || vIdx === apps.length - 1}
                        onClick={() => moveApp(app, 1)}
                        title={t("settings.appDown")}
                      >
                        <ChevronDown className="w-3.5 h-3.5" />
                      </Button>
                      <Switch
                        checked={visible}
                        onCheckedChange={(v) => toggleApp(app, v)}
                        disabled={!visible && apps.length <= 1}
                      />
                    </div>
                  </div>
                );
              })}
            </div>
          </div>
        </div>
      </div>

      {/* 模型路由:按模型名自动分流到指定供应商 */}
      <RouteRulesCard onError={onError} />

      <Row
        icon={<Database className="w-4 h-4" />}
        title={t("settings.data")}
        desc={settings ? t("settings.dataDesc", { path: settings.db_path }) : "…"}
      >
        <span className="inline-flex items-center px-2 py-0.5 rounded-md text-xs font-medium text-emerald-600 bg-emerald-500/10 dark:text-emerald-400">
          已加密
        </span>
      </Row>

      {/* Profile:供应商组合快照,一键切换工作/个人 */}
      <Row
        icon={<Layers className="w-4 h-4" />}
        title={t("profile.row")}
        desc={t("profile.rowDesc")}
      >
        <div className="flex items-center gap-2">
          <select
            value={activeProfile}
            onChange={(e) => setActiveProfile(e.target.value)}
            className="h-8 rounded-md border border-border bg-background px-2 text-xs max-w-[120px]"
          >
            <option value="">{t("profile.pick")}</option>
            {profileList.map((n) => (
              <option key={n} value={n}>{n}</option>
            ))}
          </select>
          <Input
            value={profileName}
            onChange={(e) => setProfileName(e.target.value)}
            placeholder={t("profile.namePh")}
            className="h-8 w-24 text-xs"
          />
          <Button
            size="sm"
            variant="outline"
            className="h-8"
            disabled={!profileName.trim()}
            onClick={async () => {
              try {
                const n = await invoke<number>("save_profile", {
                  name: profileName.trim(),
                });
                onSuccess(t("profile.saved", { n }));
                setProfileName("");
                setProfileList(await invoke<string[]>("list_profiles"));
                setActiveProfile(profileName.trim());
              } catch (e) {
                onError(String(e));
              }
            }}
          >
            {t("profile.save")}
          </Button>
          <Button
            size="sm"
            className="h-8"
            disabled={!activeProfile}
            onClick={async () => {
              try {
                const n = await invoke<number>("apply_profile", {
                  name: activeProfile,
                });
                onSuccess(t("profile.applied", { n }));
              } catch (e) {
                onError(String(e));
              }
            }}
          >
            {t("profile.apply")}
          </Button>
          <Button
            size="sm"
            variant="ghost"
            className="h-8 hover:bg-red-500/15 hover:text-red-500"
            disabled={!activeProfile}
            onClick={async () => {
              try {
                await invoke("delete_profile", { name: activeProfile });
                setProfileList(await invoke<string[]>("list_profiles"));
                setActiveProfile("");
                onSuccess(t("profile.deleted"));
              } catch (e) {
                onError(String(e));
              }
            }}
          >
            {t("common.delete")}
          </Button>
        </div>
      </Row>

      {/* 备份/恢复:供应商配置导出 JSON(Key 不导出),同名跳过导入 */}
      <Row
        icon={<Save className="w-4 h-4" />}
        title={t("settings.backupRow")}
        desc={
          <>
            {t("settings.backupRowDesc")}
            {settings &&
              (settings.days_since_backup == null ||
                settings.days_since_backup >= 7) && (
                <span className="block mt-1 text-amber-600 dark:text-amber-400">
                  {settings.days_since_backup == null
                    ? t("settings.backupNever")
                    : t("settings.backupStale", { n: settings.days_since_backup })}
                </span>
              )}
          </>
        }
      >
        <div className="flex items-center gap-2">
          <Button
            size="sm"
            variant="outline"
            disabled={backupBusy}
            onClick={async () => {
              setBackupBusy(true);
              try {
                const r = await invoke<{ path: string; count: number }>(
                  "export_backup",
                );
                onSuccess(t("settings.backupDone", { n: r.count, path: r.path }));
              } catch (e) {
                onError(String(e));
              } finally {
                setBackupBusy(false);
              }
            }}
          >
            {t("settings.backupExport")}
          </Button>
          <Button
            size="sm"
            variant="outline"
            disabled={backupBusy}
            onClick={async () => {
              setBackupBusy(true);
              try {
                const [created, skipped] = await invoke<[number, number]>(
                  "import_backup",
                );
                onSuccess(t("settings.restoreDone", { c: created, s: skipped }));
              } catch (e) {
                onError(String(e));
              } finally {
                setBackupBusy(false);
              }
            }}
          >
            {t("settings.backupImport")}
          </Button>
        </div>
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
