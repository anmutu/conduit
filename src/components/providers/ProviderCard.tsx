import { GripVertical } from "lucide-react";
import type { Provider } from "@/types";
import { cn } from "@/lib/utils";
import { ProviderIcon } from "@/components/ProviderIcon";
import { ProviderActions } from "@/components/providers/ProviderActions";

interface ProviderCardProps {
  provider: Provider;
  isCurrent: boolean;
  onSwitch: (provider: Provider) => void;
  onEdit: (provider: Provider) => void;
  onDuplicate: (provider: Provider) => void;
  onDelete: (provider: Provider) => void;
}

// 视觉结构复刻 CC Switch 的 ProviderCard:
// rounded-xl 卡片 + 当前项蓝色边框/渐变 + 左侧拖拽把手与图标 + hover 显示操作组
export function ProviderCard({
  provider,
  isCurrent,
  onSwitch,
  onEdit,
  onDuplicate,
  onDelete,
}: ProviderCardProps) {
  const displayUrl = provider.base_url || "未配置接口地址";
  const shouldUseBlue = isCurrent;

  return (
    <div
      className={cn(
        "relative overflow-hidden rounded-xl border border-border p-4 transition-all duration-300",
        "bg-card text-card-foreground group",
        "hover:border-border-active",
        shouldUseBlue &&
          "border-blue-500/60 shadow-sm shadow-blue-500/10",
        !isCurrent && "hover:shadow-sm",
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
          {/* 拖拽把手(M0 装饰,排序功能 M1) */}
          <div
            className={cn(
              "-ml-1.5 flex-shrink-0 p-1.5",
              "text-muted-foreground/50 hover:text-muted-foreground transition-colors",
            )}
            aria-label="拖拽排序"
          >
            <GripVertical className="h-4 w-4" />
          </div>

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
