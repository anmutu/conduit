import type { AppType, Provider, UsageSummary } from "@/types";
import { APP_PROTOCOL } from "@/types";
import { cn } from "@/lib/utils";
import { ProviderIcon } from "@/components/ProviderIcon";
import { ProviderActions } from "@/components/providers/ProviderActions";
import { useI18n } from "@/i18n";

interface ProviderCardProps {
  provider: Provider;
  isCurrent: boolean;
  /** 当前分组(用于取对应协议的端点展示/置灰判断) */
  app: AppType;
  /** 新建/更新后短暂高亮(P2-15) */
  highlight?: boolean;
  onSwitch: (provider: Provider) => void;
  onEdit: (provider: Provider) => void;
  onDuplicate: (provider: Provider) => void;
  onDelete: (provider: Provider) => void;
  /** 点击接口地址复制 */
  onCopyUrl: (url: string) => void;
  /** 累计用量(有数据才显示) */
  usage?: UsageSummary;
}

// 视觉结构复刻 CC Switch 的 ProviderCard:
// rounded-xl 卡片 + 当前项蓝色边框/渐变 + hover 显示操作组。
// 与原版的差异:当前项常显"当前"徽章(不依赖 hover);拖拽把手待 M1 排序功能一起加。
export function ProviderCard({
  provider,
  isCurrent,
  app,
  highlight = false,
  onSwitch,
  onEdit,
  onDuplicate,
  onDelete,
  onCopyUrl,
  usage,
}: ProviderCardProps) {
  const { t } = useI18n();
  // 该分组协议的端点;无端点 → 置灰(供应商实体仍在,只是未配置此协议)
  const protocol = APP_PROTOCOL[app];
  const endpoint = provider.endpoints?.[protocol];
  const displayUrl = endpoint || provider.base_url || t("provider.notConfigured");
  const shouldUseBlue = isCurrent && !!endpoint;

  return (
    <div
      id={`provider-${provider.id}`}
      className={cn(
        "relative overflow-hidden rounded-xl border border-border p-4 transition-all duration-300",
        "bg-card text-card-foreground group",
        "hover:border-border-active",
        shouldUseBlue && "border-blue-500/60 shadow-sm shadow-blue-500/10",
        !isCurrent && "hover:shadow-sm",
        !endpoint && "opacity-60",
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
              {!endpoint && (
                <span className="inline-flex items-center rounded-md bg-muted px-1.5 py-0.5 text-[10px] font-semibold text-muted-foreground">
                  {t("provider.noEndpoint")}
                </span>
              )}
            </div>

            <button
              type="button"
              onClick={() => onCopyUrl(displayUrl)}
              className="inline-flex items-center text-sm max-w-[280px] text-muted-foreground hover:text-blue-500 dark:hover:text-blue-400 transition-colors cursor-pointer"
              title={t("provider.copyUrl")}
            >
              <span className="truncate">{displayUrl}</span>
            </button>
          </div>
        </div>

        {/* hover 显示操作组 */}
        <div className="flex items-center ml-auto min-w-0 gap-3">
          {/* 累计用量(常显,有数据才出现) */}
          {usage && usage.requests > 0 && (
            <div className="hidden sm:flex flex-col items-end text-xs text-muted-foreground leading-tight">
              <span className="font-medium text-foreground">{t("provider.requests", { n: usage.requests })}</span>
              <span>
                ↓ {fmtTokens(usage.input_tokens)} · ↑ {fmtTokens(usage.output_tokens)}
              </span>
            </div>
          )}
          <div className="ml-auto">
            <div className="flex items-center gap-1" />
          </div>
          <div className="flex items-center gap-1.5 flex-shrink-0 opacity-0 pointer-events-none group-hover:opacity-100 group-focus-within:opacity-100 group-hover:pointer-events-auto group-focus-within:pointer-events-auto transition-opacity duration-200">
            <ProviderActions
              provider={provider}
              isCurrent={isCurrent}
              canSwitch={!!endpoint}
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

function fmtTokens(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}k`;
  return String(n);
}
