import { BarChart3, Settings as SettingsIcon } from "lucide-react";
import { AppSwitcher } from "@/components/AppSwitcher";
import { ModeToggle } from "@/components/mode-toggle";
import { Button } from "@/components/ui/button";
import { useI18n } from "@/i18n";
import { cn } from "@/lib/utils";
import type { AppType } from "@/types";
import type { View } from "@/components/Sidebar";

/**
 * 横向切换栏:顶部(top)/底部(bottom)布局共用。
 * 左侧 CLI 分组切换,右侧用量/设置/代理/主题。
 */
export function AppHeaderBar({
  position,
  apps,
  activeApp,
  currentView,
  onViewChange,
  onSwitchApp,
  proxyOk,
  proxyAddr,
  onTakeover,
  isMac,
}: {
  position: "top" | "bottom";
  apps: AppType[];
  activeApp: AppType;
  currentView: View;
  onViewChange: (v: View) => void;
  onSwitchApp: (app: AppType) => void;
  proxyOk: boolean | null;
  proxyAddr: string;
  onTakeover: () => void;
  isMac: boolean;
}) {
  const { t } = useI18n();

  return (
    <header
      className={cn(
        "shrink-0 z-50 w-full h-14 flex items-center gap-3 bg-background/80 backdrop-blur-md",
        position === "top" ? "border-b border-border" : "border-t border-border",
      )}
      data-tauri-drag-region
    >
      <div
        className={cn(
          "flex items-center gap-3 flex-1 min-w-0",
          position === "top" && isMac && "pl-[92px]",
          position === "bottom" && "pl-3",
        )}
        data-tauri-no-drag
      >
        <AppSwitcher
          activeApp={activeApp}
          apps={apps}
          onSwitch={(app) => {
            onSwitchApp(app);
            onViewChange("providers");
          }}
        />
      </div>
      <div className="flex items-center gap-1.5 pr-3" data-tauri-no-drag>
        <Button
          variant="ghost"
          size="icon"
          className={cn("h-8 w-8", currentView === "usage" && "bg-accent")}
          title={t("dash.title")}
          onClick={() => onViewChange("usage")}
        >
          <BarChart3 className="w-4 h-4" />
        </Button>
        <Button
          variant="ghost"
          size="icon"
          className={cn("h-8 w-8", currentView === "settings" && "bg-accent")}
          title={t("common.settings")}
          onClick={() => onViewChange("settings")}
        >
          <SettingsIcon className="w-4 h-4" />
        </Button>
        {proxyOk !== null && (
          <button
            type="button"
            onClick={onTakeover}
            className={cn(
              "inline-flex items-center gap-1.5 px-2 py-1 rounded-md text-xs font-medium transition-opacity hover:opacity-80",
              proxyOk
                ? "text-emerald-600 dark:text-emerald-400 bg-emerald-500/10"
                : "text-red-600 dark:text-red-400 bg-red-500/10",
            )}
            title={
              proxyOk
                ? t("takeover.proxyTipOn", { addr: proxyAddr })
                : t("takeover.proxyTipOff")
            }
          >
            <span
              className={cn(
                "h-1.5 w-1.5 rounded-full",
                proxyOk ? "bg-emerald-500" : "bg-red-500",
              )}
            />
            {proxyOk ? t("takeover.proxyOn") : t("takeover.proxyOff")}
          </button>
        )}
        <ModeToggle />
      </div>
    </header>
  );
}
