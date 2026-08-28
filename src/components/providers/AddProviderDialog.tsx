import { useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ArrowLeft, ExternalLink, Plus, Search } from "lucide-react";
import { useI18n } from "@/i18n";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { PasswordInput } from "@/components/PasswordInput";
import { ProviderIcon } from "@/components/ProviderIcon";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  providerPresets,
  type ProviderPreset,
} from "@/data/providerPresets";
import { loadPresetOrder, savePresetOrder } from "@/lib/appPrefs";
import { cn } from "@/lib/utils";
import type { AppType, Provider } from "@/types";

interface AddProviderDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  appId: AppType;
  onCreated: (provider: Provider) => void;
  onError: (msg: string) => void;
}

/** 校验接口地址:必须是合法 http(s) URL */
function validateUrl(v: string, t: ReturnType<typeof useI18n>["t"]): string | null {
  if (!v.trim()) return null; // 空值交给必填校验
  try {
    const u = new URL(v.trim());
    if (u.protocol !== "http:" && u.protocol !== "https:") {
      return t("url.scheme");
    }
    return null;
  } catch {
    return t("url.invalid");
  }
}

// 添加供应商对话框;
// form 包住整个内容(含 footer),支持 Enter 提交。
// 两步式:先选预设(或自定义),再进入预填表单。
export function AddProviderDialog({
  open,
  onOpenChange,
  appId,
  onCreated,
  onError,
}: AddProviderDialogProps) {
  const { t } = useI18n();
  const [preset, setPreset] = useState<ProviderPreset | null>(null);
  const [search, setSearch] = useState("");
  const [name, setName] = useState("");
  const [baseUrl, setBaseUrl] = useState("");
  const [apiKey, setApiKey] = useState("");
  const [urlError, setUrlError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);
  const keyInputRef = useRef<HTMLInputElement>(null);

  // 拖拽排序:扁平列表全局排序;持久化到 localStorage
  const [presetOrders, setPresetOrders] = useState<
    Record<string, string[]>
  >(() => loadPresetOrder().presets);
  const [dragPreset, setDragPreset] = useState<string | null>(null);
  const [dragOverPreset, setDragOverPreset] = useState<string | null>(null);

  const persist = (orders: Record<string, string[]>) => {
    savePresetOrder({ categories: [], presets: orders });
  };

  const dropPreset = (targetName: string) => {
    if (!dragPreset) return;
    const list = providerPresets[appId] ?? [];
    if (dragPreset === targetName || !list.some((p) => p.name === targetName)) {
      return setDragPreset(null);
    }
    const names = list.map((p) => p.name);
    const order = presetOrders[appId] ?? [];
    const base = order.length ? order.filter((n) => names.includes(n)) : names;
    const next = base.filter((n) => n !== dragPreset);
    next.splice(next.indexOf(targetName), 0, dragPreset);
    const orders = { ...presetOrders, [appId]: next };
    setPresetOrders(orders);
    persist(orders);
    setDragPreset(null);
    setDragOverPreset(null);
  };

  useEffect(() => {
    if (open) {
      setPreset(null);
      setSearch("");
      setName("");
      setBaseUrl("");
      setApiKey("");
      setUrlError(null);
    }
  }, [open]);

  // 选预设 → 预填并进入表单;自定义 → 空表单
  const pick = (p: ProviderPreset | null) => {
    setPreset(p);
    setName(p?.name ?? "");
    setBaseUrl(p?.baseUrl ?? "");
    setUrlError(null);
    // 预填后焦点落到 API Key;自定义则从名称开始
    requestAnimationFrame(() => {
      if (p) keyInputRef.current?.focus();
    });
  };

  // 扁平预设列表:用户拖拽过的名称优先,其余按默认顺序追加
  const presets = useMemo(() => {
    const list = providerPresets[appId] ?? [];
    const q = search.trim().toLowerCase();
    const filtered = q
      ? list.filter(
          (p) =>
            p.name.toLowerCase().includes(q) || p.baseUrl.toLowerCase().includes(q),
        )
      : list;
    const savedOrder = presetOrders[appId] ?? [];
    const byName = new Map(
      list.map((p) => [p.name, p] as const),
    );
    const orderedNames = savedOrder.filter((n) => byName.has(n));
    for (const p of list) if (!orderedNames.includes(p.name)) orderedNames.push(p.name);
    return filtered.slice().sort(
      (a, b) => orderedNames.indexOf(a.name) - orderedNames.indexOf(b.name),
    );
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [appId, search, presetOrders]);

  const urlValid = urlError === null;

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    const err = validateUrl(baseUrl, t);
    setUrlError(err);
    if (name.trim() === "" || baseUrl.trim() === "" || err) return;
    if (submitting) return;
    setSubmitting(true);
    try {
      const created = await invoke<Provider>("create_provider", {
        input: {
          app_type: appId,
          name: name.trim(),
          base_url: baseUrl.trim(),
          models: preset?.models ?? [],
          api_key: apiKey.trim() || undefined,
        },
      });
      onCreated(created);
      onOpenChange(false);
    } catch (err) {
      onError(String(err));
    } finally {
      setSubmitting(false);
    }
  };

  const step = preset === null ? "pick" : "form";

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-[425px]">
        {step === "pick" ? (
          <div className="grid gap-4">
            <DialogHeader>
              <DialogTitle>{t("dialog.addTitle")}</DialogTitle>
              <DialogDescription>{t("preset.pickDesc")}</DialogDescription>
            </DialogHeader>

            <div className="relative">
              <Search className="absolute left-2.5 top-1/2 -translate-y-1/2 w-4 h-4 text-muted-foreground" />
              <Input
                value={search}
                onChange={(e) => setSearch(e.target.value)}
                placeholder={t("preset.searchPh")}
                className="pl-8"
                autoFocus
              />
            </div>

            <div className="grid gap-1.5 max-h-[52vh] overflow-y-auto pr-1">
              {presets.map((p) => (
                <button
                  key={p.name}
                  type="button"
                  draggable={!search.trim()}
                  onDragStart={() => setDragPreset(p.name)}
                  onDragEnd={() => { setDragPreset(null); setDragOverPreset(null); }}
                  onDragOver={(e) => {
                    if (dragPreset) { e.preventDefault(); setDragOverPreset(p.name); }
                  }}
                  onDragLeave={() => setDragOverPreset(null)}
                  onDrop={(e) => {
                    e.stopPropagation();
                    dropPreset(p.name);
                  }}
                  onClick={() => pick(p)}
                  className={cn(
                    "flex items-center gap-2.5 rounded-md border px-3 py-2 text-left text-sm transition-colors hover:bg-accent",
                    !search.trim() && "cursor-grab active:cursor-grabbing",
                    dragOverPreset === p.name && dragPreset !== null && dragPreset !== p.name &&
                      "border-blue-500/60 ring-1 ring-blue-500/40",
                  )}
                >
                  <ProviderIcon
                    icon={p.icon ?? ""}
                    name={p.name}
                    size={18}
                  />
                  <span className="min-w-0 flex-1">
                    <span className="flex items-center gap-1.5">
                      <span className="truncate font-medium">{p.name}</span>
                      {p.partner && (
                        <span className="shrink-0 rounded bg-amber-500/15 px-1 py-px text-[10px] font-medium text-amber-600 dark:text-amber-400">
                          {t("preset.badge.partner")}
                        </span>
                      )}
                    </span>
                    <span className="block truncate text-xs text-muted-foreground">
                      {p.baseUrl}
                    </span>
                  </span>
                  {p.apiKeyUrl && (
                    <a
                      href={p.apiKeyUrl}
                      target="_blank"
                      rel="noreferrer"
                      title={t("preset.getKey")}
                      onClick={(e) => e.stopPropagation()}
                      className="shrink-0 rounded-md p-1.5 text-muted-foreground transition-colors hover:text-foreground"
                    >
                      <ExternalLink className="w-3.5 h-3.5" />
                    </a>
                  )}
                </button>
              ))}

              {/* 自定义:永远可用 */}
              <button
                type="button"
                onClick={() => pick(null)}
                className="flex items-center rounded-md border border-dashed px-3 py-2 text-left text-sm text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
              >
                <Plus className="w-4 h-4 mr-2" />
                {t("preset.customItem")}
              </button>
            </div>
          </div>
        ) : (
          <form onSubmit={handleSubmit} className="grid gap-4">
            <DialogHeader>
              <DialogTitle>{t("dialog.addTitle")}</DialogTitle>
              <DialogDescription>
                {preset
                  ? t("preset.formDesc", { name: preset.name })
                  : t("dialog.addDesc")}
              </DialogDescription>
            </DialogHeader>

            {/* 预设来源 + 返回重选 */}
            {preset && (
              <div className="flex items-center gap-2.5 rounded-md bg-accent px-3 py-2 text-sm">
                <ProviderIcon
                  icon={preset.icon ?? ""}
                  name={preset.name}
                  size={16}
                />
                <span className="min-w-0 flex-1 truncate font-medium">{preset.name}</span>
                <Button
                  type="button"
                  variant="ghost"
                  size="sm"
                  onClick={() => pick(null)}
                  className="h-7 px-2 text-xs"
                >
                  <ArrowLeft className="w-3.5 h-3.5 mr-1" />
                  {t("preset.repick")}
                </Button>
              </div>
            )}

            <div className="grid gap-2">
              <Label htmlFor="provider-name">{t("dialog.name")}</Label>
              <Input
                id="provider-name"
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder={t("dialog.namePh")}
                autoFocus={!preset}
              />
            </div>

            <div className="grid gap-2">
              <Label htmlFor="provider-url">{t("dialog.url")}</Label>
              <Input
                id="provider-url"
                value={baseUrl}
                onChange={(e) => {
                  setBaseUrl(e.target.value);
                  if (urlError) setUrlError(validateUrl(e.target.value, t));
                }}
                onBlur={() => setUrlError(validateUrl(baseUrl, t))}
                placeholder={t("dialog.urlPh")}
                aria-invalid={!urlValid}
                className={!urlValid ? "border-red-500 focus-visible:ring-red-500" : ""}
              />
              {urlError && (
                <p className="text-xs text-red-500">{urlError}</p>
              )}
              {preset?.models && preset.models.length > 0 && (
                <p className="text-xs text-muted-foreground">
                  {t("preset.modelsHint")}: {preset.models.join(" · ")}
                </p>
              )}
            </div>

            <div className="grid gap-2">
              <div className="flex items-center justify-between">
                <Label htmlFor="provider-key">{t("dialog.key")}</Label>
                {preset?.apiKeyUrl && (
                  <a
                    href={preset.apiKeyUrl}
                    target="_blank"
                    rel="noreferrer"
                    className="flex items-center gap-1 text-xs text-blue-500 hover:underline"
                  >
                    {t("preset.getKey")}
                    <ExternalLink className="w-3 h-3" />
                  </a>
                )}
              </div>
              <PasswordInput
                ref={keyInputRef}
                id="provider-key"
                value={apiKey}
                onChange={(e) => setApiKey(e.target.value)}
                placeholder={t("dialog.keyPh")}
              />
            </div>

            <DialogFooter>
              <Button
                type="button"
                variant="outline"
                onClick={() => onOpenChange(false)}
              >
                {t("common.cancel")}
              </Button>
              <Button
                type="submit"
                disabled={
                  submitting || name.trim() === "" || baseUrl.trim() === "" || !urlValid
                }
              >
                <Plus className="w-4 h-4 mr-1" />
                {submitting ? t("dialog.adding") : t("common.add")}
              </Button>
            </DialogFooter>
          </form>
        )}
      </DialogContent>
    </Dialog>
  );
}
