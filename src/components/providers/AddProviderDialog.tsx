import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Plus } from "lucide-react";
import { useI18n } from "@/i18n";
import { Button } from "@/components/ui/button";
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

// 对话框结构复刻 CC Switch 的 AddProviderDialog;
// form 包住整个内容(含 footer),支持 Enter 提交
export function AddProviderDialog({
  open,
  onOpenChange,
  appId,
  onCreated,
  onError,
}: AddProviderDialogProps) {
  const { t } = useI18n();
  const [name, setName] = useState("");
  const [baseUrl, setBaseUrl] = useState("");
  const [apiKey, setApiKey] = useState("");
  const [urlError, setUrlError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  useEffect(() => {
    if (open) {
      setName("");
      setBaseUrl("");
      setApiKey("");
      setUrlError(null);
    }
  }, [open]);

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
          models: [],
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

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-[425px]">
        <form onSubmit={handleSubmit} className="grid gap-4">
          <DialogHeader>
            <DialogTitle>{t("dialog.addTitle")}</DialogTitle>
            <DialogDescription>
              {t("dialog.addDesc")}
            </DialogDescription>
          </DialogHeader>

          <div className="grid gap-2">
            <Label htmlFor="provider-name">{t("dialog.name")}</Label>
            <Input
              id="provider-name"
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder={t("dialog.namePh")}
              autoFocus
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
          </div>

          <div className="grid gap-2">
            <Label htmlFor="provider-key">{t("dialog.key")}</Label>
            <PasswordInput
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
              取消
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
      </DialogContent>
    </Dialog>
  );
}
