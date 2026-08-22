import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Plus, Settings } from "lucide-react";
import type { AppType, Provider, ProxyStatus } from "@/types";
import { AppSwitcher } from "@/components/AppSwitcher";
import { ProviderCard } from "@/components/providers/ProviderCard";
import { AddProviderDialog } from "@/components/providers/AddProviderDialog";
import { EditProviderDialog } from "@/components/providers/EditProviderDialog";
import { ConfirmDialog } from "@/components/ConfirmDialog";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";

const STORAGE_KEY = "conduit-last-app";
const VALID_APPS: AppType[] = ["claude", "codex", "gemini", "opencode", "openclaw"];
const HEADER_HEIGHT = 64; // px,与 CC Switch 一致

function getInitialApp(): AppType {
  const saved = localStorage.getItem(STORAGE_KEY) as AppType | null;
  if (saved && VALID_APPS.includes(saved)) return saved;
  return "claude";
}

function App() {
  const [activeApp, setActiveApp] = useState<AppType>(getInitialApp);
  const [providers, setProviders] = useState<Provider[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [isAddOpen, setIsAddOpen] = useState(false);
  const [editingProvider, setEditingProvider] = useState<Provider | null>(null);
  const [confirmDelete, setConfirmDelete] = useState<Provider | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [proxy, setProxy] = useState<ProxyStatus | null>(null);

  const refresh = useCallback(async (app: AppType) => {
    setIsLoading(true);
    try {
      const list = await invoke<Provider[]>("list_providers", { appType: app });
      setProviders(list);
      setErr(null);
    } catch (e) {
      setErr(String(e));
    } finally {
      setIsLoading(false);
    }
  }, []);

  useEffect(() => {
    void refresh(activeApp);
  }, [activeApp, refresh]);

  useEffect(() => {
    invoke<ProxyStatus>("proxy_status")
      .then(setProxy)
      .catch(() => setProxy(null));
  }, []);

  const switchProvider = async (provider: Provider) => {
    try {
      await invoke("switch_provider", {
        id: provider.id,
        appType: provider.app_type,
      });
      await refresh(activeApp);
    } catch (e) {
      setErr(String(e));
    }
  };

  const duplicateProvider = async (provider: Provider) => {
    try {
      await invoke("create_provider", {
        input: {
          app_type: provider.app_type,
          name: `${provider.name} (副本)`,
          base_url: provider.base_url,
          models: provider.models,
        },
      });
      await refresh(activeApp);
    } catch (e) {
      setErr(String(e));
    }
  };

  const deleteProvider = async () => {
    if (!confirmDelete) return;
    try {
      await invoke("delete_provider", { id: confirmDelete.id });
      setConfirmDelete(null);
      await refresh(activeApp);
    } catch (e) {
      setErr(String(e));
    }
  };

  return (
    <div className="flex flex-col h-screen overflow-hidden bg-background text-foreground selection:bg-primary/30">
      {/* 顶部栏:品牌 + 应用切换胶囊 + 添加按钮(结构与 CC Switch 一致) */}
      <header
        className="shrink-0 z-50 w-full transition-all duration-300 bg-background/80 backdrop-blur-md border-b border-border"
        style={{ height: HEADER_HEIGHT }}
        data-tauri-drag-region
      >
        <div className="flex h-full items-center justify-between gap-2 px-6">
          <div className="flex items-center gap-2" data-tauri-no-drag>
            <span
              className="text-xl font-semibold transition-colors text-blue-500 hover:text-blue-600 dark:text-blue-400 dark:hover:text-blue-300 select-none"
              title={proxy?.running ? `本地代理 ${proxy.addr}` : "Conduit"}
            >
              Conduit
            </span>
            <Button
              variant="ghost"
              size="icon"
              title="设置"
              className="hover:bg-black/5 dark:hover:bg-white/5"
              onClick={() => setErr("设置页即将在 M1 上线")}
            >
              <Settings className="w-4 h-4" />
            </Button>
          </div>

          <div className="flex flex-1 min-w-0 items-center justify-end gap-1.5">
            <AppSwitcher activeApp={activeApp} onSwitch={setActiveApp} />
            <Button
              onClick={() => setIsAddOpen(true)}
              size="icon"
              className="ml-2"
              title="添加供应商"
            >
              <Plus className="w-5 h-5" />
            </Button>
          </div>
        </div>
      </header>

      {/* 主内容区:供应商卡片列表 */}
      <main className="flex-1 min-h-0 flex flex-col overflow-y-auto animate-fade-in">
        <div className="px-6 flex flex-col flex-1 min-h-0 overflow-hidden">
          <div className="flex-1 overflow-y-auto overflow-x-hidden pb-12 px-1">
            <div className="space-y-4" key={activeApp}>
              {isLoading && providers.length === 0 && (
                <div className="text-center text-muted-foreground py-16">
                  加载中…
                </div>
              )}
              {!isLoading && providers.length === 0 && (
                <div className="text-center py-16">
                  <p className="text-muted-foreground">
                    暂无供应商,点击右上角 + 添加
                  </p>
                </div>
              )}
              {providers.map((p) => (
                <ProviderCard
                  key={p.id}
                  provider={p}
                  isCurrent={p.is_current}
                  onSwitch={(provider) => void switchProvider(provider)}
                  onEdit={(provider) => setEditingProvider(provider)}
                  onDuplicate={(provider) => void duplicateProvider(provider)}
                  onDelete={(provider) => setConfirmDelete(provider)}
                />
              ))}
            </div>
          </div>
        </div>
      </main>

      {/* 底部错误提示条(轻量,不占布局) */}
      {err && (
        <div
          className={cn(
            "fixed bottom-4 left-1/2 -translate-x-1/2 z-[60]",
            "max-w-[80%] truncate px-4 py-2 rounded-lg text-sm shadow-lg",
            "bg-red-500/90 text-white",
          )}
          onClick={() => setErr(null)}
        >
          {err}(点击关闭)
        </div>
      )}

      <AddProviderDialog
        open={isAddOpen}
        onOpenChange={setIsAddOpen}
        appId={activeApp}
        onCreated={() => void refresh(activeApp)}
        onError={setErr}
      />

      <EditProviderDialog
        provider={editingProvider}
        onOpenChange={(open) => {
          if (!open) setEditingProvider(null);
        }}
        onSaved={() => void refresh(activeApp)}
        onError={setErr}
      />

      <ConfirmDialog
        isOpen={Boolean(confirmDelete)}
        title="删除供应商"
        message={`确定要删除「${confirmDelete?.name ?? ""}」吗?系统钥匙串中的 API Key 也会一并清除。`}
        onConfirm={() => void deleteProvider()}
        onCancel={() => setConfirmDelete(null)}
      />
    </div>
  );
}

export default App;
