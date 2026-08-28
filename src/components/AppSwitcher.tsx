import { ProviderIcon } from "@/components/ProviderIcon";
import { ALL_APPS } from "@/lib/appPrefs";
import { cn } from "@/lib/utils";
import type { AppType } from "@/types";

// CLI 分组切换器;分组多时收起文字只留图标
const STORAGE_KEY = "conduit-last-app";

const appIconName: Record<AppType, string> = {
  claude: "claude",
  codex: "openai",
  gemini: "gemini",
  opencode: "opencode",
  openclaw: "openclaw",
  qwen: "qwen",
  iflow: "iflow",
  crush: "crush",
  droid: "droid",
};

const appDisplayName: Record<AppType, string> = {
  claude: "Claude",
  codex: "Codex",
  gemini: "Gemini",
  opencode: "OpenCode",
  openclaw: "OpenClaw",
  qwen: "Qwen Code",
  iflow: "iFlow",
  crush: "Crush",
  droid: "Droid",
};

export function AppSwitcher({
  activeApp,
  onSwitch,
  apps = ALL_APPS,
  compact = false,
}: {
  activeApp: AppType;
  onSwitch: (app: AppType) => void;
  /** 可见分组(按设置中的顺序);缺省为全部 */
  apps?: AppType[];
  /** 只显示图标(分组较多时节省顶栏空间) */
  compact?: boolean;
}) {
  const visible = apps.filter((a) => ALL_APPS.includes(a));
  // 分组不多时图标+文字;超过 5 个才收起文字只留图标
  const iconOnly = compact || visible.length > 5;
  const handleSwitch = (app: AppType) => {
    if (app === activeApp) return;
    localStorage.setItem(STORAGE_KEY, app);
    onSwitch(app);
  };
  const iconSize = 20;

  return (
    <div className="inline-flex bg-muted rounded-xl p-1 gap-1">
      {visible.map((app) => (
        <button
          key={app}
          type="button"
          onClick={() => handleSwitch(app)}
          title={`${appDisplayName[app]}(⌘${visible.indexOf(app) + 1})`}
          className={cn(
            "group inline-flex items-center h-8 rounded-md text-sm font-medium transition-all duration-200",
            iconOnly ? "w-9 justify-center" : "px-3",
            activeApp === app
              ? "bg-background text-foreground shadow-sm"
              : "text-muted-foreground hover:text-foreground hover:bg-background/50",
          )}
        >
          <ProviderIcon
            icon={appIconName[app]}
            name={appDisplayName[app]}
            size={iconSize}
          />
          <span
            className={cn(
              "transition-all duration-200 whitespace-nowrap overflow-hidden",
              iconOnly
                ? "max-w-0 opacity-0 ml-0"
                : "max-w-[80px] opacity-100 ml-2",
            )}
          >
            {appDisplayName[app]}
          </span>
        </button>
      ))}
    </div>
  );
}
