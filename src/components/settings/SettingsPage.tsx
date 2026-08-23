import { useEffect, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { invoke } from "@tauri-apps/api/core";
import { Database, Monitor, Moon, Power, Sun } from "lucide-react";
import { Switch } from "@/components/ui/switch";
import { Button } from "@/components/ui/button";
import { useTheme, type Theme } from "@/components/theme-provider";
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

  useEffect(() => {
    invoke<AppSettings>("get_app_settings")
      .then(setSettings)
      .catch((e) => onError(String(e)));
    getVersion()
      .then(setVersion)
      .catch(() => setVersion("0.1.0(浏览器演示)"));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const toggleAutostart = async (enabled: boolean) => {
    try {
      await invoke("set_autostart", { enabled });
      setSettings((s) => (s ? { ...s, autostart: enabled } : s));
      onSuccess(enabled ? "已开启开机自启" : "已关闭开机自启");
    } catch (e) {
      onError(String(e));
    }
  };

  const themeOptions: { value: Theme; label: string; icon: typeof Sun }[] = [
    { value: "light", label: "浅色", icon: Sun },
    { value: "dark", label: "深色", icon: Moon },
    { value: "system", label: "系统", icon: Monitor },
  ];

  return (
    <div className="space-y-4 max-w-2xl">
      <Row
        icon={<Power className="w-4 h-4" />}
        title="开机自启动"
        desc="登录后自动在后台运行,CLI 代理随时可用"
      >
        <Switch
          checked={settings?.autostart ?? false}
          disabled={!settings}
          onCheckedChange={(v) => void toggleAutostart(v)}
        />
      </Row>

      <Row
        icon={<Sun className="w-4 h-4" />}
        title="外观"
        desc="浅色 / 深色 / 跟随系统"
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
        icon={<Database className="w-4 h-4" />}
        title="数据位置"
        desc={settings ? `${settings.db_path}（SQLCipher 整库加密）` : "…"}
      >
        <span className="inline-flex items-center px-2 py-0.5 rounded-md text-xs font-medium text-emerald-600 bg-emerald-500/10 dark:text-emerald-400">
          已加密
        </span>
      </Row>

      <Row
        icon={<Monitor className="w-4 h-4" />}
        title="本地代理"
        desc={settings ? `http://${settings.proxy_addr}（接管后 CLI 流量经此转发）` : "…"}
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
          GitHub 仓库 · MIT
        </Button>
      </div>
    </div>
  );
}
