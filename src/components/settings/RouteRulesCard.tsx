import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ArrowRight, ChevronDown, ChevronUp, Plus, Route, X } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";
import { ProviderIcon } from "@/components/ProviderIcon";
import { useI18n } from "@/i18n";
import { cn } from "@/lib/utils";
import { loadApps } from "@/lib/appPrefs";
import type { AppType, Provider } from "@/types";

interface RouteRule {
  id: number;
  app_type: string;
  pattern: string;
  provider_id: string;
  enabled: boolean;
  match_type: string;
  fallback_provider_id: string | null;
  priority: number;
}

/**
 * 模型路由设置卡:请求体 model 匹配关键词 → 优先路由到指定供应商。
 * 自带 app 分组切换与供应商加载,可独立嵌在设置页。
 */
export function RouteRulesCard({ onError }: { onError: (msg: string) => void }) {
  const { t } = useI18n();
  const [apps, setApps] = useState<AppType[]>([]);
  const [app, setApp] = useState<AppType>("codex");
  const [providers, setProviders] = useState<Provider[]>([]);
  const [rules, setRules] = useState<RouteRule[]>([]);
  const [pattern, setPattern] = useState("");
  const [providerId, setProviderId] = useState("");
  const [matchType, setMatchType] = useState("contains");
  const [adding, setAdding] = useState(false);
  // 长上下文分流预设
  const [lcProvider, setLcProvider] = useState("");
  const [lcThreshold, setLcThreshold] = useState("60000");
  // 后台轻量分流预设
  const [bgProvider, setBgProvider] = useState("");

  const reload = (a: AppType) => {
    invoke<RouteRule[]>("list_route_rules", { appType: a })
      .then(setRules)
      .catch(() => setRules([]));
    invoke<Provider[]>("list_providers", { appType: a })
      .then(setProviders)
      .catch(() => setProviders([]));
    invoke<{ provider_id: string; threshold: number } | null>(
      "get_longctx_preset",
      { appType: a },
    )
      .then((p) => {
        setLcProvider(p?.provider_id ?? "");
        setLcThreshold(String(p?.threshold ?? 60000));
      })
      .catch(() => setLcProvider(""));
    invoke<string | null>("get_background_preset", { appType: a })
      .then((pid) => setBgProvider(pid ?? ""))
      .catch(() => setBgProvider(""));
  };

  useEffect(() => {
    const list = loadApps();
    setApps(list);
    setApp(list[0]);
    reload(list[0]);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const nameOf = (id: string) =>
    providers.find((p) => p.id === id)?.name ?? id.slice(0, 8);

  const add = async () => {
    if (!pattern.trim() || !providerId) return;
    setAdding(true);
    try {
      await invoke("add_route_rule", {
        appType: app,
        pattern: pattern.trim(),
        providerId,
        matchType,
      });
      setPattern("");
      setRules(await invoke<RouteRule[]>("list_route_rules", { appType: app }));
    } catch (e) {
      onError(String(e));
    } finally {
      setAdding(false);
    }
  };

  /** 上移/下移(与相邻规则交换优先级,列表即匹配顺序) */
  const move = async (id: number, dir: number) => {
    try {
      await invoke("move_route_rule", { id, dir });
      setRules(await invoke<RouteRule[]>("list_route_rules", { appType: app }));
    } catch (e) {
      onError(String(e));
    }
  };

  const remove = async (id: number) => {
    try {
      await invoke("delete_route_rule", { id });
      setRules(await invoke<RouteRule[]>("list_route_rules", { appType: app }));
    } catch (e) {
      onError(String(e));
    }
  };

  const toggle = async (r: RouteRule, enabled: boolean) => {
    // 乐观更新,失败回滚并提示
    setRules((rs) => rs.map((x) => (x.id === r.id ? { ...x, enabled } : x)));
    try {
      await invoke("set_route_rule_enabled", { id: r.id, enabled });
    } catch (e) {
      onError(String(e));
      setRules((rs) => rs.map((x) => (x.id === r.id ? { ...x, enabled: !enabled } : x)));
    }
  };

  const saveFallback = async (r: RouteRule, pid: string) => {
    setRules((rs) =>
      rs.map((x) => (x.id === r.id ? { ...x, fallback_provider_id: pid || null } : x)),
    );
    try {
      await invoke("set_route_rule_fallback", { id: r.id, providerId: pid || null });
    } catch (e) {
      onError(String(e));
    }
  };

  const saveBackground = async (pid: string) => {
    setBgProvider(pid);
    try {
      await invoke("set_background_preset", {
        appType: app,
        providerId: pid,
      });
    } catch (e) {
      onError(String(e));
    }
  };

  const saveLongctx = async (pid: string) => {
    setLcProvider(pid);
    try {
      await invoke("set_longctx_preset", {
        appType: app,
        providerId: pid,
        threshold: Number(lcThreshold) || 60000,
      });
    } catch (e) {
      onError(String(e));
    }
  };

  return (
    <div className="rounded-xl border border-border bg-card p-4">
      <div className="flex items-start gap-3">
        <div className="mt-0.5 text-muted-foreground">
          <Route className="w-4 h-4" />
        </div>
        <div className="flex-1 min-w-0">
          <div className="text-sm font-medium">{t("route.title")}</div>
          <div className="text-xs text-muted-foreground mb-3">
            {t("route.cardDesc")}
          </div>

          {/* app 分组切换 */}
          {apps.length > 1 && (
            <div className="flex items-center gap-1 p-1 bg-muted rounded-lg w-fit mb-3">
              {apps.map((a) => (
                <button
                  key={a}
                  type="button"
                  onClick={() => {
                    setApp(a);
                    setProviderId("");
                    reload(a);
                  }}
                  className={cn(
                    "inline-flex items-center gap-1 px-2.5 h-7 rounded-md text-xs font-medium transition-all",
                    app === a
                      ? "bg-background text-foreground shadow-sm"
                      : "text-muted-foreground hover:text-foreground",
                  )}
                >
                  <ProviderIcon icon={a} name={a} size={14} />
                  <span className="capitalize">{a}</span>
                </button>
              ))}
            </div>
          )}

          <div className="space-y-2">
            {rules.map((r, i) => (
              <div
                key={r.id}
                className={cn(
                  "flex items-center gap-2 rounded-md bg-muted/50 px-2.5 py-1.5 text-xs",
                  !r.enabled && "opacity-55",
                )}
              >
                <code className="font-mono">{r.pattern}</code>
                <span className="text-[10px] text-muted-foreground">
                  {r.match_type === "starts_with"
                    ? t("route.matchStartsWith")
                    : t("route.matchContains")}
                </span>
                <ArrowRight className="w-3 h-3 text-muted-foreground" />
                <span className="font-medium">{nameOf(r.provider_id)}</span>
                <select
                  value={r.fallback_provider_id ?? ""}
                  onChange={(e) => void saveFallback(r, e.target.value)}
                  className="h-6 rounded border border-border bg-background px-1 text-[10px] text-muted-foreground max-w-[110px]"
                  title={t("route.fallbackTitle")}
                >
                  <option value="">{t("route.fallbackNone")}</option>
                  {providers
                    .filter((p) => p.id !== r.provider_id)
                    .map((p) => (
                      <option key={p.id} value={p.id}>
                        ↩ {p.name}
                      </option>
                    ))}
                </select>
                <div className="ml-auto flex items-center gap-0.5" title={t("route.orderTitle")}>
                  <button
                    type="button"
                    disabled={i === 0}
                    onClick={() => void move(r.id, -1)}
                    className="text-muted-foreground hover:text-foreground disabled:opacity-30"
                  >
                    <ChevronUp className="w-3.5 h-3.5" />
                  </button>
                  <button
                    type="button"
                    disabled={i === rules.length - 1}
                    onClick={() => void move(r.id, 1)}
                    className="text-muted-foreground hover:text-foreground disabled:opacity-30"
                  >
                    <ChevronDown className="w-3.5 h-3.5" />
                  </button>
                </div>
                <Switch
                  className="scale-75"
                  checked={r.enabled}
                  onCheckedChange={(v) => void toggle(r, v)}
                />
                <button
                  type="button"
                  onClick={() => void remove(r.id)}
                  className="text-muted-foreground hover:text-red-500"
                >
                  <X className="w-3.5 h-3.5" />
                </button>
              </div>
            ))}
            <div className="flex items-center gap-2">
              <Input
                value={pattern}
                onChange={(e) => setPattern(e.target.value)}
                placeholder={t("route.patternPh")}
                className="h-8 text-xs flex-1"
                onKeyDown={(e) => e.key === "Enter" && void add()}
              />
              <select
                value={matchType}
                onChange={(e) => setMatchType(e.target.value)}
                className="h-8 rounded-md border border-border bg-background px-2 text-xs"
                title={t("route.matchTypeTitle")}
              >
                <option value="contains">{t("route.matchContains")}</option>
                <option value="starts_with">{t("route.matchStartsWith")}</option>
              </select>
              <select
                value={providerId}
                onChange={(e) => setProviderId(e.target.value)}
                className="h-8 rounded-md border border-border bg-background px-2 text-xs"
              >
                <option value="">{t("route.pickProvider")}</option>
                {providers.map((p) => (
                  <option key={p.id} value={p.id}>
                    {p.name}
                  </option>
                ))}
              </select>
              <Button
                size="sm"
                className="h-8 px-2.5"
                disabled={adding || !pattern.trim() || !providerId}
                onClick={() => void add()}
              >
                <Plus className="w-3.5 h-3.5" />
              </Button>
            </div>
            {/* 长上下文分流预设 */}
            <div className="flex items-center gap-2 flex-wrap rounded-md border border-dashed border-border px-2.5 py-2 text-xs">
              <span className="text-muted-foreground">{t("route.longctxLabel")}</span>
              <select
                value={lcProvider}
                onChange={(e) => void saveLongctx(e.target.value)}
                className="h-7 rounded-md border border-border bg-background px-2 text-xs"
              >
                <option value="">{t("route.longctxOff")}</option>
                {providers.map((p) => (
                  <option key={p.id} value={p.id}>
                    {p.name}
                  </option>
                ))}
              </select>
              {lcProvider && (
                <>
                  <span className="text-muted-foreground">{t("route.longctxThreshold")}</span>
                  <Input
                    value={lcThreshold}
                    onChange={(e) => setLcThreshold(e.target.value.replace(/\D/g, ""))}
                    onBlur={() => void saveLongctx(lcProvider)}
                    className="h-7 w-20 text-xs"
                  />
                  <span className="text-muted-foreground">tokens</span>
                </>
              )}
            </div>
            {/* 后台轻量分流预设 */}
            <div className="flex items-center gap-2 flex-wrap rounded-md border border-dashed border-border px-2.5 py-2 text-xs">
              <span className="text-muted-foreground">{t("route.bgLabel")}</span>
              <select
                value={bgProvider}
                onChange={(e) => void saveBackground(e.target.value)}
                className="h-7 rounded-md border border-border bg-background px-2 text-xs"
              >
                <option value="">{t("route.bgOff")}</option>
                {providers.map((p) => (
                  <option key={p.id} value={p.id}>
                    {p.name}
                  </option>
                ))}
              </select>
              <span className="text-muted-foreground">{t("route.bgHint")}</span>
            </div>
            <p className="text-[11px] text-muted-foreground leading-relaxed">
              {t("route.desc")}
            </p>
          </div>
        </div>
      </div>
    </div>
  );
}
