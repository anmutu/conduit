import { Pencil, Trash2, Copy } from "lucide-react";
import { Button } from "@/components/ui/button";
import type { Provider } from "@/types";

interface ProviderActionsProps {
  provider: Provider;
  isCurrent: boolean;
  onSwitch: (provider: Provider) => void;
  onEdit: (provider: Provider) => void;
  onDuplicate: (provider: Provider) => void;
  onDelete: (provider: Provider) => void;
}

export function ProviderActions({
  provider,
  isCurrent,
  onSwitch,
  onEdit,
  onDuplicate,
  onDelete,
}: ProviderActionsProps) {
  const iconButtonClass = "h-8 w-8 p-1";

  return (
    <div className="flex items-center gap-1.5">
      <Button
        size="sm"
        variant={isCurrent ? "secondary" : "default"}
        disabled={isCurrent}
        onClick={() => onSwitch(provider)}
        title="切换到此供应商"
      >
        {isCurrent ? "当前使用中" : "切换"}
      </Button>
      <Button
        size="icon"
        variant="ghost"
        className={iconButtonClass}
        onClick={() => onDuplicate(provider)}
        title="复制供应商"
      >
        <Copy className="h-4 w-4" />
      </Button>
      <Button
        size="icon"
        variant="ghost"
        className={iconButtonClass}
        onClick={() => onEdit(provider)}
        title="编辑供应商"
      >
        <Pencil className="h-4 w-4" />
      </Button>
      <Button
        size="icon"
        variant="ghost"
        className={`${iconButtonClass} hover:bg-red-500/15 hover:text-red-500`}
        onClick={() => onDelete(provider)}
        title="删除供应商"
      >
        <Trash2 className="h-4 w-4" />
      </Button>
    </div>
  );
}
