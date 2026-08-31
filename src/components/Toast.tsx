import { cn } from "@/lib/utils";

export type ToastType = "success" | "error";

export interface ToastItem {
  id: number;
  type: ToastType;
  msg: string;
  /** 可选动作(如"撤销"):带动作的 toast 停留更久 */
  action?: { label: string; run: () => void };
}

/**
 * 轻量 toast 堆叠(底部居中),复刻桌面应用的即时反馈:
 * 成功 1.8s / 错误 3.5s 自动消失,点击立即关闭。
 */
export function ToastStack({
  items,
  onDismiss,
}: {
  items: ToastItem[];
  onDismiss: (id: number) => void;
}) {
  if (items.length === 0) return null;
  return (
    <div className="fixed bottom-4 left-1/2 -translate-x-1/2 z-[70] flex flex-col items-center gap-2 pointer-events-none">
      {items.map((t) => (
        <div
          key={t.id}
          role="status"
          onClick={() => onDismiss(t.id)}
          className={cn(
            "pointer-events-auto cursor-pointer px-4 py-2 rounded-lg text-sm shadow-lg animate-slide-up",
            "max-w-[80%] truncate select-none",
            t.type === "success"
              ? "bg-emerald-500/95 text-white"
              : "bg-red-500/95 text-white",
          )}
          title={t.msg}
        >
          {t.msg}
          {t.action && (
            <button
              type="button"
              onClick={(e) => {
                e.stopPropagation();
                onDismiss(t.id);
                t.action!.run();
              }}
              className="ml-2 rounded px-1.5 py-0.5 text-xs font-medium bg-white/20 hover:bg-white/30"
            >
              {t.action.label}
            </button>
          )}
        </div>
      ))}
    </div>
  );
}
