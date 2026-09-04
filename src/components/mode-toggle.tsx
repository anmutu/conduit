import { Moon, Sun, Monitor } from "lucide-react";
import { Button } from "@/components/ui/button";
import { useTheme, type Theme } from "@/components/theme-provider";
import { useI18n } from "@/i18n";

const ORDER: Theme[] = ["light", "dark", "system"];
const META = {
  light: { icon: Sun, labelKey: "theme.light" },
  dark: { icon: Moon, labelKey: "theme.dark" },
  system: { icon: Monitor, labelKey: "theme.system" },
} as const;

/** 三态主题循环切换:浅色 → 深色 → 跟随系统 */
export function ModeToggle() {
  const { theme, setTheme } = useTheme();
  const { t } = useI18n();
  const { icon: Icon, labelKey } = META[theme];

  return (
    <Button
      variant="ghost"
      size="icon"
      title={t(labelKey)}
      className="hover:bg-black/5 dark:hover:bg-white/5"
      onClick={() => {
        const next = ORDER[(ORDER.indexOf(theme) + 1) % ORDER.length];
        setTheme(next);
      }}
    >
      <Icon className="w-4 h-4" />
    </Button>
  );
}
