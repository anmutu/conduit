import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { BookOpen, Download, FolderOpen, Plus, RefreshCw, Sparkles, Trash2, X } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { useI18n } from "@/i18n";
import { cn } from "@/lib/utils";

interface SkillEntry {
  id: string;
  name: string;
  description: string;
  apps: string[];
  has_files: boolean;
}

const TARGET_APPS = [
  { key: "claude", label: "Claude Code" },
  { key: "codex", label: "Codex" },
  { key: "opencode", label: "OpenCode" },
];

/** Skills 统一管理:vault(~/.keyway/skills)为源,同步复制到各 CLI 的 skills 目录。 */
export function SkillsPage({
  onError,
  onSuccess,
}: {
  onError: (msg: string) => void;
  onSuccess: (msg: string) => void;
}) {
  const { t } = useI18n();
  const [skills, setSkills] = useState<SkillEntry[]>([]);
  const [editing, setEditing] = useState<{ id: string; content: string; apps: string[]; isNew: boolean } | null>(null);
  const [importOpen, setImportOpen] = useState(false);
  const [importSrc, setImportSrc] = useState<Record<string, string[]>>({});
  const [syncing, setSyncing] = useState(false);

  const reload = useCallback(() => {
    invoke<SkillEntry[]>("list_skills")
      .then(setSkills)
      .catch(() => setSkills([]));
  }, []);
  useEffect(() => reload(), [reload]);

  const save = (s: { id: string; content: string; apps: string[] }) => {
    // SKILL.md 头部需含 frontmatter name(同步依赖它作为 skill 名)
    const m = s.content.match(/^---\n([\s\S]*?)\n---/);
    const hasName = m?.[1].split("\n").some((l) => /^name:\s*\S/.test(l));
    if (!hasName) {
      onError(t("sk.needFm"));
      return;
    }
    invoke("save_skill", s)
      .then(() => {
        reload();
        onSuccess(t("sk.saved"));
      })
      .catch((e) => onError(String(e)));
  };

  const remove = (s: SkillEntry) => {
    invoke("delete_skill", { id: s.id })
      .then(() => {
        reload();
        onSuccess(t("sk.deleted"));
      })
      .catch((e) => onError(String(e)));
  };

  const syncNow = () => {
    setSyncing(true);
    invoke<string[]>("sync_skills")
      .then((r) => onSuccess(r.join("; ")))
      .catch((e) => onError(String(e)))
      .finally(() => setSyncing(false));
  };

  const openImport = () => {
    setImportOpen(true);
    for (const a of TARGET_APPS) {
      invoke<string[]>("scan_cli_skills", { app: a.key })
        .then((ids) => setImportSrc((prev) => ({ ...prev, [a.key]: ids })))
        .catch(() => setImportSrc((prev) => ({ ...prev, [a.key]: [] })));
    }
  };

  const doImport = (app: string, id: string) => {
    invoke("import_skill", { app, id })
      .then(() => {
        reload();
        onSuccess(t("sk.imported"));
      })
      .catch((e) => onError(String(e)));
  };

  return (
    <div className="space-y-4">
      <div className="flex items-start justify-between gap-4">
        <div>
          <h2 className="flex items-center gap-2 text-lg font-semibold">
            <Sparkles className="w-5 h-5 text-blue-500" />
            {t("sk.title")}
          </h2>
          <p className="text-sm text-muted-foreground mt-1">{t("sk.desc")}</p>
        </div>
        <div className="flex gap-2 shrink-0">
          <Button variant="outline" size="sm" onClick={openImport}>
            <Download className="w-4 h-4 mr-1" />
            {t("sk.import")}
          </Button>
          <Button
            variant="outline"
            size="sm"
            onClick={() =>
              invoke("open_skills_vault").catch((e) => onError(String(e)))
            }
          >
            <FolderOpen className="w-4 h-4" />
            {t("sk.openVault")}
          </Button>
          <Button variant="outline" size="sm" onClick={syncNow} disabled={syncing}>
            <RefreshCw className={cn("w-4 h-4 mr-1", syncing && "animate-spin")} />
            {t("sk.sync")}
          </Button>
          <Button
            size="sm"
            onClick={() => setEditing({ id: "", content: BOILERPLATE, apps: ["claude"], isNew: true })}
          >
            <Plus className="w-4 h-4 mr-1" />
            {t("common.add")}
          </Button>
        </div>
      </div>

      {skills.length === 0 ? (
        <div className="rounded-xl border border-dashed border-border p-8 text-center text-sm text-muted-foreground">
          {t("sk.empty")}
        </div>
      ) : (
        <div className="space-y-2">
          {skills.map((s) => (
            <div
              key={s.id}
              className="flex items-center gap-3 rounded-xl border border-border bg-card px-4 py-3"
            >
              <BookOpen className="w-4 h-4 text-muted-foreground shrink-0" />
              <div className="min-w-0 flex-1">
                <div className="flex items-center gap-2">
                  <span className="font-medium truncate">{s.name}</span>
                  <code className="text-xs text-muted-foreground">{s.id}</code>
                  {s.has_files && (
                    <span className="text-[10px] text-muted-foreground">+files</span>
                  )}
                </div>
                <div className="text-xs text-muted-foreground truncate">
                  {s.description || "—"}
                </div>
              </div>
              <div className="flex gap-1 shrink-0">
                {TARGET_APPS.map((a) => {
                  const on = s.apps.includes(a.key);
                  return (
                    <button
                      key={a.key}
                      type="button"
                      title={on ? t("sk.disableApp") : t("sk.enableApp")}
                      className={cn(
                        "rounded-md border px-2 py-1 text-xs",
                        on
                          ? "border-blue-500 text-blue-600"
                          : "border-border text-muted-foreground/60",
                      )}
                      onClick={() =>
                        invoke("set_skill_apps", {
                          id: s.id,
                          apps: on ? s.apps.filter((k) => k !== a.key) : [...s.apps, a.key],
                        })
                          .then(() => {
                            reload();
                            onSuccess(t("sk.saved"));
                          })
                          .catch((e) => onError(String(e)))
                      }
                    >
                      {a.label}
                    </button>
                  );
                })}
              </div>
              <Button
                variant="ghost"
                size="sm"
                className="h-8 px-2 text-muted-foreground hover:text-foreground"
                onClick={() =>
                  invoke<string>("read_skill_content", { id: s.id }).then(
                    (content) => setEditing({ id: s.id, content, apps: [...s.apps], isNew: false }),
                  ).catch((e) => onError(String(e)))
                }
              >
                {t("common.edit")}
              </Button>
              <Button
                variant="ghost"
                size="sm"
                className="h-8 px-2 text-muted-foreground hover:text-destructive"
                onClick={() => remove(s)}
              >
                <Trash2 className="w-4 h-4" />
              </Button>
            </div>
          ))}
        </div>
      )}

      <SkillEditDialog
        draft={editing}
        onClose={() => setEditing(null)}
        onSave={(s) => {
          save(s);
          setEditing(null);
        }}
      />

      <Dialog open={importOpen} onOpenChange={setImportOpen}>
        <DialogContent className="max-w-md">
          <DialogHeader>
            <DialogTitle>{t("sk.import")}</DialogTitle>
          </DialogHeader>
          <div className="space-y-3 max-h-[50vh] overflow-y-auto">
            {TARGET_APPS.map((a) => (
              <div key={a.key} className="space-y-1.5">
                <Label>{a.label}</Label>
                {(importSrc[a.key] ?? []).length === 0 ? (
                  <p className="text-xs text-muted-foreground">{t("sk.noSrc")}</p>
                ) : (
                  <div className="flex flex-wrap gap-1.5">
                    {(importSrc[a.key] ?? []).map((id) => (
                      <button
                        key={id}
                        type="button"
                        onClick={() => doImport(a.key, id)}
                        className="rounded-md border border-border px-2 py-1 text-xs hover:border-blue-500 hover:text-blue-600"
                      >
                        {id}
                      </button>
                    ))}
                  </div>
                )}
              </div>
            ))}
          </div>
          <div className="flex justify-end">
            <Button variant="outline" onClick={() => setImportOpen(false)}>
              <X className="w-4 h-4 mr-1" />
              {t("common.close")}
            </Button>
          </div>
        </DialogContent>
      </Dialog>
    </div>
  );
}

