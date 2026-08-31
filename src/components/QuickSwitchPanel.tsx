import { useEffect, useRef, useState } from "react";
import { CornerDownLeft } from "lucide-react";
import { ProviderIcon } from "@/components/ProviderIcon";
import { useI18n } from "@/i18n";
import { cn } from "@/lib/utils";
import type { AppType, Provider } from "@/types";

/** 跨分组条目:供应商 + 所属分组(用于分组小标题渲染) */
interface FlatEntry {
  p: Provider;
  app: AppType;
}

/**
 * 快速切换面板(全局快捷键 ⌘⇧K / Ctrl+Shift+K 唤起)。
 * 传入 groups 时跨分组列出全部供应商(分组小标题分隔),
 * 否则只列当前分组;↑↓/数字键 选择、Enter 切换、Esc 关闭。
 */
export function QuickSwitchPanel({
  open,
  onClose,
  app,
  providers,
  groups,
  onPick,
}: {
  open: boolean;
  onClose: () => void;
  app: AppType;
  providers: Provider[];
  groups?: { app: AppType; providers: Provider[] }[];
  onPick: (p: Provider, app: AppType) => void;
}) {
  const { t } = useI18n();
  const [idx, setIdx] = useState(0);
  const [q, setQ] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLDivElement>(null);

  // 模糊子序列匹配(Raycast 式):按字符顺序命中即可,并记录命中位用于高亮
  const fuzzy = (name: string, query: string): number[] | null => {
    const hit: number[] = [];
    let j = 0;
    for (let i = 0; i < name.length && j < query.length; i++) {
      if (name[i].toLowerCase() === query[j]) {
        hit.push(i);
        j++;
      }
    }
    return j === query.length ? hit : null;
  };
  // 跨分组时扁平化(编号/键盘导航按全局顺序),渲染时再按分组分组
  const flat: FlatEntry[] = groups
    ? groups.flatMap((g) => g.providers.map((p) => ({ p, app: g.app })))
    : providers.map((p) => ({ p, app }));
  const query = q.trim().toLowerCase();
  const filtered = query
    ? flat
        .map((e) => ({ e, hits: fuzzy(e.p.name, query) }))
        .filter((x) => x.hits)
        .map((x) => x.e)
    : flat;
  const hitOf = (p: Provider): Set<number> => {
    if (!query) return new Set();
    return new Set(fuzzy(p.name, query) ?? []);
  };

  useEffect(() => {
    if (open) {
      setQ("");
      setIdx(Math.max(0, filtered.findIndex((e) => e.p.is_current)));
      setTimeout(() => inputRef.current?.focus(), 30);
    }
  }, [open]); // eslint-disable-line react-hooks/exhaustive-deps

  // 输入过滤后保持选中项有效
  useEffect(() => {
    setIdx((i) => Math.min(i, Math.max(0, filtered.length - 1)));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [q]);

  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        // 有关键词时先清词,再按一次才关闭
        if (q.trim()) setQ("");
        else onClose();
      } else if (e.key === "ArrowDown") {
        e.preventDefault();
        setIdx((i) => Math.min(filtered.length - 1, i + 1));
      } else if (e.key === "ArrowUp") {
        e.preventDefault();
        setIdx((i) => Math.max(0, i - 1));
      } else if (e.key === "Enter") {
        e.preventDefault();
        const e2 = filtered[idx];
        if (e2) pick(e2.p, e2.app);
      } else if (/^[1-9]$/.test(e.key) && !e.metaKey && !e.ctrlKey) {
        // 数字键 1-9 秒切(Raycast 式)
        const e2 = filtered[Number(e.key) - 1];
        if (e2) {
          e.preventDefault();
          pick(e2.p, e2.app);
        }
      }
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open, idx, q, filtered]);

  useEffect(() => {
    listRef.current
      ?.querySelectorAll("[data-qs-item]")
      [idx]?.scrollIntoView({ block: "nearest" });
  }, [idx]);

  const pick = (p: Provider, pApp: AppType) => {
    onClose();
    onPick(p, pApp);
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
            ↑↓ · ⏎ · 1-9 · Esc
          </span>
        </div>
        <div className="px-3 py-2 border-b border-border">
          <input
            ref={inputRef}
            value={q}
            onChange={(e) => setQ(e.target.value)}
            placeholder={t("qs.filterPh")}
            className="w-full h-8 rounded-md bg-muted px-2.5 text-sm outline-none focus:ring-1 focus:ring-ring placeholder:text-muted-foreground"
          />
        </div>
        <div ref={listRef} className="max-h-[46vh] overflow-y-auto py-1.5">
          {filtered.length === 0 && (
            <p className="px-4 py-6 text-center text-sm text-muted-foreground">
              {flat.length === 0 ? t("qs.empty") : t("qs.noMatch")}
            </p>
          )}
          {(() => {
            // 按分组分段渲染:遇到新分组先输出小标题(首组不重复标题)
            let lastApp: AppType | null = null;
            return filtered.map((e, i) => {
              const showHeader = groups && e.app !== lastApp;
              lastApp = e.app;
              return (
                <div key={e.p.id}>
                  {showHeader && (
                    <p className="px-4 pt-2 pb-1 text-[10px] font-semibold uppercase tracking-wide text-muted-foreground">
                      {t(`app.${e.app}`)}
                    </p>
                  )}
                  {item(e.p, e.app, i)}
                </div>
              );
            });
          })()}
        </div>
      </div>
    </div>
  );

  function item(p: Provider, _pApp: AppType, i: number) {
    return (
            <button
              type="button"
              data-qs-item
              onMouseEnter={() => setIdx(i)}
              onClick={() => pick(p, _pApp)}
              className={cn(
                "w-full flex items-center gap-3 px-4 py-2.5 text-left text-sm",
                i === idx ? "bg-accent" : "hover:bg-accent/60",
                p.is_current && "font-medium",
              )}
            >
              <span className="w-4 text-[10px] text-muted-foreground tabular-nums text-right">
                {i < 9 ? i + 1 : ""}
              </span>
              <ProviderIcon icon={p.base_url} name={p.name} size={18} />
              <span className="flex-1 truncate">
                {(() => {
                  const hits = hitOf(p);
                  return p.name.split("").map((ch, ci) =>
                    hits.has(ci) ? (
                      <b key={ci} className="text-blue-600 dark:text-blue-400">{ch}</b>
                    ) : (
                      <span key={ci}>{ch}</span>
                    ),
                  );
                })()}
              </span>
              {p.last_test && (
                <span
                  className={
                    "text-[10px] tabular-nums " +
                    (p.last_test.ok
                      ? "text-emerald-600 dark:text-emerald-400"
                      : "text-red-500")
                  }
                >
                  {p.last_test.ok ? `${p.last_test.latency_ms}ms` : "✗"}
                </span>
              )}
              {p.is_current && (
                <span className="text-[10px] rounded bg-blue-500/15 px-1.5 py-px text-blue-600 dark:text-blue-400">
                  {t("qs.current")}
                </span>
              )}
              {i === idx && (
                <CornerDownLeft className="w-3.5 h-3.5 text-muted-foreground" />
              )}
            </button>
    );
  }
}
