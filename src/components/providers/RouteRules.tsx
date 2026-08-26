import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ArrowRight, Plus, Route, X } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { useI18n } from "@/i18n";
import type { AppType, Provider } from "@/types";

interface RouteRule {
  id: number;
  app_type: string;
  pattern: string;
  provider_id: string;
}

/**
 * 模型路由规则:请求体 model 包含匹配词 → 路由到指定供应商(优先于当前供应商)。
 * 折叠态只占一行,不干扰主流程。
 */
export function RouteRules({
  app,
  providers,
  onChanged,
  onError,
}: {
  app: AppType;
  providers: Provider[];
  onChanged: () => void;
  onError: (msg: string) => void;
}) {
  const { t } = useI18n();
  const [rules, setRules] = useState<RouteRule[]>([]);
  const [open, setOpen] = useState(false);
  const [pattern, setPattern] = useState("");
  const [providerId, setProviderId] = useState("");
  const [adding, setAdding] = useState(false);

  useEffect(() => {
    invoke<RouteRule[]>("list_route_rules", { appType: app })
      .then(setRules)
      .catch(() => setRules([]));
  }, [app]);

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
      });
      setPattern("");
      setRules(
        await invoke<RouteRule[]>("list_route_rules", { appType: app }),
      );
      onChanged();
    } catch (e) {
      onError(String(e));
    } finally {
      setAdding(false);
    }
  };

  const remove = async (id: number) => {
    try {
      await invoke("delete_route_rule", { id });
      setRules(
        await invoke<RouteRule[]>("list_route_rules", { appType: app }),
      );
      onChanged();
    } catch (e) {
      onError(String(e));
    }
  };

  return (
    <div className="rounded-xl border border-dashed border-border p-3">
      <button
        type="button"
        onClick={() => setOpen((v) => !v)}
        className="flex items-center gap-2 text-xs font-medium text-muted-foreground w-full"
      >
        <Route className="w-3.5 h-3.5" />
        {t("route.title")}
        {rules.length > 0 && (
          <span className="rounded bg-blue-500/15 px-1.5 py-px text-[10px] text-blue-600 dark:text-blue-400">
            {rules.length}
          </span>
        )}
        <span className="ml-auto">{open ? "▴" : "▾"}</span>
      </button>

      {open && (
        <div className="mt-3 space-y-2">
          {rules.map((r) => (
            <div
              key={r.id}
              className="flex items-center gap-2 rounded-md bg-muted/50 px-2.5 py-1.5 text-xs"
            >
              <code className="font-mono">{r.pattern}</code>
              <ArrowRight className="w-3 h-3 text-muted-foreground" />
              <span className="font-medium">{nameOf(r.provider_id)}</span>
              <button
                type="button"
                onClick={() => void remove(r.id)}
                className="ml-auto text-muted-foreground hover:text-red-500"
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
          <p className="text-[11px] text-muted-foreground">{t("route.desc")}</p>
        </div>
      )}
    </div>
  );
}
