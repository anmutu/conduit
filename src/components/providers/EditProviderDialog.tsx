import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Plus, X } from "lucide-react";
import { Button } from "@/components/ui/button";
import { useI18n } from "@/i18n";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { PasswordInput } from "@/components/PasswordInput";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import type { Protocol, Provider } from "@/types";

interface EditProviderDialogProps {
  provider: Provider | null;
  /** 预填:从置灰卡片"补配"进入时,默认选中的协议 */
  defaultProtocol?: Protocol;
  onOpenChange: (open: boolean) => void;
  onSaved: (provider: Provider) => void;
  onError: (msg: string) => void;
}

const PROTOCOLS: { value: Protocol; label: string }[] = [
  { value: "anthropic", label: "Anthropic" },
  { value: "openai", label: "OpenAI" },
  { value: "gemini", label: "Gemini" },
];

function validateUrl(v: string): string | null {
  if (!v.trim()) return null;
  try {
    const u = new URL(v.trim());
    if (u.protocol !== "http:" && u.protocol !== "https:") return "scheme";
    return null;
  } catch {
    return "invalid";
  }
}

// form 包住整个内容,支持 Enter 提交
export function EditProviderDialog({
  provider,
  defaultProtocol,
  onOpenChange,
  onSaved,
  onError,
}: EditProviderDialogProps) {
  const { t } = useI18n();
  const [name, setName] = useState("");
  /** 三行端点编辑状态:protocol -> url(url 为空字符串表示该行新增未提交) */
  const [endpoints, setEndpoints] = useState<Record<string, string>>({});
  const [apiKey, setApiKey] = useState("");
  const [models, setModels] = useState("");
  const [urlError, setUrlError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);
  /** /v1/responses → chat/completions 桥接(仅 codex 供应商有意义) */
  const [bridge, setBridge] = useState(false);

  useEffect(() => {
    if (provider) {
      setName(provider.name);
      if (provider.app_type === "codex") {
        invoke<boolean>("get_responses_bridge", { id: provider.id })
          .then(setBridge)
          .catch(() => setBridge(false));
      } else {
        setBridge(false);
      }
      // 兼容旧数据:endpoints 为空时退回 base_url 记为 anthropic
      const eps = { ...provider.endpoints };
      if (Object.keys(eps).length === 0 && provider.base_url) {
        eps.anthropic = provider.base_url;
      }
      // "补配"场景:默认带出目标协议空行
      if (defaultProtocol && !(defaultProtocol in eps)) {
        eps[defaultProtocol] = "";
      }
      setEndpoints(eps);
      setApiKey("");
      setModels(provider.models.join(", "));
      setUrlError(null);
    }
  }, [provider, defaultProtocol]);

  const urlValid = urlError === null;
  const filledCount = Object.values(endpoints).filter((v) => v.trim() !== "").length;
  const canSubmit = Boolean(provider) && name.trim() !== "" && urlValid && filledCount >= 1;

  const setEndpoint = (proto: string, v: string) => {
    setEndpoints((prev) => ({ ...prev, [proto]: v }));
    if (urlError) setUrlError(validateUrl(v));
  };

  const addEndpointRow = (proto: Protocol) => {
    setEndpoints((prev) => (proto in prev ? prev : { ...prev, [proto]: "" }));
  };
  const removeEndpointRow = (proto: string) => {
    setEndpoints((prev) => {
      const next = { ...prev };
      delete next[proto];
      return next;
    });
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!provider || !canSubmit || submitting) return;
    setSubmitting(true);
    try {
      await invoke("update_provider", { id: provider.id, name: name.trim() });
      // 端点逐协议保存:与原值不同的才调用
      const original = { ...provider.endpoints };
      for (const { value: proto } of PROTOCOLS) {
        const next = (endpoints[proto] ?? "").trim();
        const prev = original[proto] ?? "";
        if (next === prev) continue;
        if (next === "") {
          if (prev !== "") {
            await invoke("remove_provider_endpoint", { id: provider.id, protocol: proto });
          }
        } else {
          await invoke("upsert_provider_endpoint", {
            id: provider.id,
            protocol: proto,
            baseUrl: next,
          });
        }
      }
      // Key 留空表示不变更
      if (apiKey.trim()) {
        await invoke("set_provider_key", {
          id: provider.id,
          apiKey: apiKey.trim(),
        });
      }
      if (provider.app_type === "codex") {
        await invoke("set_responses_bridge", { id: provider.id, enabled: bridge });
      }
      await invoke("set_provider_models", {
        id: provider.id,
        models: models.split(",").map((m) => m.trim()).filter(Boolean),
      });
      onSaved(provider);
      onOpenChange(false);
    } catch (err) {
      onError(String(err));
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <Dialog open={Boolean(provider)} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-[460px]">
        <form onSubmit={handleSubmit} className="grid gap-4">
          <DialogHeader>
            <DialogTitle>{t("dialog.editTitle")}</DialogTitle>
            <DialogDescription>{t("dialog.editDesc")}</DialogDescription>
          </DialogHeader>

          <div className="grid gap-2">
            <Label htmlFor="edit-name">{t("dialog.name")}</Label>
            <Input
              id="edit-name"
              value={name}
              onChange={(e) => setName(e.target.value)}
            />
          </div>

          {/* 多协议端点:每个协议一行,可增删 */}
          <div className="grid gap-2">
            <Label>{t("dialog.endpoints")}</Label>
            <div className="grid gap-2">
              {PROTOCOLS.filter((p) => p.value in endpoints).map(({ value: proto, label }) => (
                <div key={proto} className="grid gap-1">
                  <div className="flex items-center justify-between">
                    <span className="text-xs font-medium text-muted-foreground">{label}</span>
                    <button
                      type="button"
                      onClick={() => removeEndpointRow(proto)}
                      className="inline-flex items-center gap-0.5 text-xs text-muted-foreground hover:text-red-500 transition-colors"
                    >
                      <X className="w-3 h-3" />
                      {t("common.remove")}
                    </button>
                  </div>
                  <Input
                    value={endpoints[proto]}
                    onChange={(e) => setEndpoint(proto, e.target.value)}
                    onBlur={() => setUrlError(validateUrl(endpoints[proto] ?? ""))}
                    aria-invalid={!urlValid}
                    className={!urlValid ? "border-red-500 focus-visible:ring-red-500" : ""}
                    placeholder={t("dialog.urlPh")}
                  />
                </div>
              ))}
            </div>
            {/* 未配置的协议:按钮形式追加 */}
            <div className="flex flex-wrap gap-1.5">
              {PROTOCOLS.filter((p) => !(p.value in endpoints)).map(({ value: proto, label }) => (
                <Button
                  key={proto}
                  type="button"
                  variant="outline"
                  size="sm"
                  className="h-7 px-2 text-xs"
                  onClick={() => addEndpointRow(proto)}
                >
                  <Plus className="w-3 h-3 mr-1" />
                  {label}
                </Button>
              ))}
            </div>
            {urlError && (
              <p className="text-xs text-red-500">{t(urlError === "scheme" ? "url.scheme" : "url.invalid")}</p>
            )}
          </div>

          <div className="grid gap-2">
            <Label htmlFor="edit-key">{t("dialog.editKey")}</Label>
            <PasswordInput
              id="edit-key"
              value={apiKey}
              onChange={(e) => setApiKey(e.target.value)}
              placeholder={t("dialog.keyPh")}
            />
          </div>

          <div className="grid gap-2">
            <Label htmlFor="edit-models">{t("dialog.models")}</Label>
            <Input
              id="edit-models"
              value={models}
              onChange={(e) => setModels(e.target.value)}
              placeholder={t("dialog.modelsPh")}
            />
          </div>

          {provider?.app_type === "codex" && (
            <label className="flex items-start gap-2 text-xs text-muted-foreground">
              <input
                type="checkbox"
                checked={bridge}
                onChange={(e) => setBridge(e.target.checked)}
                className="mt-0.5"
              />
              <span>{t("dialog.responsesBridge")}</span>
            </label>
          )}

          <DialogFooter>
            <Button
              type="button"
              variant="outline"
              onClick={() => onOpenChange(false)}
            >
              {t("common.cancel")}
            </Button>
            <Button type="submit" disabled={!canSubmit || submitting}>
              {submitting ? t("dialog.saving") : t("common.save")}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
