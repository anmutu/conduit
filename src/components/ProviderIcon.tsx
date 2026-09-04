import { Zap } from "lucide-react";
import { cn } from "@/lib/utils";

/**
 * 品牌图标:lobehub 静态 SVG(MIT)。
 * 深色适配:openai 为 currentColor 单色,img 引用下为黑色,深色背景需反转。
 * full:自带背景的方形徽标(如 coderplan 终端造型),渲染时占满容器/独立圆角,
 * 避免在卡片图标配底色框里出现"双重边框"。
 */
const BRAND: Record<string, { src: string; darkInvert?: boolean; full?: boolean }> = {
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
  coderplan: { src: "icons/coderplan.svg", full: true },
  // CLI 分组品牌(官方发布渠道获取;商标归各自所有者)
  iflow: { src: "icons/iflow.png", full: true },
  crush: { src: "icons/crush.png", full: true },
  droid: { src: "icons/droid.svg", full: true },
};

export function ProviderIcon({
  icon,
  name,
  size = 20,
  fill = false,
}: {
  icon: string;
  name?: string;
  color?: string;
  size?: number;
  /** 占满父容器(父容器需有固定尺寸并自行圆角/裁切);与 size 互斥 */
  fill?: boolean;
}) {
  const key = icon?.toLowerCase() ?? "";
  const brand = BRAND[key] ?? BRAND[name?.toLowerCase() ?? ""];

  if (brand) {
    if (brand.full) {
      return (
        <img
          src={brand.src}
          alt={name ?? key}
          width={size}
          height={size}
          className={cn(
            "object-cover",
            fill ? "w-full h-full" : "rounded-[22%]",
            brand.darkInvert && "dark:invert",
          )}
          aria-hidden="true"
        />
      );
    }
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
