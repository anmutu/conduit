import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
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
import type { Provider } from "@/types";

interface EditProviderDialogProps {
  provider: Provider | null;
  onOpenChange: (open: boolean) => void;
  onSaved: (provider: Provider) => void;
  onError: (msg: string) => void;
}

function validateUrl(v: string, t: ReturnType<typeof useI18n>["t"]): string | null {
  if (!v.trim()) return null;
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

// form 包住整个内容,支持 Enter 提交
export function EditProviderDialog({
  provider,
  onOpenChange,
  onSaved,
  onError,
}: EditProviderDialogProps) {
  const { t } = useI18n();
  const [name, setName] = useState("");
  const [baseUrl, setBaseUrl] = useState("");
  const [apiKey, setApiKey] = useState("");
  const [urlError, setUrlError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  useEffect(() => {
    if (provider) {
      setName(provider.name);
      setBaseUrl(provider.base_url);
      setApiKey("");
      setUrlError(null);
    }
  }, [provider]);

  const urlValid = urlError === null;
  const canSubmit =
    Boolean(provider) && name.trim() !== "" && baseUrl.trim() !== "" && urlValid;

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!provider || !canSubmit || submitting) return;
    setSubmitting(true);
    try {
      await invoke("update_provider", {
        id: provider.id,
        name: name.trim(),
        baseUrl: baseUrl.trim(),
      });
      // Key 留空表示不变更
      if (apiKey.trim()) {
        await invoke("set_provider_key", {
          id: provider.id,
          apiKey: apiKey.trim(),
        });
      }
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
      <DialogContent className="sm:max-w-[425px]">
        <form onSubmit={handleSubmit} className="grid gap-4">
          <DialogHeader>
            <DialogTitle>{t("dialog.editTitle")}</DialogTitle>
            <DialogDescription>
              {t("dialog.editDesc")}
            </DialogDescription>
          </DialogHeader>

          <div className="grid gap-2">
            <Label htmlFor="edit-name">{t("dialog.name")}</Label>
            <Input
              id="edit-name"
              value={name}
              onChange={(e) => setName(e.target.value)}
            />
          </div>

          <div className="grid gap-2">
            <Label htmlFor="edit-url">{t("dialog.url")}</Label>
            <Input
              id="edit-url"
              value={baseUrl}
              onChange={(e) => {
                setBaseUrl(e.target.value);
                if (urlError) setUrlError(validateUrl(e.target.value, t));
              }}
              onBlur={() => setUrlError(validateUrl(baseUrl, t))}
              aria-invalid={!urlValid}
              className={!urlValid ? "border-red-500 focus-visible:ring-red-500" : ""}
            />
            {urlError && <p className="text-xs text-red-500">{urlError}</p>}
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

          <DialogFooter>
            <Button
              type="button"
              variant="outline"
              onClick={() => onOpenChange(false)}
            >
              取消
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
