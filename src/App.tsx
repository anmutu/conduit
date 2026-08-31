import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Activity, Boxes, Download, Gauge, Plus } from "lucide-react";
import type { AppType, Provider, ProxyStatus, UsageSummary } from "@/types";
import { APP_PROTOCOL } from "@/types";
import { Sidebar } from "@/components/Sidebar";
import { AppHeaderBar } from "@/components/AppHeaderBar";
import { ProviderCard } from "@/components/providers/ProviderCard";
import { AddProviderDialog } from "@/components/providers/AddProviderDialog";
import { GatewayStatusStrip } from "@/components/providers/GatewayStatusStrip";
import { EditProviderDialog } from "@/components/providers/EditProviderDialog";
import { ConfirmDialog } from "@/components/ConfirmDialog";
import { ToastStack, type ToastItem, type ToastType } from "@/components/Toast";
import { AboutDialog } from "@/components/AboutDialog";
import { QuickSwitchPanel } from "@/components/QuickSwitchPanel";
import { OnboardingDialog, ONBOARD_KEY } from "@/components/OnboardingDialog";
import { TakeoverDialog } from "@/components/TakeoverDialog";
import { SettingsPage } from "@/components/settings/SettingsPage";
import { UsagePage } from "@/components/usage/UsagePage";
import { LogsPage } from "@/components/usage/LogsPage";
import { McpPage } from "@/components/settings/McpPage";
import { SkillsPage } from "@/components/settings/SkillsPage";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import { useI18n } from "@/i18n";
import {
  loadLayout,
  saveLayout,
  loadApps,
  saveApps,
  ALL_APPS,
  type LayoutMode,
} from "@/lib/appPrefs";

const STORAGE_KEY = "conduit-last-app";
const VALID_APPS: AppType[] = ALL_APPS;

function getInitialApp(): AppType {
  const saved = localStorage.getItem(STORAGE_KEY) as AppType | null;
  if (saved && VALID_APPS.includes(saved)) return saved;
  return "claude";
}

