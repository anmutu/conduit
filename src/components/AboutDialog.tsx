import { useEffect, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { openUrl } from "@tauri-apps/plugin-opener";
import { ExternalLink } from "lucide-react";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { useI18n } from "@/i18n";

/** 关于弹窗:品牌名点击打开 */
export function AboutDialog({
  open,
  onOpenChange,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const { t } = useI18n();
  const [version, setVersion] = useState("…");
  const [canOpenExternal, setCanOpenExternal] = useState(true);

  useEffect(() => {
    if (!open) return;
    getVersion()
      .then(setVersion)
      .catch(() => setVersion(t("about.demoVersion")));
  }, [open]);

  const github = "https://github.com/anmutu/conduit";
  const openGithub = async () => {
    try {
      if (canOpenExternal) {
        await openUrl(github);
      } else {
        window.open(github, "_blank");
      }
    } catch {
      setCanOpenExternal(false);
      window.open(github, "_blank");
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-[360px]">
        <DialogHeader>
          <div className="flex items-center gap-3">
            <img
              src="icons/conduit-logo.svg?v=2"
              alt="Conduit"
              width={40}
              height={40}
              className="rounded-[10px]"
            />
            <div>
              <DialogTitle>Conduit</DialogTitle>
              <DialogDescription>v{version} · {t("about.openSource")}</DialogDescription>
            </div>
          </div>
        </DialogHeader>
        <p className="text-sm text-muted-foreground leading-relaxed">
          {t("about.desc")}
        </p>
        <DialogFooter>
          <Button variant="outline" onClick={() => void openGithub()}>
            <ExternalLink className="w-4 h-4 mr-1" />
            GitHub
          </Button>
          <Button variant="ghost" onClick={() => onOpenChange(false)}>
            {t("common.close")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
