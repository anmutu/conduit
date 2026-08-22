import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Plus } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
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
  onCreated: () => void;
  onError: (msg: string) => void;
}

// 对话框结构复刻 CC Switch 的 AddProviderDialog(shadcn Dialog + 表单)
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
  const [submitting, setSubmitting] = useState(false);

  useEffect(() => {
    if (open) {
      setName("");
      setBaseUrl("");
      setApiKey("");
    }
  }, [open]);

  const canSubmit = name.trim() !== "" && baseUrl.trim() !== "";

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!canSubmit || submitting) return;
    setSubmitting(true);
    try {
      await invoke<Provider>("create_provider", {
        input: {
          app_type: appId,
          name: name.trim(),
          base_url: baseUrl.trim(),
          models: [],
          api_key: apiKey.trim() || undefined,
        },
      });
      onCreated();
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
        <DialogHeader>
          <DialogTitle>添加供应商</DialogTitle>
          <DialogDescription>
            API Key 将存入系统钥匙串(Keychain),不落盘、不入库。
          </DialogDescription>
        </DialogHeader>
        <form onSubmit={handleSubmit} className="grid gap-4 py-2">
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
              onChange={(e) => setBaseUrl(e.target.value)}
              placeholder="https://api.example.com"
            />
          </div>
          <div className="grid gap-2">
            <Label htmlFor="provider-key">API Key(可选)</Label>
            <Input
              id="provider-key"
              type="password"
              value={apiKey}
              onChange={(e) => setApiKey(e.target.value)}
              placeholder="sk-..."
            />
          </div>
        </form>
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
            disabled={!canSubmit || submitting}
            onClick={(e) => void handleSubmit(e)}
          >
            <Plus className="w-4 h-4 mr-1" />
            {submitting ? "添加中…" : "添加"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