/** 把底层错误串转成人话 */
type TFunc = ReturnType<typeof useI18n>["t"];
function humanizeError(raw: string, t: TFunc): string {
  const msg = raw.replace(/^Error:\s*/i, "");
  if (msg.includes("keychain")) return t("err.keychain");
  if (msg.includes("数据库") || msg.includes("database")) return t("err.db");
  if (msg.includes("invoke") || msg.includes("ipc")) return t("err.ipc");
  return t("err.fallback", { msg: msg.length > 120 ? `${msg.slice(0, 120)}…` : msg });
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
    endpoints: { anthropic: "https://api.coderplan.ai", openai: "https://api.coderplan.ai/v1" },
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
    name: "OpenAI",
    base_url: "https://api.openai.com",
    endpoints: { openai: "https://api.openai.com" },
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
    endpoints: { anthropic: "https://api.anthropic.com" },
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
  const { t, locale } = useI18n();
  const [activeApp, setActiveApp] = useState<AppType>(getInitialApp);
  const [providers, setProviders] = useState<Provider[]>([]);
  const [usageMap, setUsageMap] = useState<Record<string, UsageSummary>>({});
  const [hasCache, setHasCache] = useState(false); // 当前 app 是否已有可展示数据
  const [isAddOpen, setIsAddOpen] = useState(false);
  const [editingProvider, setEditingProvider] = useState<Provider | null>(null);
  const [confirmDelete, setConfirmDelete] = useState<Provider | null>(null);
  const [aboutOpen, setAboutOpen] = useState(false);
  const [quickOpen, setQuickOpen] = useState(false);
  // 拖拽排序:被拖卡片 id 与悬停目标 id
  const [dragId, setDragId] = useState<string | null>(null);
  const [dragOverId, setDragOverId] = useState<string | null>(null);
  // 全部测速:批量结果 + 进行中标记
  const [batchResults, setBatchResults] = useState<Record<string, { ok: boolean; latency_ms: number; message: string }>>({});
  const [testingAll, setTestingAll] = useState(false);
  // 候选链展示(Clash 式)+ 按延迟排序开关
  const [chain, setChain] = useState<string[]>([]);
  const [latencySort, setLatencySort] = useState(false);

  const runTestAll = async () => {
    setTestingAll(true);
    try {
      const items = await invoke<{ provider_id: string; ok: boolean; latency_ms: number; message: string }[]>(
        "test_all_providers",
        { appType: activeApp },
      );
      const map: Record<string, { ok: boolean; latency_ms: number; message: string }> = {};
      for (const it of items) {
        map[it.provider_id] = { ok: it.ok, latency_ms: it.latency_ms, message: it.message };
      }
      setBatchResults(map);
      const okN = items.filter((i) => i.ok).length;
      toast(okN === items.length ? "success" : "error",
        t("provider.testAllDone", { ok: okN, total: items.length }));
    } catch (e) {
      toast("error", humanizeError(String(e), t));
    } finally {
      setTestingAll(false);
    }
  };

  /** 拖拽落点:本地立即换序,并把完整顺序写回后端 */
  const dropOn = (targetId: string) => {
    const from = dragId;
    setDragId(null);
    setDragOverId(null);
    if (!from || from === targetId) return;
    setProviders((list) => {
      const next = [...list];
      const fi = next.findIndex((p) => p.id === from);
      const ti = next.findIndex((p) => p.id === targetId);
      if (fi < 0 || ti < 0) return list;
      const [moved] = next.splice(fi, 1);
      next.splice(ti, 0, moved);
      void invoke("reorder_providers", { ids: next.map((p) => p.id) }).catch(
        () => {},
      );
      return next;
    });
  };
  const [currentView, setCurrentView] = useState<"providers" | "settings" | "usage" | "logs" | "mcp" | "skills">("providers");
  // 界面偏好:布局(左侧/顶部)+ 可见分组顺序,设置页可改
  const [layout, setLayoutState] = useState<LayoutMode>(loadLayout);
  const [appsOrder, setAppsOrder] = useState<AppType[]>(loadApps);
  const setLayout = (m: LayoutMode) => {
    setLayoutState(m);
    saveLayout(m);
  };
  const updateApps = (apps: AppType[]) => {
    setAppsOrder(apps);
    saveApps(apps);
  };
  const [takeoverOpen, setTakeoverOpen] = useState(false);
  // 首启向导:零供应商且未看过向导时触发
  const [onboardingOpen, setOnboardingOpen] = useState(false);
  const [toasts, setToasts] = useState<ToastItem[]>([]);
  const [proxyOk, setProxyOk] = useState<boolean | null>(null);
  const [proxyAddr, setProxyAddr] = useState("");
  const [highlightId, setHighlightId] = useState<string | null>(null);
  // macOS 融合标题栏(Overlay):红绿灯占据左上角,侧栏顶部需向下避让
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
  useEffect(() => {
    if (IS_DEMO) return;
    invoke<string[]>("get_failover_chain", { appType: activeApp })
      .then(setChain)
      .catch(() => setChain([]));
  }, [activeApp, providers]);

  const toast = useCallback(
    (type: ToastType, msg: string, action?: ToastItem["action"]) => {
      const id = ++toastSeq;
      setToasts((ts) => [...ts.slice(-2), { id, type, msg, action }]);
      // 带动作(如"撤销")时停留 5s,给用户反应时间
      const ttl = action ? 5000 : type === "success" ? 1800 : 3500;
      setTimeout(() => setToasts((ts) => ts.filter((t) => t.id !== id)), ttl);
    },
    [],
  );

  const dismissToast = useCallback((id: number) => {
    setToasts((ts) => ts.filter((t) => t.id !== id));
  }, []);

  const refresh = useCallback(async (app: AppType) => {
    // 演示模式:不走 IPC,直接展示 mock 数据
    if (IS_DEMO) {
      setProviders(DEMO_PROVIDERS);
      setHasCache(true);
      setUsageMap({
        "demo-1": { requests: 128, input_tokens: 412_000, output_tokens: 1_260_000 },
        "demo-2": { requests: 36, input_tokens: 98_000, output_tokens: 210_000 },
      });
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
        // 播种上次测速结果(meta 持久化,重启后徽章仍在)
        setBatchResults((prev) => {
          const next = { ...prev };
          for (const p of list) {
            if (p.last_test && !next[p.id]) {
              next[p.id] = {
                ok: p.last_test.ok,
                latency_ms: p.last_test.latency_ms,
                message: p.last_test.ok ? "" : "不可达",
              };
            }
          }
          return next;
        });
        invoke<Record<string, UsageSummary>>("get_usage_map", { appType: app })
          .then(setUsageMap)
          .catch(() => setUsageMap({}));
      }
    } catch (e) {
      toast("error", humanizeError(String(e), t));
    }
  }, [toast]);

  useEffect(() => {
    setBatchResults({});
    void refresh(activeApp);
  }, [activeApp, refresh]);

  // 首启向导:首次加载完成、无任何供应商、未看过 → 弹出 3 步向导
  useEffect(() => {
    if (IS_DEMO) return;
    if (hasCache && providers.length === 0 && !localStorage.getItem(ONBOARD_KEY)) {
      setOnboardingOpen(true);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [hasCache]);

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
      // Cmd/Ctrl+Shift+M → MCP;Cmd/Ctrl+Shift+S → Skills
      if (e.shiftKey) {
        const k = e.key.toLowerCase();
        if (k === "m") {
          e.preventDefault();
          setCurrentView("mcp");
          return;
        }
        if (k === "s") {
          e.preventDefault();
          setCurrentView("skills");
          return;
        }
      }
      const idx = Number(e.key) - 1;
      if (Number.isInteger(idx) && idx >= 0 && idx < appsOrder.length) {
        e.preventDefault();
        setActiveApp(appsOrder[idx]);
        setCurrentView("providers");
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [appsOrder]);

  // 故障转移自动回退提示
  useEffect(() => {
    if (IS_DEMO) return;
    let un: (() => void) | undefined;
    void listen<{ chain: string }>("provider-fallback", (e) => {
      toast("error", t("fo.fallback", { chain: e.payload.chain }));
    }).then((fn) => (un = fn));
    return () => un?.();
  }, [toast, t]);

  // 托盘快速切换后:同步刷新 + toast(与主界面操作同一反馈通道)
  useEffect(() => {
    if (IS_DEMO) return;
    let un: (() => void) | undefined;
    void listen<{ appType: AppType; providerId: string; name: string }>(
      "provider-switched",
      (e) => {
        toast("success", t("toast.switchedTray", { name: e.payload.name }));
        cacheRef.current = {};
        void refresh(e.payload.appType);
      },
    ).then((fn) => (un = fn));
    return () => un?.();
  }, [refresh, toast]);

  // 全局快捷键 ⌘⇧K:唤起快速切换面板(演示模式同样可用,纯前端交互)
  useEffect(() => {
    let un: (() => void) | undefined;
    void listen("quick-switch", () => setQuickOpen(true)).then(
      (fn) => (un = fn),
    );
    return () => un?.();
  }, []);

  /** 首启导入:扫描现有 CLI 配置建供应商 */
  const [importing, setImporting] = useState(false);
  const runImport = async () => {
    setImporting(true);
    try {
      const list = await invoke<{ app: string; name: string; has_key: boolean }[]>(
        "import_existing",
      );
      if (list.length === 0) {
        toast("error", t("import.none"));
      } else {
        toast(
          "success",
          t("import.done", { n: list.length, names: list.map((x) => x.name.replace("导入的 ", "").replace("Imported ", "")).join(list.length > 1 && locale === "zh" ? "、" : ", ") }),
        );
        syncTray();
        await refresh(activeApp);
      }
    } catch (e) {
      toast("error", humanizeError(String(e), t));
    } finally {
      setImporting(false);
    }
  };

  /** 跨分组快速切换:面板打开时拉取所有分组的供应商(缓存优先) */
  const [qsGroups, setQsGroups] = useState<{ app: AppType; providers: Provider[] }[]>([]);
  useEffect(() => {
    if (!quickOpen) return;
    if (IS_DEMO) {
      setQsGroups(
        [...new Set(DEMO_PROVIDERS.map((p) => p.app_type))].map((a) => ({
          app: a,
          providers: DEMO_PROVIDERS.filter((p) => p.app_type === a),
        })),
      );
      return;
    }
    let alive = true;
    void (async () => {
      const gs: { app: AppType; providers: Provider[] }[] = [];
      for (const a of appsOrder) {
        let list = cacheRef.current[a];
        if (!list) {
          try {
            list = await invoke<Provider[]>("list_providers", { appType: a });
            cacheRef.current[a] = list;
          } catch {
            list = [];
          }
        }
        gs.push({ app: a, providers: list });
      }
      if (alive) setQsGroups(gs);
    })();
    return () => {
      alive = false;
    };
  }, [quickOpen]); // eslint-disable-line react-hooks/exhaustive-deps

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

  const switchProvider = async (provider: Provider, app: AppType = activeApp) => {
    // 记住切换前的当前供应商,支持 toast「撤销」一键切回
    const prev = cacheRef.current[app]?.find((p) => p.is_current);
    try {
      await invoke("switch_provider", {
        id: provider.id,
        appType: app,
      });
      // 跨组切换:同时把界面切到对应分组,所见即所得
      if (app !== activeApp) {
        setActiveApp(app);
      }
      toast(
        "success",
        t("toast.switched", { name: provider.name }),
        prev && prev.id !== provider.id
          ? {
              label: t("toast.undo"),
              run: () => void switchProvider(prev, app),
            }
          : undefined,
      );
      syncTray();
      await refresh(app);
    } catch (e) {
      toast("error", humanizeError(String(e), t));
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
      toast("success", t("toast.duplicated", { name: created.name }));
      syncTray();
      await refresh(activeApp);
      focusProvider(created.id);
    } catch (e) {
      toast("error", humanizeError(String(e), t));
    }
  };

  const deleteProvider = async () => {
    if (!confirmDelete) return;
    const target = confirmDelete;
    try {
      await invoke("delete_provider", { id: target.id });
      setConfirmDelete(null);
      toast("success", t("toast.deleted", { name: target.name }));
      syncTray();
      await refresh(activeApp);
    } catch (e) {
      toast("error", humanizeError(String(e), t));
    }
  };

  return (
    <div
      className={cn(
        "flex h-screen overflow-hidden bg-background text-foreground selection:bg-primary/30",
        layout === "top" || layout === "bottom" ? "flex-col" : "flex-row",
      )}
    >
      {/* 布局一:左侧导航栏(默认,腾讯会议式) */}
      {layout === "side" && (
        <Sidebar
          apps={appsOrder}
          activeApp={activeApp}
          onSwitchApp={setActiveApp}
          currentView={currentView}
          onViewChange={setCurrentView}
          proxyOk={proxyOk}
          proxyAddr={proxyAddr}
          onTakeover={() => setTakeoverOpen(true)}
          onAbout={() => setAboutOpen(true)}
          onAdd={() => setIsAddOpen(true)}
          isMac={isMac}
        />
      )}

      {/* 顶部横栏布局 */}
      {layout === "top" && (
        <AppHeaderBar
          position="top"
          apps={appsOrder}
          activeApp={activeApp}
          currentView={currentView}
          onViewChange={setCurrentView}
          onSwitchApp={setActiveApp}
          proxyOk={proxyOk}
          proxyAddr={proxyAddr}
          onTakeover={() => setTakeoverOpen(true)}
          isMac={isMac}
        />
      )}

      {/* 右侧主区:内容直接铺满(标题与添加入口在侧栏) */}
      <div className="flex-1 min-w-0 min-h-0 flex flex-col">
      {/* 主内容区:供应商卡片列表 */}
      <main
        className={cn(
          "flex-1 min-h-0 flex flex-col overflow-y-auto",
          // 右侧/底部布局:macOS 红绿灯悬于主区左上,顶部避让
          (layout === "right" || layout === "bottom") && isMac && "pt-7",
        )}
        data-tauri-drag-region
      >
        {currentView === "skills" && (
          <div className="px-6 py-6 max-w-[760px] w-full mx-auto animate-fade-in">
            <SkillsPage
              onError={(m) => toast("error", humanizeError(m, t))}
              onSuccess={(m) => toast("success", m)}
            />
          </div>
        )}
        {currentView === "mcp" && (
          <div className="px-6 py-6 max-w-[760px] w-full mx-auto animate-fade-in">
            <McpPage
              onError={(m) => toast("error", humanizeError(m, t))}
              onSuccess={(m) => toast("success", m)}
            />
          </div>
        )}
        {currentView === "settings" && (
          <div className="px-6 py-6 max-w-[760px] w-full mx-auto animate-fade-in">
            <SettingsPage
              onError={(m) => toast("error", humanizeError(m, t))}
              onSuccess={(m) => toast("success", m)}
              layout={layout}
              onLayoutChange={setLayout}
              apps={appsOrder}
              onAppsChange={updateApps}
            />
          </div>
        )}
        {currentView === "usage" && (
          <div className="px-6 py-6 max-w-[760px] w-full mx-auto my-auto animate-fade-in">
            <UsagePage
              app={activeApp}
              providers={providers}
              onError={(m) => toast("error", humanizeError(m, t))}
              onInfo={(m) => toast("success", m)}
            />
          </div>
        )}
        {currentView === "logs" && (
          <div className="px-6 py-6 max-w-[860px] w-full mx-auto my-auto animate-fade-in">
            <LogsPage
              app={activeApp}
              providers={providers}
              onError={(m) => toast("error", humanizeError(m, t))}
            />
          </div>
        )}
        {currentView === "providers" && (
        <div className="px-6 pt-6 flex flex-col flex-1 min-h-0 overflow-hidden">
          <div className="flex-1 overflow-y-auto overflow-x-hidden pb-12 px-1">
            <div className="max-w-[760px] mx-auto w-full h-full flex flex-col space-y-4 animate-fade-in" key={activeApp}>
              <GatewayStatusStrip />
              {/* 候选链(Clash 式):开启故障转移时展示 A → B → C */}
              {chain.length > 0 && (
                <div className="flex items-center gap-1.5 text-xs text-muted-foreground rounded-xl border border-dashed border-border px-4 py-2">
                  <span>{t("gw.chain")}</span>
                  {chain.map((n, i) => (
                    <span key={i} className="flex items-center gap-1.5">
                      {i > 0 && <span className="text-border">→</span>}
                      <span className={i === 0 ? "font-medium text-foreground" : ""}>{n}</span>
                    </span>
                  ))}
                </div>
              )}
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
                <div className="flex-1 flex flex-col items-center justify-center gap-3 text-center">
                  <div className="h-12 w-12 rounded-xl bg-muted flex items-center justify-center border border-border">
                    <Boxes className="h-6 w-6 text-muted-foreground" />
                  </div>
                  <p className="text-base font-medium">{t("empty.title", { app: activeApp })}</p>
                  <p className="text-sm text-muted-foreground -mt-2">
                    {t("empty.desc")}
                  </p>
                  <div className="flex items-center gap-2 mt-2">
                    <Button onClick={() => setIsAddOpen(true)}>
                      <Plus className="w-4 h-4 mr-1" />
                      {t("empty.cta")}
                    </Button>
                    {!IS_DEMO && (
                      <Button
                        variant="outline"
                        disabled={importing}
                        onClick={() => void runImport()}
                      >
                        <Download className="w-4 h-4 mr-1" />
                        {importing ? t("empty.importing") : t("empty.import")}
                      </Button>
                    )}
                  </div>
                  <p className="text-xs text-muted-foreground">
                    {t("empty.kbd")}{" "}
                    <kbd className="px-1.5 py-0.5 rounded border border-border bg-muted font-mono text-[10px]">
                      ⌘N
                    </kbd>
                  </p>
                </div>
              )}

              {providers.length > 0 && (
                <div className="flex items-center justify-end gap-1 -mb-2">
                  <Button
                    variant={latencySort ? "secondary" : "ghost"}
                    size="sm"
                    title={t("gw.latencySort")}
                    onClick={() => setLatencySort((v) => !v)}
                  >
                    <Gauge className="w-4 h-4 mr-1" />
                    {t("gw.latencySort")}
                  </Button>
                  <Button variant="ghost" size="sm" disabled={testingAll} onClick={() => void runTestAll()}>
                    <Activity className={cn("w-4 h-4 mr-1", testingAll && "animate-pulse")} />
                    {testingAll ? t("provider.testingAll") : t("provider.testAll")}
                  </Button>
                </div>
              )}

              {(latencySort
                ? [...providers].sort((a, b) => {
                    const la = a.last_test?.ok ? a.last_test.latency_ms : Infinity;
                    const lb = b.last_test?.ok ? b.last_test.latency_ms : Infinity;
                    return la - lb;
                  })
                : providers
              ).map((p) => (
                <div
                  key={p.id}
                  draggable
                  onDragStart={(e) => {
                    setDragId(p.id);
                    e.dataTransfer.effectAllowed = "move";
                  }}
                  onDragOver={(e) => {
                    e.preventDefault();
                    setDragOverId(p.id);
                  }}
                  onDragLeave={() => setDragOverId((x) => (x === p.id ? null : x))}
                  onDrop={(e) => {
                    e.preventDefault();
                    dropOn(p.id);
                  }}
                  onDragEnd={() => {
                    setDragId(null);
                    setDragOverId(null);
                  }}
                  className={cn(
                    "rounded-xl transition-shadow",
                    dragId === p.id && "opacity-40",
                    dragOverId === p.id && dragId !== p.id && "ring-2 ring-blue-500/50",
                  )}
                  title={t("provider.dragHint")}
                >
                  <ProviderCard
                    provider={p}
                    isCurrent={p.is_current}
                    app={activeApp}
                    highlight={p.id === highlightId}
                    onSwitch={(provider) => void switchProvider(provider)}
                    onEdit={(provider) => setEditingProvider(provider)}
                    onDuplicate={(provider) => void duplicateProvider(provider)}
                    onDelete={(provider) => setConfirmDelete(provider)}
                    usage={usageMap[p.id]}
                    batchResult={batchResults[p.id] ?? null}
                    testedAt={p.last_test?.ts ?? null}
                    onError={(m) => toast("error", humanizeError(m, t))}
                    onCopyUrl={(url) => {
                      navigator.clipboard
                        .writeText(url)
                        .then(() => toast("success", t("toast.copied")))
                        .catch(() => toast("error", t("toast.copyFailed")));
                    }}
                  />
                </div>
              ))}
            </div>
          </div>
        </div>
        )}
      </main>
      </div>

      {/* 右侧边栏布局:导航在右 */}
      {layout === "right" && (
        <Sidebar
          apps={appsOrder}
          edge="right"
          activeApp={activeApp}
          onSwitchApp={setActiveApp}
          currentView={currentView}
          onViewChange={setCurrentView}
          proxyOk={proxyOk}
          proxyAddr={proxyAddr}
          onTakeover={() => setTakeoverOpen(true)}
          onAbout={() => setAboutOpen(true)}
          onAdd={() => setIsAddOpen(true)}
          isMac={isMac}
        />
      )}

      {/* 底部横栏布局 */}
      {layout === "bottom" && (
        <AppHeaderBar
          position="bottom"
          apps={appsOrder}
          activeApp={activeApp}
          currentView={currentView}
          onViewChange={setCurrentView}
          onSwitchApp={setActiveApp}
          proxyOk={proxyOk}
          proxyAddr={proxyAddr}
          onTakeover={() => setTakeoverOpen(true)}
          isMac={isMac}
        />
      )}

      <ToastStack items={toasts} onDismiss={dismissToast} />

      <AboutDialog open={aboutOpen} onOpenChange={setAboutOpen} />

      <QuickSwitchPanel
        open={quickOpen}
        onClose={() => setQuickOpen(false)}
        app={activeApp}
        providers={providers}
        groups={qsGroups}
        onPick={(p, a) => void switchProvider(p, a)}
      />

      <OnboardingDialog
        open={onboardingOpen}
        onOpenChange={setOnboardingOpen}
        apps={appsOrder}
        onAppsChange={updateApps}
        onAdd={() => setIsAddOpen(true)}
        onImport={() => void runImport()}
        providersCount={providers.length}
        onDone={() => void refresh(activeApp)}
      />

      <TakeoverDialog
        open={takeoverOpen}
        onOpenChange={setTakeoverOpen}
        onError={(m) => toast("error", humanizeError(m, t))}
        onSuccess={(m) => toast("success", m)}
      />

      <AddProviderDialog
        open={isAddOpen}
        onOpenChange={setIsAddOpen}
        appId={activeApp}
        onCreated={async (created) => {
          toast("success", t("toast.added", { name: created.name }));
          syncTray();
          await refresh(activeApp);
          focusProvider(created.id);
        }}
        onError={(msg) => toast("error", humanizeError(msg, t))}
      />

      <EditProviderDialog
        provider={editingProvider}
        defaultProtocol={APP_PROTOCOL[activeApp]}
        onOpenChange={(open: boolean) => {
          if (!open) setEditingProvider(null);
        }}
        onSaved={async (saved) => {
          toast("success", t("toast.saved", { name: saved.name }));
          syncTray();
          await refresh(activeApp);
          focusProvider(saved.id);
        }}
        onError={(msg) => toast("error", humanizeError(msg, t))}
      />

      <ConfirmDialog
        isOpen={Boolean(confirmDelete)}
        title={t("confirm.deleteTitle")}
        message={t("confirm.deleteMsg", { name: confirmDelete?.name ?? "" })}
        onConfirm={() => void deleteProvider()}
        onCancel={() => setConfirmDelete(null)}
      />
    </div>
  );
}

export default App;
