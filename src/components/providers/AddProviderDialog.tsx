import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Plus } from "lucide-react";
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
function validateUrl(v: string): string | null {
  if (!v.trim()) return null; // 空值交给必填校验
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

// 对话框结构复刻 CC Switch 的 AddProviderDialog;
// form 包住整个内容(含 footer),支持 Enter 提交
export function AddProviderDialog({
  open,
  onOpenChange,
  appId,
  onCreated,
  onError,
}: AddProviderDialogProps) {
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
    const err = validateUrl(baseUrl);
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
            <DialogTitle>添加供应商</DialogTitle>
            <DialogDescription>
              API Key 将存入系统钥匙串(Keychain),不落盘、不入库。
            </DialogDescription>
          </DialogHeader>

          <div className="grid gap-2">
            <Label htmlFor="provider-name">名称</Label>
            <Input
              id="provider-name"
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="例如 CoderPlan"
              autoFocus
            />
          </div>

          <div className="grid gap-2">
            <Label htmlFor="provider-url">接口地址</Label>
            <Input
              id="provider-url"
              value={baseUrl}
              onChange={(e) => {
                setBaseUrl(e.target.value);
                if (urlError) setUrlError(validateUrl(e.target.value));
              }}
              onBlur={() => setUrlError(validateUrl(baseUrl))}
              placeholder="https://api.example.com"
              aria-invalid={!urlValid}
              className={!urlValid ? "border-red-500 focus-visible:ring-red-500" : ""}
            />
            {urlError && (
              <p className="text-xs text-red-500">{urlError}</p>
            )}
          </div>

          <div className="grid gap-2">
            <Label htmlFor="provider-key">API Key(可选)</Label>
            <PasswordInput
              id="provider-key"
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
            <Button
              type="submit"
              disabled={
                submitting || name.trim() === "" || baseUrl.trim() === "" || !urlValid
              }
            >
              <Plus className="w-4 h-4 mr-1" />
              {submitting ? "添加中…" : "添加"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
