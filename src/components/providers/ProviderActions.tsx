import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useI18n } from "@/i18n";
import { Pencil, Trash2, Copy, Activity, Wallet } from "lucide-react";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import type { AppType, Provider } from "@/types";

interface ProviderActionsProps {
  provider: Provider;
  isCurrent: boolean;
  /** 当前分组协议是否已配置端点(未配置则禁用"切换") */
  canSwitch?: boolean;
  app: AppType;
  /** 当前分组接管是否实际生效(false = 被外部覆盖,按钮文案降级为「未接管」) */
  takeoverEffective?: boolean;
  onSwitch: (provider: Provider) => void;
  onEdit: (provider: Provider) => void;
  onDuplicate: (provider: Provider) => void;
  onDelete: (provider: Provider) => void;
  onError: (msg: string) => void;
}

interface Balance {
  usage: number | null;
  limit: number | null;
}

interface TestResult {
  ok: boolean;
  status: number | null;
  latency_ms: number;
  message: string;
}

export function ProviderActions({
  provider,
  isCurrent,
  canSwitch = true,
  app,
  takeoverEffective,
  onSwitch,
  onEdit,
  onDuplicate,
  onDelete,
  onError,
}: ProviderActionsProps) {
  const { t } = useI18n();
  const iconButtonClass = "h-8 w-8 p-1";
  const switchDisabled = isCurrent || !canSwitch;
  const [testing, setTesting] = useState(false);
  const [result, setResult] = useState<TestResult | null>(null);
  const [balance, setBalance] = useState<Balance | null>(null);
  const [balanceNA, setBalanceNA] = useState(false);
  const [balanceLoading, setBalanceLoading] = useState(false);

  const runTest = async () => {
    setTesting(true);
    setResult(null);
    try {
      const r = await invoke<TestResult>("test_provider", {
        id: provider.id,
        appType: app,
      });
      setResult(r);
    } catch (e) {
      onError(String(e));
    } finally {
      setTesting(false);
    }
  };

  const runBalance = async () => {
    setBalanceNA(false);
    setBalanceLoading(true);
    try {
      const b = await invoke<Balance>("get_provider_balance", { id: provider.id });
      setBalance(b);
    } catch {
      setBalance(null);
      setBalanceNA(true);
    } finally {
      setBalanceLoading(false);
    }
  };

  return (
    <div className="flex items-center gap-1.5">
      {/* 测试结果:延迟/状态 */}
      {result && (
        <span
          title={result.ok ? `${result.latency_ms}ms` : result.message || t("provider.testFail")}
          className={cn(
            "text-xs tabular-nums max-w-[160px] truncate",
            result.ok ? "text-emerald-600 dark:text-emerald-400" : "text-red-500",
          )}
        >
          {result.ok
            ? `✓ ${result.latency_ms}ms`
            : `✗ ${result.message || t("provider.testFail")}`}
        </span>
      )}
      <Button
        size="sm"
        variant={isCurrent ? "secondary" : "default"}
        disabled={switchDisabled}
        onClick={() => onSwitch(provider)}
        title={canSwitch ? t("provider.switch") : t("provider.noEndpoint")}
        className={
          isCurrent && takeoverEffective === false
            ? "text-amber-600 dark:text-amber-400"
            : undefined
        }
      >
        {isCurrent
          ? takeoverEffective === false
            ? t("provider.notTaken")
            : t("provider.current")
          : t("provider.switchBtn")}
      </Button>
      <Button
        size="icon"
        variant="ghost"
        className={iconButtonClass}
        disabled={testing || !canSwitch}
        onClick={() => void runTest()}
        title={t("provider.test")}
      >
        <Activity className={cn("h-4 w-4", testing && "animate-pulse")} />
      </Button>
      <Button
        size="icon"
        variant="ghost"
        className={iconButtonClass}
        disabled={!provider.has_key || balanceLoading}
        onClick={() => void runBalance()}
        title={t("provider.balance")}
      >
        <Wallet className={cn("h-4 w-4", balanceLoading && "animate-pulse")} />
      </Button>
      {balanceLoading && (
        <span className="text-xs text-muted-foreground">…</span>
      )}
      {balance && (
        <span className="text-xs tabular-nums text-muted-foreground">
          ${(balance.usage ?? 0).toFixed(2)}
          {balance.limit != null ? ` / $${balance.limit.toFixed(2)}` : ""}
        </span>
      )}
      {balanceNA && (
        <span className="text-xs text-muted-foreground">{t("provider.balanceNA")}</span>
      )}
      <Button
        size="icon"
        variant="ghost"
        className={iconButtonClass}
        onClick={() => onDuplicate(provider)}
        title={t("provider.duplicate")}
      >
        <Copy className="h-4 w-4" />
      </Button>
      <Button
        size="icon"
        variant="ghost"
        className={iconButtonClass}
        onClick={() => onEdit(provider)}
        title={t("provider.edit")}
      >
        <Pencil className="h-4 w-4" />
      </Button>
      <Button
        size="icon"
        variant="ghost"
        className={`${iconButtonClass} hover:bg-red-500/15 hover:text-red-500`}
        onClick={() => onDelete(provider)}
        title={t("provider.delete")}
      >
        <Trash2 className="h-4 w-4" />
      </Button>
    </div>
  );
}