const BOILERPLATE = `---
name: "my-skill"
description: "这个 skill 做什么、什么时候触发"
---

# My Skill

在这里写指令正文(Markdown)。
`;

function SkillEditDialog({
  draft,
  onClose,
  onSave,
}: {
  draft: { id: string; content: string; apps: string[]; isNew: boolean } | null;
  onClose: () => void;
  onSave: (s: { id: string; content: string; apps: string[] }) => void;
}) {
  const { t } = useI18n();
  const [d, setD] = useState(draft);
  useEffect(() => setD(draft), [draft]);
  if (!d) return null;

  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-xl">
        <DialogHeader>
          <DialogTitle>{d.isNew ? t("sk.add") : t("sk.edit")}</DialogTitle>
        </DialogHeader>
        <div className="space-y-3">
          <div className="space-y-1.5">
            <Label>{t("sk.fId")}</Label>
            <Input
              value={d.id}
              disabled={!d.isNew}
              placeholder="my-skill"
              onChange={(e) => setD({ ...d, id: e.target.value })}
            />
            <p className="text-[11px] text-muted-foreground">{t("sk.fIdHint")}</p>
          </div>
          <div className="space-y-1.5">
            <Label>SKILL.md</Label>
            <textarea
              value={d.content}
              onChange={(e) => setD({ ...d, content: e.target.value })}
              spellCheck={false}
              className="w-full h-64 rounded-md border border-border bg-background px-3 py-2 font-mono text-xs resize-y focus:outline-none focus:ring-1 focus:ring-blue-500"
            />
            <p className="text-[11px] text-muted-foreground">{t("sk.fmHint")}</p>
          </div>
          <div className="space-y-1.5">
            <Label>{t("sk.fApps")}</Label>
            <div className="flex gap-2">
              {TARGET_APPS.map((a) => {
                const on = d.apps.includes(a.key);
                return (
                  <button
                    key={a.key}
                    type="button"
                    className={cn(
                      "rounded-md border px-3 py-1.5 text-sm",
                      on ? "border-blue-500 text-blue-600" : "border-border text-muted-foreground",
                    )}
                    onClick={() =>
                      setD({
                        ...d,
                        apps: on ? d.apps.filter((k) => k !== a.key) : [...d.apps, a.key],
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
            {t("common.cancel")}
          </Button>
          <Button
            onClick={() => onSave(d)}
            disabled={!d.id.trim() || !d.content.trim() || d.apps.length === 0}
          >
            {t("common.save")}
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  );
}
