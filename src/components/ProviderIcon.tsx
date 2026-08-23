import { Zap } from "lucide-react";
import { cn } from "@/lib/utils";

/**
 * 品牌图标:与 CC Switch 同源(lobehub 静态 SVG,MIT)。
 * 深色适配:openai 为 currentColor 单色,img 引用下为黑色,深色背景需反转。
 */
const BRAND: Record<string, { src: string; darkInvert?: boolean }> = {
  claude: { src: "icons/claude.svg" },
  openai: { src: "icons/openai.svg", darkInvert: true },
  gemini: { src: "icons/gemini.svg" },
  opencode: { src: "icons/opencode.svg" },
  openclaw: { src: "icons/openclaw.svg" },
};

export function ProviderIcon({
  icon,
  name,
  size = 20,
}: {
  icon: string;
  name?: string;
  color?: string;
  size?: number;
}) {
  const key = icon?.toLowerCase() ?? "";
  const brand = BRAND[key] ?? BRAND[name?.toLowerCase() ?? ""];

  if (brand) {
    return (
      <img
        src={brand.src}
        alt={name ?? key}
        width={size}
        height={size}
        className={cn(brand.darkInvert && "dark:invert", "object-contain")}
        aria-hidden="true"
      />
    );
  }

  // 未识别品牌:中性 fallback
  return <Zap size={size} color="#71717a" aria-hidden="true" />;
}
