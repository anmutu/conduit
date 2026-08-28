import { Zap } from "lucide-react";
import { cn } from "@/lib/utils";

/**
 * 品牌图标:lobehub 静态 SVG(MIT)。
 * 深色适配:openai 为 currentColor 单色,img 引用下为黑色,深色背景需反转。
 */
const BRAND: Record<string, { src: string; darkInvert?: boolean }> = {
  claude: { src: "icons/claude.svg" },
  openai: { src: "icons/openai.svg", darkInvert: true },
  gemini: { src: "icons/gemini.svg" },
  opencode: { src: "icons/opencode.svg" },
  openclaw: { src: "icons/openclaw.svg" },
  // 供应商品牌(预设列表用,lobehub SVG,MIT)
  deepseek: { src: "icons/deepseek.svg" },
  kimi: { src: "icons/kimi.svg" },
  zhipu: { src: "icons/zhipu.svg" },
  doubao: { src: "icons/doubao.svg" },
  minimax: { src: "icons/minimax.svg" },
  qwen: { src: "icons/qwen.svg" },
  bailian: { src: "icons/bailian.svg" },
  siliconflow: { src: "icons/siliconflow.svg" },
  openrouter: { src: "icons/openrouter.svg" },
  aihubmix: { src: "icons/aihubmix-color.svg" },
  stepfun: { src: "icons/stepfun.svg" },
  modelscope: { src: "icons/modelscope-color.svg" },
  coderplan: { src: "icons/coderplan.svg" },
  // CLI 分组品牌(官方发布渠道获取;商标归各自所有者)
  iflow: { src: "icons/iflow.png" },
  crush: { src: "icons/crush.png" },
  droid: { src: "icons/droid.svg" },
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
