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

/** 关于弹窗:品牌名点击打开 */
export function AboutDialog({
  open,
  onOpenChange,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const [version, setVersion] = useState("…");
  const [canOpenExternal, setCanOpenExternal] = useState(true);

  useEffect(() => {
    if (!open) return;
    getVersion()
      .then(setVersion)
      .catch(() => setVersion("0.1.0(浏览器演示)"));
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
            <svg width="40" height="40" viewBox="0 0 32 32" fill="none" aria-hidden="true">
              <rect width="32" height="32" rx="8" fill="#0A84FF" />
              <path
                d="M8 16h5l2.5-6 3 12 2.5-6h3"
                stroke="#fff"
                strokeWidth="2.2"
                strokeLinecap="round"
                strokeLinejoin="round"
              />
            </svg>
            <div>
              <DialogTitle>Conduit</DialogTitle>
              <DialogDescription>v{version} · MIT 开源</DialogDescription>
            </div>
          </div>
        </DialogHeader>
        <p className="text-sm text-muted-foreground leading-relaxed">
          AI CLI 的本地供应中心:切换即生效、免重启,API Key 加密不落盘。
          视觉体系致敬 CC Switch(MIT)。
        </p>
        <DialogFooter>
          <Button variant="outline" onClick={() => void openGithub()}>
            <ExternalLink className="w-4 h-4 mr-1" />
            GitHub
          </Button>
          <Button variant="ghost" onClick={() => onOpenChange(false)}>
            关闭
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
