import type { Provider } from "@/types";
import { cn } from "@/lib/utils";
import { ProviderIcon } from "@/components/ProviderIcon";
import { ProviderActions } from "@/components/providers/ProviderActions";

interface ProviderCardProps {
  provider: Provider;
  isCurrent: boolean;
  /** 新建/更新后短暂高亮(P2-15) */
  highlight?: boolean;
  onSwitch: (provider: Provider) => void;
  onEdit: (provider: Provider) => void;
  onDuplicate: (provider: Provider) => void;
  onDelete: (provider: Provider) => void;
}

// 视觉结构复刻 CC Switch 的 ProviderCard:
// rounded-xl 卡片 + 当前项蓝色边框/渐变 + hover 显示操作组。
// 与原版的差异:当前项常显"当前"徽章(不依赖 hover);拖拽把手待 M1 排序功能一起加。
export function ProviderCard({
  provider,
  isCurrent,
  highlight = false,
  onSwitch,
  onEdit,
  onDuplicate,
  onDelete,
}: ProviderCardProps) {
  const displayUrl = provider.base_url || "未配置接口地址";
  const shouldUseBlue = isCurrent;

  return (
    <div
      id={`provider-${provider.id}`}
      className={cn(
        "relative overflow-hidden rounded-xl border border-border p-4 transition-all duration-300",
        "bg-card text-card-foreground group",
        "hover:border-border-active",
        shouldUseBlue && "border-blue-500/60 shadow-sm shadow-blue-500/10",
        !isCurrent && "hover:shadow-sm",
        highlight && "highlight-flash",
      )}
    >
      {/* 当前项左侧渐变高亮层 */}
      <div
        className={cn(
          "absolute inset-0 bg-gradient-to-r to-transparent transition-opacity duration-500 pointer-events-none",
          shouldUseBlue ? "from-blue-500/10" : "from-primary/10",
          isCurrent ? "opacity-100" : "opacity-0",
        )}
      />
      <div className="relative flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
        <div className="flex flex-1 items-center gap-2">
          {/* 供应商图标 */}
          <div className="h-8 w-8 rounded-lg bg-muted flex items-center justify-center border border-border group-hover:scale-105 transition-transform duration-300">
            <ProviderIcon
              icon={provider.app_type}
              name={provider.name}
              size={20}
            />
          </div>

          <div className="space-y-1">
            <div className="flex flex-wrap items-center gap-2 min-h-7">
              <h3 className="text-base font-semibold leading-none">
                {provider.name}
              </h3>
              {/* 当前项常显徽章:不依赖 hover,一眼可辨 */}
              {isCurrent && (
                <span className="inline-flex items-center rounded-md bg-blue-100 px-1.5 py-0.5 text-[10px] font-semibold text-blue-700 dark:bg-blue-900/40 dark:text-blue-300">
                  当前
                </span>
              )}
              {!provider.has_key && (
                <span className="inline-flex items-center rounded-md bg-amber-100 px-1.5 py-0.5 text-[10px] font-semibold text-amber-700 dark:bg-amber-900/40 dark:text-amber-300">
                  无 API Key
                </span>
              )}
            </div>

            <div
              className="inline-flex items-center text-sm max-w-[280px] text-muted-foreground cursor-default"
              title={displayUrl}
            >
              <span className="truncate">{displayUrl}</span>
            </div>
          </div>
        </div>

        {/* hover 显示操作组 */}
        <div className="flex items-center ml-auto min-w-0 gap-3">
          <div className="ml-auto">
            <div className="flex items-center gap-1" />
          </div>
          <div className="flex items-center gap-1.5 flex-shrink-0 opacity-0 pointer-events-none group-hover:opacity-100 group-focus-within:opacity-100 group-hover:pointer-events-auto group-focus-within:pointer-events-auto transition-opacity duration-200">
            <ProviderActions
              provider={provider}
              isCurrent={isCurrent}
              onSwitch={onSwitch}
              onEdit={onEdit}
              onDuplicate={onDuplicate}
              onDelete={onDelete}
            />
          </div>
        </div>
      </div>
    </div>
  );
}
