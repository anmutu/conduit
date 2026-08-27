import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ScrollText, Search, Download } from "lucide-react";
import { useI18n } from "@/i18n";
import { Input } from "@/components/ui/input";
import type { Provider } from "@/types";

interface UsageEntry {
  id: number;
  provider_id: string;
  model: string | null;
  input_tokens: number;
  output_tokens: number;
  status: number;
  rule_pattern: string | null;
  duration_ms: number;
  error_note: string | null;
  created_at: number;
}

const IS_DEMO = new URLSearchParams(location.search).has("demo");

/** 请求日志浏览器:最近 100 条请求(时间/供应商/模型/tokens/状态) */
export function LogsPage({
  app,
  providers,
  onError,
}: {
  app: string;
  providers: Provider[];
  onError: (msg: string) => void;
}) {
  const { t } = useI18n();
  const [entries, setEntries] = useState<UsageEntry[] | null>(null);
  const [q, setQ] = useState("");
  const [prov, setProv] = useState("");
  const [onlyErrors, setOnlyErrors] = useState(false);

  const exportCsv = async () => {
    try {
      const r = await invoke<{ path: string; count: number }>(
        "export_usage_csv",
        { appType: app },
      );
      onError(t("logs.exportedTo", { path: r.path, n: r.count }));
    } catch (e) {
      onError(String(e));
    }
  };

  const filtered = useMemo(() => {
    if (!entries) return null;
    const kw = q.trim().toLowerCase();
    return entries.filter(
      (e) =>
        (!prov || e.provider_id === prov) &&
        (!onlyErrors || e.status >= 400) &&
        (!kw ||
          (e.model ?? "").toLowerCase().includes(kw) ||
          nameOf(e.provider_id).toLowerCase().includes(kw) ||
          (e.rule_pattern ?? "").toLowerCase().includes(kw) ||
          (e.error_note ?? "").toLowerCase().includes(kw)),
    );
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [entries, q, prov, onlyErrors, providers]);

  useEffect(() => {
    if (IS_DEMO) {
      setEntries([]);
      return;
    }
    invoke<UsageEntry[]>("get_recent_usage", { appType: app, limit: 100 })
      .then(setEntries)
      .catch((e) => onError(String(e)));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [app]);

  const nameOf = (id: string) =>
    providers.find((p) => p.id === id)?.name ?? id.slice(0, 8);

  const fmtTime = (ts: number) => {
    const d = new Date(ts * 1000);
    const pad = (n: number) => String(n).padStart(2, "0");
    return `${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`;
  };

  return (
    <div className="space-y-4 w-full">
      <div className="rounded-xl border border-border bg-card">
        <div className="flex items-center justify-between px-4 py-3 border-b border-border">
          <h3 className="text-sm font-semibold">{t("logs.title")}</h3>
          <span className="text-xs text-muted-foreground">
            {t("logs.count", {
              n: filtered?.length ?? 0,
              total: entries?.length ?? 0,
            })}
          </span>
        </div>
        {/* 筛选行:关键词 / 供应商 / 仅看失败 */}
        {entries !== null && entries.length > 0 && (
          <div className="flex items-center gap-2 px-4 py-2 border-b border-border flex-wrap">
            <div className="relative flex-1 min-w-[140px] max-w-[220px]">
              <Search className="w-3.5 h-3.5 absolute left-2 top-1/2 -translate-y-1/2 text-muted-foreground" />
              <Input
                value={q}
                onChange={(e) => setQ(e.target.value)}
                placeholder={t("logs.searchPh")}
                className="h-7 pl-7 text-xs"
              />
            </div>
            <select
              value={prov}
              onChange={(e) => setProv(e.target.value)}
              className="h-7 rounded-md border border-border bg-background px-1.5 text-xs"
            >
              <option value="">{t("logs.allProviders")}</option>
              {providers.map((p) => (
                <option key={p.id} value={p.id}>
                  {p.name}
                </option>
              ))}
            </select>
            <label className="flex items-center gap-1 text-xs text-muted-foreground cursor-pointer select-none">
              <input
                type="checkbox"
                checked={onlyErrors}
                onChange={(e) => setOnlyErrors(e.target.checked)}
                className="accent-red-500"
              />
              {t("logs.onlyErrors")}
            </label>
            <button
              type="button"
              onClick={() => void exportCsv()}
              className="ml-auto flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground"
              title={t("logs.exportTitle")}
            >
              <Download className="w-3.5 h-3.5" />
              CSV
            </button>
          </div>
        )}
        {entries === null ? (
          <p className="text-sm text-muted-foreground text-center py-8">
            {t("common.loading")}
          </p>
        ) : (filtered?.length ?? 0) === 0 ? (
          <div className="flex flex-col items-center gap-2 py-10 text-center">
            <ScrollText className="w-8 h-8 text-muted-foreground" />
            <p className="text-sm text-muted-foreground">{t("logs.empty")}</p>
          </div>
        ) : (
          <div className="max-h-[60vh] overflow-y-auto">
            <table className="w-full text-xs">
              <thead className="text-muted-foreground sticky top-0 bg-card">
                <tr className="border-b border-border">
                  <th className="text-left font-medium px-4 py-2">{t("logs.time")}</th>
                  <th className="text-left font-medium px-2 py-2">{t("logs.provider")}</th>
                  <th className="text-left font-medium px-2 py-2">{t("logs.model")}</th>
                  <th className="text-right font-medium px-2 py-2">↓</th>
                  <th className="text-right font-medium px-2 py-2">↑</th>
                  <th className="text-right font-medium px-2 py-2">{t("logs.duration")}</th>
                  <th className="text-right font-medium px-4 py-2">{t("logs.status")}</th>
                </tr>
              </thead>
              <tbody className="tabular-nums">
                {filtered?.map((e) => (
                  <tr
                    key={e.id}
                    className="border-b border-border/50 hover:bg-accent/50"
                  >
                    <td className="px-4 py-1.5 text-muted-foreground whitespace-nowrap">
                      {fmtTime(e.created_at)}
                    </td>
                    <td className="px-2 py-1.5 max-w-[140px] truncate">
                      {nameOf(e.provider_id)}
                    </td>
                    <td className="px-2 py-1.5 max-w-[180px] truncate">
                      {e.model ?? "—"}
                      {e.rule_pattern && (
                        <span
                          className="ml-1.5 rounded bg-blue-500/15 px-1 py-px text-[10px] text-blue-600 dark:text-blue-400 align-middle"
                          title={t("logs.ruleHit", { pattern: e.rule_pattern })}
                        >
                          {e.rule_pattern}
                        </span>
                      )}
                    </td>
                    <td className="px-2 py-1.5 text-right text-muted-foreground">
                      {e.input_tokens}
                    </td>
                    <td className="px-2 py-1.5 text-right text-muted-foreground">
                      {e.output_tokens}
                    </td>
                    <td className="px-2 py-1.5 text-right text-muted-foreground">
                      {e.duration_ms > 0 ? `${(e.duration_ms / 1000).toFixed(1)}s` : "—"}
                    </td>
                    <td className="px-4 py-1.5 text-right">
                      <span
                        className={
                          e.status < 400
                            ? "text-emerald-600 dark:text-emerald-400"
                            : "text-red-500"
                        }
                        title={e.error_note ?? undefined}
                      >
                        {e.status}
                      </span>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </div>
      <p className="text-xs text-muted-foreground text-right">{t("logs.note")}</p>
    </div>
  );
}
