import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
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
import type { Provider } from "@/types";

interface EditProviderDialogProps {
  provider: Provider | null;
  onOpenChange: (open: boolean) => void;
  onSaved: (provider: Provider) => void;
  onError: (msg: string) => void;
}

function validateUrl(v: string): string | null {
  if (!v.trim()) return null;
  try {
    const u = new URL(v.trim());
    if (u.protocol !== "http:" && u.protocol !== "https:") {
      return "地址需以 http:// 或 https:// 开头";
    }
    return null;
  } catch {
    return "地址格式不正确,例如 https://api.example.com";
  }
}

// form 包住整个内容,支持 Enter 提交
export function EditProviderDialog({
  provider,
  onOpenChange,
  onSaved,
  onError,
}: EditProviderDialogProps) {
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
            <DialogTitle>编辑供应商</DialogTitle>
            <DialogDescription>
              API Key 留空表示保持不变;填写则更新到系统钥匙串。
            </DialogDescription>
          </DialogHeader>

          <div className="grid gap-2">
            <Label htmlFor="edit-name">名称</Label>
            <Input
              id="edit-name"
              value={name}
              onChange={(e) => setName(e.target.value)}
            />
          </div>

          <div className="grid gap-2">
            <Label htmlFor="edit-url">接口地址</Label>
            <Input
              id="edit-url"
              value={baseUrl}
              onChange={(e) => {
                setBaseUrl(e.target.value);
                if (urlError) setUrlError(validateUrl(e.target.value));
              }}
              onBlur={() => setUrlError(validateUrl(baseUrl))}
              aria-invalid={!urlValid}
              className={!urlValid ? "border-red-500 focus-visible:ring-red-500" : ""}
            />
            {urlError && <p className="text-xs text-red-500">{urlError}</p>}
          </div>

          <div className="grid gap-2">
            <Label htmlFor="edit-key">API Key(留空不变)</Label>
            <PasswordInput
              id="edit-key"
              value={apiKey}
              onChange={(e) => setApiKey(e.target.value)}
              placeholder="sk-..."
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
              {submitting ? "保存中…" : "保存"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
