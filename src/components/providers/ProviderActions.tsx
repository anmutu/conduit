import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useI18n } from "@/i18n";
import { Pencil, Trash2, Copy, Activity } from "lucide-react";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import type { AppType, Provider } from "@/types";

interface ProviderActionsProps {
  provider: Provider;
  isCurrent: boolean;
  /** 当前分组协议是否已配置端点(未配置则禁用"切换") */
  canSwitch?: boolean;
  app: AppType;
  onSwitch: (provider: Provider) => void;
  onEdit: (provider: Provider) => void;
  onDuplicate: (provider: Provider) => void;
  onDelete: (provider: Provider) => void;
  onError: (msg: string) => void;
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

  return (
    <div className="flex items-center gap-1.5">
      {/* 测试结果:延迟/状态 */}
      {result && (
        <span
          className={cn(
            "text-xs tabular-nums",
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
      >
        {isCurrent ? t("provider.current") : t("provider.switchBtn")}
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
