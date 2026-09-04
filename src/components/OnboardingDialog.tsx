import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Check, Download, Plus, ShieldCheck } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { ProviderIcon } from "@/components/ProviderIcon";
import { useI18n } from "@/i18n";
import { cn } from "@/lib/utils";
import type { AppType } from "@/types";

export const ONBOARD_KEY = "conduit-onboarded";

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
  opencode: "OpenCode",
  openclaw: "OpenClaw",
};

/**
 * 首启向导(3 步):选常用 CLI → 添加/迁移第一个供应商 → 一键接管。
 * 完成或跳过都会写 localStorage 标记,不再打扰。
 */
export function OnboardingDialog({
  open,
  onOpenChange,
  apps,
  onAppsChange,
  onAdd,
  onImport,
  providersCount,
  onDone,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  apps: AppType[];
  onAppsChange: (apps: AppType[]) => void;
  onAdd: () => void;
  onImport: () => void;
  providersCount: number;
  onDone: () => void;
}) {
  const { t } = useI18n();
  const [step, setStep] = useState(0);
  const [selected, setSelected] = useState<AppType[]>(apps);
  const [importing, setImporting] = useState(false);
  const [takeoverList, setTakeoverList] = useState<TakeoverStatus[]>([]);
  const [busyApp, setBusyApp] = useState<string | null>(null);

  useEffect(() => {
    if (open) {
      setStep(0);
      setSelected(apps);
      // eslint-disable-next-line react-hooks/exhaustive-deps
    }
  }, [open]);

  // 进入第 3 步时拉取接管状态
  useEffect(() => {
    if (open && step === 2) {
      invoke<TakeoverStatus[]>("takeover_status")
        .then((l) => setTakeoverList(l.filter((x) => x.supported)))
        .catch(() => setTakeoverList([]));
    }
  }, [open, step]);

  const finish = () => {
    localStorage.setItem(ONBOARD_KEY, "1");
    onOpenChange(false);
    onDone();
  };

  const toggleApp = (app: AppType) => {
    setSelected((s) =>
      s.includes(app) ? s.filter((a) => a !== app) : [...s, app],
    );
  };

  const applyTakeover = async (app: string) => {
    setBusyApp(app);
    try {
      await invoke("apply_takeover", { appType: app });
      const l = await invoke<TakeoverStatus[]>("takeover_status");
      setTakeoverList(l.filter((x) => x.supported));
    } finally {
      setBusyApp(null);
    }
  };

  const titles = [
    t("ob.step1Title"),
    t("ob.step2Title"),
    t("ob.step3Title"),
  ];

  return (
    <Dialog open={open} onOpenChange={(o) => { if (!o) finish(); }}>
      <DialogContent className="sm:max-w-[460px]">
        <DialogHeader>
          <DialogTitle>
            {t("ob.title")} · {step + 1}/3
          </DialogTitle>
          <DialogDescription>{titles[step]}</DialogDescription>
        </DialogHeader>

        {/* 步骤指示 */}
        <div className="flex gap-1.5">
          {[0, 1, 2].map((i) => (
            <div
              key={i}
              className={cn(
                "h-1 flex-1 rounded-full transition-colors",
                i <= step ? "bg-primary" : "bg-muted",
              )}
            />
          ))}
        </div>

        {step === 0 && (
          <div className="grid gap-4">
            <div className="grid grid-cols-3 gap-2">
              {apps.map((app) => (
                <button
                  key={app}
                  type="button"
                  onClick={() => toggleApp(app)}
                  className={cn(
                    "flex flex-col items-center gap-1.5 rounded-lg border p-3 text-xs transition-colors",
                    selected.includes(app)
                      ? "border-primary bg-accent"
                      : "border-border hover:bg-accent",
                  )}
                >
                  <ProviderIcon icon={app} size={22} />
                  <span className="font-medium">{t(`app.${app}`)}</span>
                  {selected.includes(app) && (
                    <Check className="w-3.5 h-3.5 text-primary" />
                  )}
                </button>
              ))}
            </div>
            <Button
              disabled={selected.length === 0}
              onClick={() => {
                onAppsChange(selected);
                setStep(1);
              }}
            >
              {t("common.next")}
            </Button>
          </div>
        )}

        {step === 1 && (
          <div className="grid gap-3">
            <Button
              onClick={() => {
                onAdd();
                onOpenChange(false);
                // 添加完成与否都算过关(用户可稍后再加),不写完成标记,交给 onDone 流程
                localStorage.setItem(ONBOARD_KEY, "1");
              }}
            >
              <Plus className="w-4 h-4 mr-1" />
              {t("ob.addFirst")}
            </Button>
            <Button
              variant="outline"
              disabled={importing}
              onClick={async () => {
                setImporting(true);
                onImport();
                setImporting(false);
              }}
            >
              <Download className="w-4 h-4 mr-1" />
              {t("empty.import")}
            </Button>
            <Button variant="ghost" onClick={() => setStep(2)}>
              {providersCount > 0 ? t("common.next") : t("ob.skip")}
            </Button>
          </div>
        )}

        {step === 2 && (
          <div className="grid gap-3">
            {takeoverList.map((s) => (
              <div
                key={s.app}
                className="flex items-center justify-between gap-3 rounded-xl border border-border p-3"
              >
                <div className="flex items-center gap-2.5">
                  <ProviderIcon icon={s.app} size={18} />
                  <span className="text-sm font-medium">
                    {APP_LABEL[s.app] ?? s.app}
                  </span>
                </div>
                {s.active ? (
                  <span className="flex items-center gap-1 text-xs text-emerald-600 dark:text-emerald-400">
                    <ShieldCheck className="w-3.5 h-3.5" />
                    {t("takeover.active")}
                  </span>
                ) : (
                  <Button
                    size="sm"
                    variant="outline"
                    disabled={busyApp === s.app}
                    onClick={() => void applyTakeover(s.app)}
                  >
                    {busyApp === s.app ? "…" : t("takeover.apply")}
                  </Button>
                )}
              </div>
            ))}
            <Button onClick={finish}>{t("ob.finish")}</Button>
          </div>
        )}
      </DialogContent>
    </Dialog>
  );
}
