import { useEffect, useRef, useState } from "react";
import { CornerDownLeft } from "lucide-react";
import { ProviderIcon } from "@/components/ProviderIcon";
import { useI18n } from "@/i18n";
import { cn } from "@/lib/utils";
import type { AppType, Provider } from "@/types";

/**
 * 快速切换面板(全局快捷键 ⌘⇧K / Ctrl+Shift+K 唤起)。
 * 列出当前分组的供应商,↑↓ 选择、Enter 切换、Esc 关闭。
 */
export function QuickSwitchPanel({
  open,
  onClose,
  app,
  providers,
  onPick,
}: {
  open: boolean;
  onClose: () => void;
  app: AppType;
  providers: Provider[];
  onPick: (p: Provider) => void;
}) {
  const { t } = useI18n();
  const [idx, setIdx] = useState(0);
  const listRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (open) setIdx(Math.max(0, providers.findIndex((p) => p.is_current)));
  }, [open, providers]);

  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        onClose();
      } else if (e.key === "ArrowDown") {
        e.preventDefault();
        setIdx((i) => Math.min(providers.length - 1, i + 1));
      } else if (e.key === "ArrowUp") {
        e.preventDefault();
        setIdx((i) => Math.max(0, i - 1));
      } else if (e.key === "Enter") {
        e.preventDefault();
        const p = providers[idx];
        if (p) pick(p);
      }
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open, idx, providers]);

  useEffect(() => {
    listRef.current
      ?.querySelectorAll("[data-qs-item]")
      [idx]?.scrollIntoView({ block: "nearest" });
  }, [idx]);

  const pick = (p: Provider) => {
    onClose();
    onPick(p);
  };

  if (!open) return null;

  return (
    <div
      className="fixed inset-0 z-[80] flex items-start justify-center bg-black/25 pt-[16vh] animate-fade-in"
      onClick={onClose}
    >
      <div
        className="w-[92vw] max-w-[420px] rounded-2xl border border-border bg-card shadow-2xl overflow-hidden"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="px-4 py-3 border-b border-border flex items-center justify-between">
          <span className="text-sm font-semibold">
            {t("qs.title", { app: app })}
          </span>
          <span className="text-[11px] text-muted-foreground">
            ↑↓ · ⏎ · Esc
          </span>
        </div>
        <div ref={listRef} className="max-h-[46vh] overflow-y-auto py-1.5">
          {providers.length === 0 && (
            <p className="px-4 py-6 text-center text-sm text-muted-foreground">
              {t("qs.empty")}
            </p>
          )}
          {providers.map((p, i) => (
            <button
              key={p.id}
              type="button"
              data-qs-item
              onMouseEnter={() => setIdx(i)}
              onClick={() => pick(p)}
              className={cn(
                "w-full flex items-center gap-3 px-4 py-2.5 text-left text-sm",
                i === idx ? "bg-accent" : "hover:bg-accent/60",
                p.is_current && "font-medium",
              )}
            >
              <ProviderIcon icon={p.base_url} name={p.name} size={18} />
              <span className="flex-1 truncate">{p.name}</span>
              {p.is_current && (
                <span className="text-[10px] rounded bg-blue-500/15 px-1.5 py-px text-blue-600 dark:text-blue-400">
                  {t("qs.current")}
                </span>
              )}
              {i === idx && (
                <CornerDownLeft className="w-3.5 h-3.5 text-muted-foreground" />
              )}
            </button>
          ))}
        </div>
      </div>
    </div>
  );
}
