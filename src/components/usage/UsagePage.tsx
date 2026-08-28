import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";
import { BarChart3 } from "lucide-react";
import { useI18n } from "@/i18n";
import type { Provider } from "@/types";

interface UsageSummary {
  requests: number;
  input_tokens: number;
  output_tokens: number;
  errors?: number;
}
interface NamedUsage extends UsageSummary {
  avg_duration_ms?: number;
  key: string;
}
interface DayUsage {
  date: string;
  requests: number;
  tokens: number;
}
interface UsageDashboard {
  total: UsageSummary;
  by_provider: NamedUsage[];
  by_model: NamedUsage[];
  by_day: DayUsage[];
}

const IS_DEMO = new URLSearchParams(location.search).has("demo");

const DEMO: UsageDashboard = {
  total: { requests: 412, input_tokens: 3_860_000, output_tokens: 9_420_000 },
  by_provider: [
    { key: "demo-1", requests: 351, input_tokens: 3_300_000, output_tokens: 8_100_000 },
    { key: "demo-2", requests: 61, input_tokens: 560_000, output_tokens: 1_320_000 },
  ],
  by_model: [
    { key: "glm-4.6", requests: 288, input_tokens: 2_700_000, output_tokens: 6_400_000 },
    { key: "kimi-k2", requests: 98, input_tokens: 1_000_000, output_tokens: 2_600_000 },
    { key: "claude-sonnet-4.5", requests: 26, input_tokens: 160_000, output_tokens: 420_000 },
  ],
  by_day: [
    { date: "08-17", requests: 41, tokens: 1_020_000 },
    { date: "08-18", requests: 66, tokens: 1_540_000 },
    { date: "08-19", requests: 38, tokens: 910_000 },
    { date: "08-20", requests: 72, tokens: 1_780_000 },
    { date: "08-21", requests: 95, tokens: 2_260_000 },
    { date: "08-22", requests: 61, tokens: 1_410_000 },
    { date: "08-23", requests: 39, tokens: 860_000 },
  ],
};

