import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { ArrowLeft, Boxes, Download, Plus, Settings } from "lucide-react";
import type { AppType, Provider, ProxyStatus } from "@/types";
import { AppSwitcher } from "@/components/AppSwitcher";
import { ProviderCard } from "@/components/providers/ProviderCard";
import { AddProviderDialog } from "@/components/providers/AddProviderDialog";
import { EditProviderDialog } from "@/components/providers/EditProviderDialog";
import { ConfirmDialog } from "@/components/ConfirmDialog";
import { ToastStack, type ToastItem, type ToastType } from "@/components/Toast";
import { ModeToggle } from "@/components/mode-toggle";
import { AboutDialog } from "@/components/AboutDialog";
import { TakeoverDialog } from "@/components/TakeoverDialog";
import { SettingsPage } from "@/components/settings/SettingsPage";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";

const STORAGE_KEY = "conduit-last-app";
const VALID_APPS: AppType[] = ["claude", "codex", "gemini", "opencode", "openclaw"];
const HEADER_HEIGHT = 64; // px,与 CC Switch 一致
/** 窗口窄于该宽度时,AppSwitcher 收起文字只留图标 */
const COMPACT_BREAKPOINT = 860;

function getInitialApp(): AppType {
  const saved = localStorage.getItem(STORAGE_KEY) as AppType | null;
  if (saved && VALID_APPS.includes(saved)) return saved;
  return "claude";
}

/** 把底层错误串转成人话 */
function humanizeError(raw: string): string {
  const msg = raw.replace(/^Error:\s*/i, "");
  if (msg.includes("keychain")) return "系统钥匙串访问失败,请检查授权";
  if (msg.includes("数据库") || msg.includes("database"))
    return "本地数据库读写失败";
  if (msg.includes("invoke") || msg.includes("ipc"))
    return "无法连接本地服务(浏览器预览模式下属正常)";
  return msg.length > 120 ? `${msg.slice(0, 120)}…` : msg;
}

/** 骨架卡片:列表加载时占位,避免布局抖动 */
function SkeletonCard() {
  return (
    <div className="rounded-xl border border-border p-4 bg-card">
      <div className="flex items-center gap-2">
        <div className="h-8 w-8 rounded-lg bg-muted animate-pulse" />
        <div className="space-y-1.5 flex-1">
          <div className="h-4 w-32 rounded bg-muted animate-pulse" />
          <div className="h-3 w-56 rounded bg-muted animate-pulse" />
        </div>
      </div>
    </div>
  );
}

let toastSeq = 0;

/** 浏览器演示模式(?demo=1):mock 数据渲染 UI,便于截图与样式开发 */
const IS_DEMO = new URLSearchParams(location.search).has("demo");
const DEMO_PROVIDERS: Provider[] = [
  {
    id: "demo-1",
    app_type: "claude",
    name: "CoderPlan",
    base_url: "https://api.coderplan.ai",
    keychain_id: "demo-1",
    models: [],
    is_current: true,
    is_healthy: true,
    sort_index: 0,
    created_at: 0,
    has_key: true,
  },
  {
    id: "demo-2",
    app_type: "claude",
    name: "PackyCode",
    base_url: "https://api.packyapi.com",
    keychain_id: "demo-2",
    models: [],
    is_current: false,
    is_healthy: true,
    sort_index: 1,
    created_at: 0,
    has_key: true,
  },
  {
    id: "demo-3",
    app_type: "claude",
    name: "官方登录",
    base_url: "https://api.anthropic.com",
    keychain_id: null,
    models: [],
    is_current: false,
    is_healthy: true,
    sort_index: 2,
    created_at: 0,
    has_key: false,
  },
];

