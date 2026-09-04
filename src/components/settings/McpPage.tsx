import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Activity, Plug, Plus, RefreshCw, Server, Trash2, X } from "lucide-react";
import { Button } from "@/components/ui/button";
import { ConfirmDialog } from "@/components/ConfirmDialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { useI18n } from "@/i18n";
import { cn } from "@/lib/utils";

interface McpServer {
  id: string;
  name: string;
  config: {
    command?: string;
    args?: string[];
    env?: Record<string, string>;
    type?: string;
    url?: string;
  };
  apps: string[];
  enabled: boolean;
  created_at: number;
}

const TARGET_APPS = [
  { key: "claude", label: "Claude Code" },
  { key: "codex", label: "Codex" },
];

/** MCP 统一管理:一处定义,同步写入各 CLI 配置(Claude ~/.claude.json / Codex config.toml)。 */
export function McpPage({
  onError,
  onSuccess,
}: {
  onError: (msg: string) => void;
  onSuccess: (msg: string) => void;
}) {
  const { t } = useI18n();
  const [testing, setTesting] = useState<string | null>(null);

  const test = async (id: string) => {
    setTesting(id);
    try {
      const msg = await invoke<string>("test_mcp_server", { id });
      onSuccess(msg);
    } catch (e) {
      onError(String(e));
    } finally {
      setTesting(null);
    }
  };
  const [servers, setServers] = useState<McpServer[]>([]);
  const [editing, setEditing] = useState<McpServer | null>(null);
  const [syncing, setSyncing] = useState(false);
  const [pendingDelete, setPendingDelete] = useState<McpServer | null>(null);

  const reload = useCallback(() => {
    invoke<McpServer[]>("list_mcp_servers")
      .then(setServers)
      .catch(() => setServers([]));
  }, []);

  useEffect(() => reload(), [reload]);

  const save = (s: McpServer) => {
    invoke("save_mcp_server", { server: s })
      .then(() => {
        reload();
        onSuccess(t("mcp.saved"));
      })
      .catch((e) => onError(String(e)));
  };

  const toggle = (s: McpServer, enabled: boolean) => {
    invoke("set_mcp_server_enabled", { id: s.id, enabled })
      .then(reload)
      .catch((e) => onError(String(e)));
  };

  const remove = (s: McpServer) => {
    invoke("delete_mcp_server", { id: s.id })
      .then(() => {
        reload();
        onSuccess(t("mcp.deleted"));
      })
      .catch((e) => onError(String(e)));
  };

  const syncNow = () => {
    setSyncing(true);
    invoke<string[]>("sync_mcp_servers")
      .then((report) => onSuccess(report.join("; ") || t("mcp.noTarget")))
      .catch((e) => onError(String(e)))
      .finally(() => setSyncing(false));
  };

  return (
    <div className="space-y-4">
      <div className="flex items-start justify-between gap-4">
        <div>
          <h2 className="flex items-center gap-2 text-lg font-semibold">
            <Plug className="w-5 h-5 text-blue-500" />
            {t("mcp.title")}
          </h2>
          <p className="text-sm text-muted-foreground mt-1">{t("mcp.desc")}</p>
        </div>
        <div className="flex gap-2 shrink-0">
          <Button variant="outline" size="sm" onClick={syncNow} disabled={syncing}>
            <RefreshCw className={cn("w-4 h-4 mr-1", syncing && "animate-spin")} />
            {t("mcp.sync")}
          </Button>
          <Button size="sm" onClick={() => setEditing(newServer())}>
            <Plus className="w-4 h-4 mr-1" />
            {t("common.add")}
          </Button>
        </div>
      </div>

      {servers.length === 0 ? (
        <div className="rounded-xl border border-dashed border-border p-8 text-center text-sm text-muted-foreground">
          {t("mcp.empty")}
        </div>
      ) : (
        <div className="space-y-2">
          {servers.map((s) => (
            <div
              key={s.id}
              className="flex items-center gap-3 rounded-xl border border-border bg-card px-4 py-3"
            >
              <Server className="w-4 h-4 text-muted-foreground shrink-0" />
              <div className="min-w-0 flex-1">
                <div className="flex items-center gap-2">
                  <span className="font-medium truncate">{s.name}</span>
                  <code className="text-xs text-muted-foreground">{s.id}</code>
                </div>
                <div className="text-xs text-muted-foreground truncate">
                  {s.config.url
                    ? `${s.config.type ?? "http"} · ${s.config.url}`
                    : [s.config.command, ...(s.config.args ?? [])].join(" ")}
                  {" → "}
                  {s.apps
                    .map((a) => TARGET_APPS.find((t2) => t2.key === a)?.label ?? a)
                    .join(", ")}
                </div>
              </div>
              <Button
                variant="ghost"
                size="sm"
                className="h-8 px-2 text-muted-foreground hover:text-foreground"
                disabled={testing === s.id}
                onClick={() => void test(s.id)}
                title={t("mcp.test")}
              >
                <Activity className={"w-4 h-4 " + (testing === s.id ? "animate-pulse" : "")} />
              </Button>
              <Switch checked={s.enabled} onCheckedChange={(v) => toggle(s, v)} />
              <Button
                variant="ghost"
                size="sm"
                className="h-8 px-2 text-muted-foreground hover:text-foreground"
                onClick={() => setEditing({ ...s, config: { ...s.config } })}
              >
                {t("common.edit")}
              </Button>
              <Button
                variant="ghost"
                size="sm"
                className="h-8 px-2 text-muted-foreground hover:text-destructive"
                onClick={() => setPendingDelete(s)}
              >
                <Trash2 className="w-4 h-4" />
              </Button>
            </div>
          ))}
        </div>
      )}

      <ConfirmDialog
        isOpen={!!pendingDelete}
        title={t("mcp.deleteTitle")}
        message={t("mcp.deleteConfirm", { name: pendingDelete?.name ?? "" })}
        onConfirm={() => {
          const target = pendingDelete;
          setPendingDelete(null);
          if (target) remove(target);
        }}
        onCancel={() => setPendingDelete(null)}
      />

      <McpEditDialog
        server={editing}
        onClose={() => setEditing(null)}
        onSave={(s) => {
          save(s);
          setEditing(null);
        }}
        exists={servers.some((x) => x.id === editing?.id)}
      />
    </div>
  );
}

