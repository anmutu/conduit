import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ShieldCheck, ShieldOff, TriangleAlert } from "lucide-react";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { ProviderIcon } from "@/components/ProviderIcon";
import { useI18n } from "@/i18n";
import { cn } from "@/lib/utils";

interface TakeoverStatus {
  app: string;
  supported: boolean;
  config_exists: boolean;
  active: boolean;
  effective: boolean;
}

const APP_LABEL: Record<string, string> = {
  claude: "Claude Code",
  codex: "Codex",
  gemini: "Gemini CLI",
};

/** 接管管理:把各 CLI 的 live 配置指向本地代理(可随时还原) */
export function TakeoverDialog({
  open,
  onOpenChange,
  onError,
  onSuccess,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onError: (msg: string) => void;
  onSuccess: (msg: string) => void;
}) {
  const [list, setList] = useState<TakeoverStatus[]>([]);
  const [busy, setBusy] = useState<string | null>(null);
  const { t } = useI18n();

  const refresh = () => {
    invoke<TakeoverStatus[]>("takeover_status")
      .then((l) => setList(l.filter((x) => x.supported)))
      .catch((e) => onError(String(e)));
  };

  useEffect(() => {
    if (open) refresh();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open]);

  const act = async (app: string, restore: boolean) => {
    setBusy(app);
    try {
      const appEnum = app as "claude" | "codex" | "gemini";
      if (restore) {
        await invoke("restore_takeover", { appType: appEnum });
        onSuccess(t("takeover.restored", { name: APP_LABEL[app] }));
      } else {
        await invoke("apply_takeover", { appType: appEnum });
        onSuccess(t("takeover.applied", { name: APP_LABEL[app] }));
      }
      refresh();
    } catch (e) {
      onError(String(e));
    } finally {
      setBusy(null);
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-[460px]">
        <DialogHeader>
          <DialogTitle>{t("takeover.title")}</DialogTitle>
          <DialogDescription>
            {t("takeover.desc")}
          </DialogDescription>
        </DialogHeader>

        <div className="grid gap-3">
          {list.map((s) => (
            <div
              key={s.app}
              className="flex items-center justify-between gap-3 rounded-xl border border-border p-3 bg-card"
            >
              <div className="flex items-center gap-2.5 min-w-0">
                <ProviderIcon icon={s.app} size={18} />
                <div className="min-w-0">
                  <div className="text-sm font-medium">
                    {APP_LABEL[s.app] ?? s.app}
                  </div>
                  <div
                    className={cn(
                      "text-xs flex items-center gap-1",
                      s.active && s.effective
                        ? "text-emerald-600 dark:text-emerald-400"
                        : s.active && !s.effective
                          ? "text-amber-600 dark:text-amber-400"
                          : "text-muted-foreground",
                    )}
                  >
                    {s.active && s.effective && (
                      <>
                        <ShieldCheck className="w-3 h-3" /> {t("takeover.active")}
                      </>
                    )}
                    {s.active && !s.effective && (
                      <>
                        <TriangleAlert className="w-3 h-3" />
                        {t("takeover.overridden")}
                      </>
                    )}
                    {!s.active && (
                      <>
                        <ShieldOff className="w-3 h-3" />
                        {s.config_exists ? t("takeover.inactive") : t("takeover.noConfig")}
                      </>
                    )}
                  </div>
                </div>
              </div>
              {s.active ? (
                <Button
                  size="sm"
                  variant="outline"
                  disabled={busy === s.app}
                  onClick={() => void act(s.app, true)}
                >
                  {busy === s.app ? "…" : t("takeover.restore")}
                </Button>
              ) : (
                <Button
                  size="sm"
                  disabled={busy === s.app}
                  onClick={() => void act(s.app, false)}
                >
                  {busy === s.app ? "…" : t("takeover.apply")}
                </Button>
              )}
            </div>
          ))}
          {list.length === 0 && (
            <p className="text-sm text-muted-foreground text-center py-4">
              {t("common.loading")}
            </p>
          )}
        </div>
      </DialogContent>
    </Dialog>
  );
}