function App() {
  const [activeApp, setActiveApp] = useState<AppType>(getInitialApp);
  const [providers, setProviders] = useState<Provider[]>([]);
  const [hasCache, setHasCache] = useState(false); // 当前 app 是否已有可展示数据
  const [isAddOpen, setIsAddOpen] = useState(false);
  const [editingProvider, setEditingProvider] = useState<Provider | null>(null);
  const [confirmDelete, setConfirmDelete] = useState<Provider | null>(null);
  const [aboutOpen, setAboutOpen] = useState(false);
  const [currentView, setCurrentView] = useState<"providers" | "settings">("providers");
  const [takeoverOpen, setTakeoverOpen] = useState(false);
  const [toasts, setToasts] = useState<ToastItem[]>([]);
  const [proxyOk, setProxyOk] = useState<boolean | null>(null);
  const [proxyAddr, setProxyAddr] = useState("");
  const [highlightId, setHighlightId] = useState<string | null>(null);
  const [winWidth, setWinWidth] = useState(() => window.innerWidth);
  // macOS 融合标题栏(Overlay):红绿灯占据左上角,header 内容需左侧避让
  const [isMac, setIsMac] = useState(false);
  useEffect(() => {
    const p = (window as any).__TAURI_INTERNALS__?.platform;
    setIsMac(p ? p === "darwin" : /Mac/i.test(navigator.userAgent));
  }, []);

  // 按 app 缓存列表数据:切换时先显示缓存,后台刷新(stale-while-revalidate)
  const cacheRef = useRef<Partial<Record<AppType, Provider[]>>>({});
  // 跟踪当前 app:异步刷新返回时防竞态
  const activeAppRef = useRef(activeApp);
  useEffect(() => {
    activeAppRef.current = activeApp;
  }, [activeApp]);

  const toast = useCallback((type: ToastType, msg: string) => {
    const id = ++toastSeq;
    setToasts((ts) => [...ts.slice(-2), { id, type, msg }]);
    const ttl = type === "success" ? 1800 : 3500;
    setTimeout(() => setToasts((ts) => ts.filter((t) => t.id !== id)), ttl);
  }, []);

  const dismissToast = useCallback((id: number) => {
    setToasts((ts) => ts.filter((t) => t.id !== id));
  }, []);

  const refresh = useCallback(async (app: AppType) => {
    // 演示模式:不走 IPC,直接展示 mock 数据
    if (IS_DEMO) {
      setProviders(DEMO_PROVIDERS);
      setHasCache(true);
      return;
    }
    const cached = cacheRef.current[app];
    if (cached) {
      // 已有缓存:先展示旧数据,静默刷新
      setProviders(cached);
      setHasCache(true);
    } else {
      setHasCache(false);
    }
    try {
      const list = await invoke<Provider[]>("list_providers", { appType: app });
      cacheRef.current[app] = list;
      // 异步返回时若已切走其他 app,不覆盖当前展示
      if (activeAppRef.current === app) {
        setProviders(list);
        setHasCache(true);
      }
    } catch (e) {
      toast("error", humanizeError(String(e)));
    }
  }, [toast]);

  useEffect(() => {
    void refresh(activeApp);
  }, [activeApp, refresh]);

  // 代理状态:常显于 header(卖点可见性)
  useEffect(() => {
    if (IS_DEMO) {
      setProxyOk(true);
      setProxyAddr("127.0.0.1:9527");
      return;
    }
    invoke<ProxyStatus>("proxy_status")
      .then((s) => {
        setProxyOk(s.running);
        setProxyAddr(s.addr);
      })
      .catch(() => setProxyOk(false));
  }, []);

  // 窗口宽度:驱动 AppSwitcher compact 收缩
  useEffect(() => {
    const onResize = () => setWinWidth(window.innerWidth);
    window.addEventListener("resize", onResize);
    return () => window.removeEventListener("resize", onResize);
  }, []);

  // 快捷键:Cmd/Ctrl+N 新建,Cmd/Ctrl+1..5 切换应用
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      // Escape:从设置页返回主界面
      if (e.key === "Escape") {
        setCurrentView("providers");
        return;
      }
      const mod = e.metaKey || e.ctrlKey;
      if (!mod) return;
      if (e.key.toLowerCase() === "n") {
        e.preventDefault();
        setIsAddOpen(true);
        return;
      }
      const idx = Number(e.key) - 1;
      if (Number.isInteger(idx) && idx >= 0 && idx < VALID_APPS.length) {
        e.preventDefault();
        setActiveApp(VALID_APPS[idx]);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  // 托盘快速切换后:同步刷新 + toast(与主界面操作同一反馈通道)
  useEffect(() => {
    if (IS_DEMO) return;
    let un: (() => void) | undefined;
    void listen<{ appType: AppType; providerId: string; name: string }>(
      "provider-switched",
      (e) => {
        toast("success", `已切换到 ${e.payload.name}(托盘)`);
        cacheRef.current = {};
        void refresh(e.payload.appType);
      },
    ).then((fn) => (un = fn));
    return () => un?.();
  }, [refresh, toast]);

  /** 首启导入:扫描现有 CLI 配置建供应商 */
  const [importing, setImporting] = useState(false);
  const runImport = async () => {
    setImporting(true);
    try {
      const list = await invoke<{ app: string; name: string; has_key: boolean }[]>(
        "import_existing",
      );
      if (list.length === 0) {
        toast("error", "未发现可导入的配置(无第三方 base_url 或已导入过)");
      } else {
        toast(
          "success",
          `已导入 ${list.length} 个:${list.map((x) => x.name.replace("导入的 ", "")).join("、")}`,
        );
        syncTray();
        await refresh(activeApp);
      }
    } catch (e) {
      toast("error", humanizeError(String(e)));
    } finally {
      setImporting(false);
    }
  };

  /** 供应商变更后同步托盘菜单(演示模式无后端,跳过) */
  const syncTray = useCallback(() => {
    if (!IS_DEMO) void invoke("refresh_tray").catch(() => {});
  }, []);

  /** 新建/更新后:滚动到该卡片并短暂高亮 */
  const focusProvider = useCallback((id: string) => {
    setHighlightId(id);
    setTimeout(() => {
      document
        .getElementById(`provider-${id}`)
        ?.scrollIntoView({ behavior: "smooth", block: "nearest" });
    }, 80);
    setTimeout(() => setHighlightId(null), 2200);
  }, []);

  const switchProvider = async (provider: Provider) => {
    try {
      await invoke("switch_provider", {
        id: provider.id,
        appType: provider.app_type,
      });
      toast("success", `已切换到 ${provider.name}`);
      syncTray();
      await refresh(activeApp);
    } catch (e) {
      toast("error", humanizeError(String(e)));
    }
  };

  const duplicateProvider = async (provider: Provider) => {
    try {
      const created = await invoke<Provider>("create_provider", {
        input: {
          app_type: provider.app_type,
          name: `${provider.name} (副本)`,
          base_url: provider.base_url,
          models: provider.models,
        },
      });
      toast("success", `已复制为「${created.name}」`);
      syncTray();
      await refresh(activeApp);
      focusProvider(created.id);
    } catch (e) {
      toast("error", humanizeError(String(e)));
    }
  };

  const deleteProvider = async () => {
    if (!confirmDelete) return;
    const target = confirmDelete;
    try {
      await invoke("delete_provider", { id: target.id });
      setConfirmDelete(null);
      toast("success", `已删除 ${target.name}`);
      syncTray();
      await refresh(activeApp);
    } catch (e) {
      toast("error", humanizeError(String(e)));
    }
  };

  return (
    <div className="flex flex-col h-screen overflow-hidden bg-background text-foreground selection:bg-primary/30">
      {/* 顶部栏:品牌 + 代理状态 + 应用切换胶囊 + 添加按钮 */}
      <header
        className="shrink-0 z-50 w-full transition-all duration-300 bg-background/80 backdrop-blur-md border-b border-border"
        style={{ height: HEADER_HEIGHT }}
        data-tauri-drag-region
      >
        <div
          className={cn(
            "flex h-full items-center justify-between gap-2 px-6",
            isMac && "pl-[84px]",
          )}
        >
          <div className="flex items-center gap-2" data-tauri-no-drag>
            {currentView === "settings" && (
              <>
                <Button
                  variant="outline"
                  size="icon"
                  className="mr-1 rounded-lg"
                  title="返回(Esc)"
                  onClick={() => setCurrentView("providers")}
                >
                  <ArrowLeft className="w-4 h-4" />
                </Button>
                <h1 className="text-lg font-semibold">设置</h1>
              </>
            )}
            {currentView === "providers" && (
            <button
              type="button"
              onClick={() => setAboutOpen(true)}
              title="关于 Conduit"
              className="text-xl font-semibold transition-colors text-blue-500 hover:text-blue-600 dark:text-blue-400 dark:hover:text-blue-300 select-none cursor-pointer"
            >
              Conduit
            </button>
            )}
            {/* 代理状态常显:产品核心卖点的可见性 */}
            {proxyOk !== null && (
              <button
                type="button"
                onClick={() => setTakeoverOpen(true)}
                className={cn(
                  "inline-flex items-center gap-1.5 px-2 py-0.5 rounded-md text-xs font-medium select-none cursor-pointer transition-opacity hover:opacity-80",
                  proxyOk
                    ? "text-emerald-600 dark:text-emerald-400 bg-emerald-500/10"
                    : "text-red-600 dark:text-red-400 bg-red-500/10",
                )}
                title={proxyOk ? `本地代理 ${proxyAddr},点击管理接管` : "代理未运行,点击查看"}
              >
                <span
                  className={cn(
                    "h-1.5 w-1.5 rounded-full",
                    proxyOk ? "bg-emerald-500" : "bg-red-500",
                  )}
                />
                {proxyOk ? "代理运行中" : "代理离线"}
              </button>
            )}
            <ModeToggle />
            <Button
              variant="ghost"
              size="icon"
              title="设置"
              className="hover:bg-black/5 dark:hover:bg-white/5"
              onClick={() => setCurrentView("settings")}
            >
              <Settings className="w-4 h-4" />
            </Button>
          </div>

          <div className="flex flex-1 min-w-0 items-center justify-end gap-1.5">
            {currentView === "providers" && (
              <>
                <AppSwitcher
                  activeApp={activeApp}
                  onSwitch={setActiveApp}
                  compact={winWidth < COMPACT_BREAKPOINT}
                />
                <Button
                  onClick={() => setIsAddOpen(true)}
                  size="icon"
                  className="ml-2"
                  title="添加供应商 (⌘N)"
                >
                  <Plus className="w-5 h-5" />
                </Button>
              </>
            )}
          </div>
        </div>
      </header>

      {/* 主内容区:供应商卡片列表 */}
      <main className="flex-1 min-h-0 flex flex-col overflow-y-auto">
        {currentView === "settings" && (
          <div className="px-6 py-6 animate-fade-in">
            <SettingsPage
              onError={(m) => toast("error", humanizeError(m))}
              onSuccess={(m) => toast("success", m)}
            />
          </div>
        )}
        {currentView === "providers" && (
        <div className="px-6 flex flex-col flex-1 min-h-0 overflow-hidden">
          <div className="flex-1 overflow-y-auto overflow-x-hidden pb-12 px-1">
            <div className="space-y-4 animate-fade-in" key={activeApp}>
              {/* 首次加载:骨架屏占位 */}
              {!hasCache && providers.length === 0 && (
                <>
                  <SkeletonCard />
                  <SkeletonCard />
                  <SkeletonCard />
                </>
              )}

              {/* 空状态:CTA 引导 */}
              {hasCache && providers.length === 0 && (
                <div className="flex flex-col items-center justify-center py-20 gap-3 text-center">
                  <div className="h-12 w-12 rounded-xl bg-muted flex items-center justify-center border border-border">
                    <Boxes className="h-6 w-6 text-muted-foreground" />
                  </div>
                  <p className="text-base font-medium">还没有{activeApp} 供应商</p>
                  <p className="text-sm text-muted-foreground -mt-2">
                    添加一个开始使用;切换即生效,无需重启终端
                  </p>
                  <div className="flex items-center gap-2 mt-2">
                    <Button onClick={() => setIsAddOpen(true)}>
                      <Plus className="w-4 h-4 mr-1" />
                      添加供应商
                    </Button>
                    {!IS_DEMO && (
                      <Button
                        variant="outline"
                        disabled={importing}
                        onClick={() => void runImport()}
                      >
                        <Download className="w-4 h-4 mr-1" />
                        {importing ? "导入中…" : "从现有 CLI 配置导入"}
                      </Button>
                    )}
                  </div>
                  <p className="text-xs text-muted-foreground">
                    或按{" "}
                    <kbd className="px-1.5 py-0.5 rounded border border-border bg-muted font-mono text-[10px]">
                      ⌘N
                    </kbd>
                  </p>
                </div>
              )}

              {providers.map((p) => (
                <ProviderCard
                  key={p.id}
                  provider={p}
                  isCurrent={p.is_current}
                  highlight={p.id === highlightId}
                  onSwitch={(provider) => void switchProvider(provider)}
                  onEdit={(provider) => setEditingProvider(provider)}
                  onDuplicate={(provider) => void duplicateProvider(provider)}
                  onDelete={(provider) => setConfirmDelete(provider)}
                  onCopyUrl={(url) => {
                    navigator.clipboard
                      .writeText(url)
                      .then(() => toast("success", "接口地址已复制"))
                      .catch(() => toast("error", "复制失败"));
                  }}
                />
              ))}
            </div>
          </div>
        </div>
        )}
      </main>

      <ToastStack items={toasts} onDismiss={dismissToast} />

      <AboutDialog open={aboutOpen} onOpenChange={setAboutOpen} />

      <TakeoverDialog
        open={takeoverOpen}
        onOpenChange={setTakeoverOpen}
        onError={(m) => toast("error", humanizeError(m))}
        onSuccess={(m) => toast("success", m)}
      />

      <AddProviderDialog
        open={isAddOpen}
        onOpenChange={setIsAddOpen}
        appId={activeApp}
        onCreated={async (created) => {
          toast("success", `已添加 ${created.name}`);
          syncTray();
          await refresh(activeApp);
          focusProvider(created.id);
        }}
        onError={(msg) => toast("error", humanizeError(msg))}
      />

      <EditProviderDialog
        provider={editingProvider}
        onOpenChange={(open: boolean) => {
          if (!open) setEditingProvider(null);
        }}
        onSaved={async (saved) => {
          toast("success", `已保存 ${saved.name}`);
          syncTray();
          await refresh(activeApp);
          focusProvider(saved.id);
        }}
        onError={(msg) => toast("error", humanizeError(msg))}
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
