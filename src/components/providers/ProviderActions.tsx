import { useI18n } from "@/i18n";
import { Pencil, Trash2, Copy } from "lucide-react";
import { Button } from "@/components/ui/button";
import type { Provider } from "@/types";

interface ProviderActionsProps {
  provider: Provider;
  isCurrent: boolean;
  /** 当前分组协议是否已配置端点(未配置则禁用"切换") */
  canSwitch?: boolean;
  onSwitch: (provider: Provider) => void;
  onEdit: (provider: Provider) => void;
  onDuplicate: (provider: Provider) => void;
  onDelete: (provider: Provider) => void;
}

export function ProviderActions({
  provider,
  isCurrent,
  canSwitch = true,
  onSwitch,
  onEdit,
  onDuplicate,
  onDelete,
}: ProviderActionsProps) {
  const { t } = useI18n();
  const iconButtonClass = "h-8 w-8 p-1";
  const switchDisabled = isCurrent || !canSwitch;

  return (
    <div className="flex items-center gap-1.5">
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
