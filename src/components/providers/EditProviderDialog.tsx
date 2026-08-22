import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
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
import type { Provider } from "@/types";

interface EditProviderDialogProps {
  provider: Provider | null;
  onOpenChange: (open: boolean) => void;
  onSaved: () => void;
  onError: (msg: string) => void;
}

export function EditProviderDialog({
  provider,
  onOpenChange,
  onSaved,
  onError,
}: EditProviderDialogProps) {
  const [name, setName] = useState("");
  const [baseUrl, setBaseUrl] = useState("");
  const [apiKey, setApiKey] = useState("");
  const [submitting, setSubmitting] = useState(false);

  useEffect(() => {
    if (provider) {
      setName(provider.name);
      setBaseUrl(provider.base_url);
      setApiKey("");
    }
  }, [provider]);

  const canSubmit = Boolean(provider) && name.trim() !== "" && baseUrl.trim() !== "";

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
      onSaved();
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
        <DialogHeader>
          <DialogTitle>编辑供应商</DialogTitle>
          <DialogDescription>
            API Key 留空表示保持不变;填写则更新到系统钥匙串。
          </DialogDescription>
        </DialogHeader>
        <form onSubmit={handleSubmit} className="grid gap-4 py-2">
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
              onChange={(e) => setBaseUrl(e.target.value)}
            />
          </div>
          <div className="grid gap-2">
            <Label htmlFor="edit-key">API Key(留空不变)</Label>
            <Input
              id="edit-key"
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
            {submitting ? "保存中…" : "保存"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
