import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Loader2, FolderOpen, Check } from "lucide-react";

interface AppSettings {
  threads_per_download: number;
  max_concurrent_downloads: number;
  download_dir: string;
  quiet_hours_enabled: boolean;
  quiet_hours_start?: string;
  quiet_hours_end?: string;
}

export default function SettingsPage() {
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);

  useEffect(() => {
    invoke<AppSettings>("get_settings").then(setSettings).catch(console.error);
  }, []);

  async function handleSave() {
    if (!settings) return;
    setSaving(true);
    try {
      await invoke("save_settings_cmd", { newSettings: settings });
      setSaved(true);
      setTimeout(() => setSaved(false), 2000);
    } catch (e) {
      console.error(e);
    } finally {
      setSaving(false);
    }
  }

  async function pickDir() {
    const path = await open({ directory: true });
    if (typeof path === "string") {
      setSettings((s) => s ? { ...s, download_dir: path } : s);
    }
  }

  if (!settings) {
    return (
      <div className="flex items-center gap-2 text-muted-foreground">
        <Loader2 size={16} className="animate-spin" />
        Loading settings…
      </div>
    );
  }

  function set<K extends keyof AppSettings>(key: K, val: AppSettings[K]) {
    setSettings((s) => s ? { ...s, [key]: val } : s);
  }

  return (
    <div className="flex flex-col gap-6 max-w-lg">
      <div>
        <h1 className="text-xl font-semibold">Settings</h1>
        <p className="text-sm text-muted-foreground mt-1">Configure download behavior.</p>
      </div>

      <div className="flex flex-col gap-5">
        <section className="flex flex-col gap-3">
          <h2 className="text-sm font-semibold text-muted-foreground uppercase tracking-wider">
            Downloads
          </h2>

          <div className="flex flex-col gap-1.5">
            <label className="text-sm font-medium">Threads per download</label>
            <p className="text-xs text-muted-foreground">
              Number of parallel chunks per file (1-16)
            </p>
            <div className="flex items-center gap-3">
              <input
                type="range"
                min={1}
                max={16}
                value={settings.threads_per_download}
                onChange={(e) => set("threads_per_download", Number(e.target.value))}
                className="flex-1 accent-[oklch(0.723_0.219_149.579)]"
              />
              <span className="text-sm font-mono w-6 text-center">
                {settings.threads_per_download}
              </span>
            </div>
          </div>

          <div className="flex flex-col gap-1.5">
            <label className="text-sm font-medium">Max concurrent downloads</label>
            <p className="text-xs text-muted-foreground">
              How many files download simultaneously (1-10)
            </p>
            <div className="flex items-center gap-3">
              <input
                type="range"
                min={1}
                max={10}
                value={settings.max_concurrent_downloads}
                onChange={(e) => set("max_concurrent_downloads", Number(e.target.value))}
                className="flex-1 accent-[oklch(0.723_0.219_149.579)]"
              />
              <span className="text-sm font-mono w-6 text-center">
                {settings.max_concurrent_downloads}
              </span>
            </div>
          </div>

          <div className="flex flex-col gap-1.5">
            <label className="text-sm font-medium">Download directory</label>
            <div className="flex gap-2">
              <Input
                value={settings.download_dir}
                onChange={(e) => set("download_dir", e.target.value)}
                className="font-mono text-xs"
              />
              <Button variant="outline" size="icon" onClick={pickDir}>
                <FolderOpen size={14} />
              </Button>
            </div>
          </div>
        </section>

        <section className="flex flex-col gap-3">
          <h2 className="text-sm font-semibold text-muted-foreground uppercase tracking-wider">
            Quiet Hours
          </h2>
          <p className="text-xs text-muted-foreground">
            Pause all downloads during the specified time window.
          </p>

          <div className="flex items-center gap-3">
            <input
              type="checkbox"
              id="qh"
              checked={settings.quiet_hours_enabled}
              onChange={(e) => set("quiet_hours_enabled", e.target.checked)}
              className="accent-[oklch(0.723_0.219_149.579)] h-4 w-4"
            />
            <label htmlFor="qh" className="text-sm">Enable quiet hours</label>
          </div>

          {settings.quiet_hours_enabled && (
            <div className="flex items-center gap-3">
              <div className="flex flex-col gap-1">
                <label className="text-xs text-muted-foreground">Start</label>
                <Input
                  type="time"
                  value={settings.quiet_hours_start ?? ""}
                  onChange={(e) => set("quiet_hours_start", e.target.value || undefined)}
                  className="w-32"
                />
              </div>
              <div className="flex flex-col gap-1">
                <label className="text-xs text-muted-foreground">End</label>
                <Input
                  type="time"
                  value={settings.quiet_hours_end ?? ""}
                  onChange={(e) => set("quiet_hours_end", e.target.value || undefined)}
                  className="w-32"
                />
              </div>
            </div>
          )}
        </section>
      </div>

      <Button variant="rd" onClick={handleSave} disabled={saving} className="w-fit">
        {saving ? <Loader2 size={14} className="animate-spin" /> : saved ? <Check size={14} /> : null}
        {saved ? "Saved!" : "Save Settings"}
      </Button>
    </div>
  );
}