function newServer(): McpServer {
  return {
    id: "",
    name: "",
    config: { command: "", args: [] },
    apps: ["claude"],
    enabled: true,
    created_at: 0,
  };
}

function McpEditDialog({
  server,
  onClose,
  onSave,
  exists,
}: {
  server: McpServer | null;
  onClose: () => void;
  onSave: (s: McpServer) => void;
  exists: boolean;
}) {
  const { t } = useI18n();
  const [draft, setDraft] = useState<McpServer | null>(server);
  useEffect(() => setDraft(server), [server]);
  if (!draft) return null;

  const set = (patch: Partial<McpServer>) => setDraft({ ...draft, ...patch });
  const isRemote = !!draft.config.url;
  const argsText = (draft.config.args ?? []).join(" ");
  const envText = Object.entries(draft.config.env ?? {})
    .map(([k, v]) => `${k}=${v}`)
    .join("\n");
  const parseEnv = (text: string) => {
    const env: Record<string, string> = {};
    for (const line of text.split("\n")) {
      const i = line.indexOf("=");
      if (i > 0) env[line.slice(0, i).trim()] = line.slice(i + 1).trim();
    }
    return env;
  };

  const submit = () => {
    onSave({
      ...draft,
      name: draft.name.trim() || draft.id.trim(),
      config: isRemote
        ? { type: draft.config.type ?? "http", url: draft.config.url }
        : {
            command: draft.config.command?.trim(),
            args: argsText
              .split(/\s+/)
              .filter(Boolean)
              .map((a) => a.replace(/^"(.*)"$/, "$1")),
            env: draft.config.env,
          },
    });
  };

  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-lg">
        <DialogHeader>
          <DialogTitle>{exists ? t("mcp.edit") : t("mcp.add")}</DialogTitle>
        </DialogHeader>
        <div className="space-y-3">
          <div className="grid grid-cols-2 gap-3">
            <div className="space-y-1.5">
              <Label>{t("mcp.fId")}</Label>
              <Input
                value={draft.id}
                disabled={exists}
                placeholder="filesystem"
                onChange={(e) => set({ id: e.target.value })}
              />
              <p className="text-[11px] text-muted-foreground">{t("mcp.fIdHint")}</p>
            </div>
            <div className="space-y-1.5">
              <Label>{t("mcp.fName")}</Label>
              <Input
                value={draft.name}
                placeholder={t("mcp.fNamePh")}
                onChange={(e) => set({ name: e.target.value })}
              />
            </div>
          </div>

          <div className="flex gap-2 text-sm">
            <button
              type="button"
              className={cn(
                "rounded-md border px-3 py-1.5",
                !isRemote ? "border-blue-500 text-blue-600" : "border-border text-muted-foreground",
              )}
              onClick={() => set({ config: { command: "", args: [] } })}
            >
              stdio
            </button>
            <button
              type="button"
              className={cn(
                "rounded-md border px-3 py-1.5",
                isRemote ? "border-blue-500 text-blue-600" : "border-border text-muted-foreground",
              )}
              onClick={() => set({ config: { type: "http", url: "" } })}
            >
              http / sse
            </button>
          </div>

          {isRemote ? (
            <div className="space-y-1.5">
              <Label>URL</Label>
              <Input
                value={draft.config.url ?? ""}
                placeholder="https://mcp.example.com/sse"
                onChange={(e) =>
                  set({ config: { ...draft.config, url: e.target.value } })
                }
              />
              <p className="text-[11px] text-muted-foreground">{t("mcp.remoteHint")}</p>
            </div>
          ) : (
            <>
              <div className="space-y-1.5">
                <Label>{t("mcp.fCommand")}</Label>
                <Input
                  value={draft.config.command ?? ""}
                  placeholder="npx"
                  onChange={(e) =>
                    set({ config: { ...draft.config, command: e.target.value } })
                  }
                />
              </div>
              <div className="space-y-1.5">
                <Label>{t("mcp.fArgs")}</Label>
                <Input
                  defaultValue={argsText}
                  placeholder="-y @modelcontextprotocol/server-filesystem /tmp"
                  onChange={(e) =>
                    set({ config: { ...draft.config, args: e.target.value.split(/\s+/) } })
                  }
                />
                <p className="text-[11px] text-muted-foreground">{t("mcp.fArgsHint")}</p>
              </div>
              <div className="space-y-1.5">
                <Label>{t("mcp.fEnv")}</Label>
                <textarea
                  defaultValue={envText}
                  placeholder={"API_TOKEN=xxx\nDEBUG=1"}
                  onChange={(e) =>
                    set({ config: { ...draft.config, env: parseEnv(e.target.value) } })
                  }
                  className="w-full min-h-[56px] rounded-md border border-border bg-background px-2 py-1.5 font-mono text-xs"
                />
                <p className="text-[11px] text-muted-foreground">{t("mcp.fEnvHint")}</p>
              </div>
            </>
          )}

          <div className="space-y-1.5">
            <Label>{t("mcp.fApps")}</Label>
            <div className="flex gap-3">
              {TARGET_APPS.map((a) => {
                const on = draft.apps.includes(a.key);
                return (
                  <button
                    key={a.key}
                    type="button"
                    className={cn(
                      "rounded-md border px-3 py-1.5 text-sm",
                      on
                        ? "border-blue-500 text-blue-600"
                        : "border-border text-muted-foreground",
                    )}
                    onClick={() =>
                      set({
                        apps: on
                          ? draft.apps.filter((k) => k !== a.key)
                          : [...draft.apps, a.key],
                      })
                    }
                  >
                    {a.label}
                  </button>
                );
              })}
            </div>
          </div>
        </div>

        <div className="flex justify-end gap-2 mt-2">
          <Button variant="outline" onClick={onClose}>
            <X className="w-4 h-4 mr-1" />
            {t("common.cancel")}
          </Button>
          <Button
            onClick={submit}
            disabled={
              !draft.id.trim() ||
              (isRemote ? !draft.config.url?.trim() : !draft.config.command?.trim()) ||
              draft.apps.length === 0
            }
          >
            {t("common.save")}
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  );
}