function fmt(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}k`;
  return String(n);
}

function StatCard({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-xl border border-border p-4 bg-card text-center">
      <div className="text-2xl font-bold tabular-nums">{value}</div>
      <div className="text-xs text-muted-foreground mt-1">{label}</div>
    </div>
  );
}

function Bars({
  rows,
  format,
}: {
  rows: { label: string; value: number; sub?: string }[];
  format: (n: number) => string;
}) {
  const max = Math.max(...rows.map((r) => r.value), 1);
  return (
    <div className="space-y-2.5">
      {rows.map((r) => (
        <div key={r.label} className="text-xs">
          <div className="flex justify-between mb-1">
            <span className="font-medium truncate max-w-[60%]">{r.label}</span>
            <span className="text-muted-foreground tabular-nums">
              {format(r.value)}
              {r.sub ? ` · ${r.sub}` : ""}
            </span>
          </div>
          <div className="h-2 rounded-full bg-muted overflow-hidden">
            <div
              className="h-full rounded-full bg-gradient-to-r from-blue-500 to-indigo-500"
              style={{ width: `${Math.max((r.value / max) * 100, 2)}%` }}
            />
          </div>
        </div>
      ))}
    </div>
  );
}

export function UsagePage({
  app,
  providers,
  onError,
  onInfo,
}: {
  app: string;
  providers: Provider[];
  onError: (msg: string) => void;
  onInfo: (msg: string) => void;
}) {
  const { t } = useI18n();
  const [data, setData] = useState<UsageDashboard | null>(null);
  const [days, setDays] = useState(() => {
    const v = Number(localStorage.getItem("keyway.dash.days"));
    return [1, 7, 30].includes(v) ? v : 7;
  });

  useEffect(() => {
    if (IS_DEMO) {
      setData(DEMO);
      return;
    }
    invoke<UsageDashboard>("get_usage_dashboard", { appType: app, days })
      .then(setData)
      .catch((e) => onError(String(e)));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [app, days]);

  const nameOf = (id: string) =>
    providers.find((p) => p.id === id)?.name ?? id.slice(0, 8);

  const dayMax = Math.max(...(data?.by_day.map((d) => d.tokens) ?? [1]), 1);

  return (
    <div className="space-y-6 w-full">
      {/* 总览 */}
      <div className="grid grid-cols-4 gap-3">
        <StatCard label={t("dash.requests")} value={fmt(data?.total.requests ?? 0)} />
        <StatCard label={t("dash.input")} value={`↓ ${fmt(data?.total.input_tokens ?? 0)}`} />
        <StatCard label={t("dash.output")} value={`↑ ${fmt(data?.total.output_tokens ?? 0)}`} />
        <StatCard
          label={t("dash.successRate")}
          value={
            data && data.total.requests > 0
              ? `${Math.round(((data.total.requests - (data.total.errors ?? 0)) / data.total.requests) * 100)}%`
              : "—"
          }
        />
      </div>

      {/* 近 7 日趋势 */}
      <div className="rounded-xl border border-border p-4 bg-card">
        <div className="flex items-center justify-between mb-4">
          <h3 className="text-sm font-semibold">
            {days === 1 ? t("dash.trendToday") : t("dash.trendN", { n: days })}
          </h3>
          <div className="flex items-center gap-2">
          <button
            type="button"
            title={t("dash.exportModels")}
            onClick={async () => {
              try {
                const target = await save({
                  defaultPath: `keyway-models-${days}d.csv`,
                  filters: [{ name: "CSV", extensions: ["csv"] }],
                });
                if (!target) return;
                const r = await invoke<{ path: string; count: number }>(
                  "export_usage_models_csv",
                  { appType: app, days, target },
                );
                onInfo(t("dash.exported", { n: r.count, path: r.path }));
              } catch (e) {
                onError(String(e));
              }
            }}
            className="px-2 h-6 rounded-md text-[11px] font-medium text-muted-foreground hover:text-foreground transition-all"
          >
            CSV
          </button>
          <div className="flex items-center gap-1 p-0.5 bg-muted rounded-lg">
            {[1, 7, 30].map((d) => (
              <button
                key={d}
                type="button"
                onClick={() => {
                  setDays(d);
                  localStorage.setItem("keyway.dash.days", String(d));
                }}
                className={
                  "px-2 h-6 rounded-md text-[11px] font-medium transition-all " +
                  (days === d
                    ? "bg-background text-foreground shadow-sm"
                    : "text-muted-foreground hover:text-foreground")
                }
              >
                {d}d
              </button>
            ))}
          </div>
          </div>
        </div>
        {data && data.by_day.length > 0 ? (
          <div className="flex items-end gap-2 h-28">
            {data.by_day.map((d) => (
              <div key={d.date} className="flex-1 flex flex-col items-center gap-1.5 group">
                <div
                  className="w-full rounded-t-md bg-gradient-to-t from-blue-500/70 to-indigo-500/70 group-hover:from-blue-500 group-hover:to-indigo-500 transition-all"
                  style={{ height: `${Math.max((d.tokens / dayMax) * 100, 4)}%` }}
                  title={`${d.date}: ${fmt(d.tokens)} tokens / ${d.requests} 次`}
                />
                <span className="text-[10px] text-muted-foreground tabular-nums">
                  {d.date.slice(5)}
                </span>
              </div>
            ))}
          </div>
        ) : (
          <p className="text-sm text-muted-foreground py-6 text-center">{t("dash.empty")}</p>
        )}
      </div>

      {/* 按供应商 */}
      <div className="rounded-xl border border-border p-4 bg-card">
        <h3 className="text-sm font-semibold mb-4">{t("dash.byProvider")}</h3>
        {data && data.by_provider.length > 0 ? (
          <Bars
            rows={data.by_provider.map((p) => ({
              label: nameOf(p.key),
              value: p.input_tokens + p.output_tokens,
              sub:
                `${p.requests} ${t("dash.reqUnit")}` +
                ((p.errors ?? 0) > 0 ? ` · ${t("dash.fails", { n: p.errors ?? 0 })}` : "") +
                (p.avg_duration_ms && p.avg_duration_ms > 0
                  ? ` · ${(p.avg_duration_ms / 1000).toFixed(1)}s`
                  : ""),
            }))}
            format={fmt}
          />
        ) : (
          <p className="text-sm text-muted-foreground py-4 text-center">{t("dash.empty")}</p>
        )}
      </div>

      {/* 按模型 */}
      <div className="rounded-xl border border-border p-4 bg-card">
        <h3 className="text-sm font-semibold mb-4">{t("dash.byModel")}</h3>
        {data && data.by_model.length > 0 ? (
          <Bars
            rows={data.by_model.map((m) => ({
              label: m.key,
              value: m.input_tokens + m.output_tokens,
              sub:
                `${m.requests} ${t("dash.reqUnit")}` +
                ((m.errors ?? 0) > 0 ? ` · ${t("dash.fails", { n: m.errors ?? 0 })}` : "") +
                (m.avg_duration_ms && m.avg_duration_ms > 0
                  ? ` · ${(m.avg_duration_ms / 1000).toFixed(1)}s`
                  : ""),
            }))}
            format={fmt}
          />
        ) : (
          <p className="text-sm text-muted-foreground py-4 text-center">{t("dash.empty")}</p>
        )}
      </div>

      <p className="text-xs text-muted-foreground flex items-center justify-end gap-1.5">
        <BarChart3 className="w-3.5 h-3.5" />
        {t("dash.note")}
      </p>
    </div>
  );
}
