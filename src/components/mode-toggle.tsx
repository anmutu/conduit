import { Moon, Sun, Monitor } from "lucide-react";
import { Button } from "@/components/ui/button";
import { useTheme, type Theme } from "@/components/theme-provider";

const ORDER: Theme[] = ["light", "dark", "system"];
const META: Record<Theme, { icon: typeof Sun; label: string }> = {
  light: { icon: Sun, label: "浅色模式" },
  dark: { icon: Moon, label: "深色模式" },
  system: { icon: Monitor, label: "跟随系统" },
};

/** 三态主题循环切换:浅色 → 深色 → 跟随系统(与 CC Switch 的 mode-toggle 一致) */
export function ModeToggle() {
  const { theme, setTheme } = useTheme();
  const { icon: Icon, label } = META[theme];

  return (
    <Button
      variant="ghost"
      size="icon"
      title={label}
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
