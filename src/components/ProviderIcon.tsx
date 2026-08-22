import { Sparkles, Bot, Gem, Code2, PawPrint, Zap } from "lucide-react";

/**
 * 应用/供应商品牌图标(近似版)。
 * CC Switch 用 lobehub 静态 SVG,这里先用 lucide 图标 + 品牌色近似,
 * 视觉位置与尺寸完全一致(8x8 方块内 20px 图标)。
 */
const ICONS: Record<string, { Icon: typeof Zap; color: string }> = {
  claude: { Icon: Sparkles, color: "#D97757" },
  openai: { Icon: Bot, color: "#10A37F" },
  gemini: { Icon: Gem, color: "#4285F4" },
  opencode: { Icon: Code2, color: "#CBA6F7" },
  openclaw: { Icon: PawPrint, color: "#FF6B35" },
  coderplan: { Icon: Zap, color: "#CBA6F7" },
};

export function ProviderIcon({
  icon,
  name,
  color,
  size = 20,
}: {
  icon: string;
  name?: string;
  color?: string;
  size?: number;
}) {
  const entry = ICONS[icon] ?? ICONS[name?.toLowerCase() ?? ""] ?? {
    Icon: Zap,
    color: "#71717a",
  };
  const { Icon } = entry;
  return (
    <Icon
      size={size}
      color={color || entry.color}
      aria-hidden="true"
    />
  );
}
