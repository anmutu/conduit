import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useI18n } from "@/i18n";
import { AlertTriangle } from "lucide-react";
import { cn } from "@/lib/utils";

interface Degraded {
  id: string;
  name: string;
  total: number;
  errors: number;
}

interface GatewayStatus {
  proxy_addr: string;
  version: string;
  current: Record<string, string | null>;
  today_tokens: number;
  today_requests: number;
}

const IS_DEMO = new URLSearchParams(location.search).has("demo");

function fmtTokens(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}k`;
  return String(n);
}

/** 网关状态条(Docker Desktop 式状态感):绿点 + 地址 + 各分组当前出口 + 今日用量。 */
export function GatewayStatusStrip() {
  const { t } = useI18n();
  const [st, setSt] = useState<GatewayStatus | null>(null);
  const [degraded, setDegraded] = useState<Degraded[]>([]);

  useEffect(() => {
    if (IS_DEMO) {
      setSt({
        proxy_addr: "127.0.0.1:9527",
        version: "0.2.0",
        current: { claude: "Relay A", codex: "官方", gemini: null },
        today_tokens: 123_400,
        today_requests: 18,
      });
      return;
    }
    const load = () =>
      invoke<GatewayStatus>("gateway_status")
        .then(setSt)
        .catch(() => setSt(null));
    const loadDegraded = () =>
      invoke<Degraded[]>("get_degraded_providers")
        .then(setDegraded)
        .catch(() => setDegraded([]));
    load();
    loadDegraded();
    const id = setInterval(load, 30_000);
    const id2 = setInterval(loadDegraded, 60_000);
    return () => {
      clearInterval(id);
      clearInterval(id2);
    };
  }, []);

  if (!st) return null;

  return (
    <div className="space-y-2">
      {degraded.length > 0 && (
        <div className="flex items-center gap-2 rounded-xl border border-amber-500/40 bg-amber-500/10 px-4 py-2.5 text-xs text-amber-700 dark:text-amber-300">
          <AlertTriangle className="w-4 h-4 shrink-0" />
          <span>
            {t("gw.degraded", {
              names: degraded
                .map((d) => `${d.name}(${d.errors}/${d.total})`)
                .join(t("common.sep")),
            })}
          </span>
        </div>
      )}
      <div className="flex flex-wrap items-center gap-x-4 gap-y-2 rounded-xl border border-border bg-card px-4 py-3">
      <span className="flex items-center gap-2 text-sm font-semibold">
        <span className="relative flex h-2.5 w-2.5">
          <span className="absolute inline-flex h-full w-full animate-ping rounded-full bg-emerald-500 opacity-60" />
          <span className="relative inline-flex h-2.5 w-2.5 rounded-full bg-emerald-500" />
        </span>
        {t("gw.running")}
      </span>
      <span className="text-xs text-muted-foreground font-mono">
        {st.proxy_addr} · v{st.version}
      </span>
      <span className="h-4 w-px bg-border" />
      {(["claude", "codex", "gemini"] as const).map((k) => (
        <span key={k} className="flex items-center gap-1.5 text-xs">
          <span className="text-muted-foreground">{t(`app.${k}`)}</span>
          <span
            className={cn(
              "font-medium",
              st.current[k] ? "text-foreground" : "text-muted-foreground/60",
            )}
          >
            {st.current[k] ?? t("gw.none")}
          </span>
        </span>
      ))}
        {st.today_requests > 0 && (
          <>
            <span className="h-4 w-px bg-border" />
            <span className="text-xs text-muted-foreground tabular-nums">
              {t("gw.today", { tok: fmtTokens(st.today_tokens), n: st.today_requests })}
            </span>
          </>
        )}
      </div>
    </div>
  );
}
