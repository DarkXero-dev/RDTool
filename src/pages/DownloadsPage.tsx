import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Button } from "@/components/ui/button";
import { Progress } from "@/components/ui/progress";
import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import { Play, Pause, X, Trash2, Clock } from "lucide-react";

interface QueuedDownload {
  id: string;
  filename: string;
  url: string;
  dest_path: string;
  status: string;
  priority: number;
  threads: number;
  scheduled_at?: string;
  total_bytes?: number;
  bytes_done: number;
  error_msg?: string;
  created_at: string;
}

interface ProgressEvent {
  id: string;
  bytes_done: number;
  total_bytes?: number;
  speed_bps: number;
  status: string;
}

function formatBytes(n: number): string {
  if (n < 1024 ** 2) return `${(n / 1024).toFixed(1)} KB`;
  if (n < 1024 ** 3) return `${(n / 1024 ** 2).toFixed(1)} MB`;
  return `${(n / 1024 ** 3).toFixed(2)} GB`;
}

function statusBadge(s: string): "default" | "success" | "secondary" | "destructive" | "outline" {
  if (s === "completed") return "success";
  if (s === "active") return "default";
  if (s === "failed") return "destructive";
  if (s === "paused") return "outline";
  return "secondary";
}

export default function DownloadsPage() {
  const [items, setItems] = useState<QueuedDownload[]>([]);
  const [progress, setProgress] = useState<Record<string, ProgressEvent>>({});
  const [scheduleId, setScheduleId] = useState<string | null>(null);
  const [scheduleAt, setScheduleAt] = useState("");

  async function refresh() {
    const data = await invoke<QueuedDownload[]>("get_queue").catch(() => []);
    setItems(data);
  }

  useEffect(() => {
    refresh();
    const unsub = listen<ProgressEvent>("download-progress", (e) => {
      setProgress((p) => ({ ...p, [e.payload.id]: e.payload }));
    });
    const unsub2 = listen("download-complete", () => refresh());
    const unsub3 = listen("download-error", () => refresh());
    return () => {
      unsub.then((f) => f());
      unsub2.then((f) => f());
      unsub3.then((f) => f());
    };
  }, []);

  async function start(id: string) {
    await invoke("start_download", { id }).catch(console.error);
    await refresh();
  }

  async function pause(id: string) {
    await invoke("pause_download", { id }).catch(console.error);
    await refresh();
  }

  async function cancel(id: string) {
    await invoke("cancel_download", { id }).catch(console.error);
    await refresh();
  }

  async function remove(id: string) {
    await invoke("remove_download", { id }).catch(console.error);
    setItems((i) => i.filter((x) => x.id !== id));
  }

  async function scheduleItem() {
    if (!scheduleId || !scheduleAt) return;
    await invoke("schedule_download", {
      id: scheduleId,
      at: new Date(scheduleAt).toISOString(),
    }).catch(console.error);
    setScheduleId(null);
    setScheduleAt("");
    await refresh();
  }

  return (
    <div className="flex flex-col gap-6 max-w-3xl">
      <div>
        <h1 className="text-xl font-semibold">Downloads</h1>
        <p className="text-sm text-muted-foreground mt-1">Queue, active, and completed downloads.</p>
      </div>

      {scheduleId && (
        <div className="flex gap-2 items-center p-3 rounded-lg border border-border bg-card">
          <Clock size={14} className="text-muted-foreground shrink-0" />
          <Input
            type="datetime-local"
            value={scheduleAt}
            onChange={(e) => setScheduleAt(e.target.value)}
            className="flex-1"
          />
          <Button variant="rd" size="sm" onClick={scheduleItem}>Set</Button>
          <Button variant="ghost" size="sm" onClick={() => setScheduleId(null)}>Cancel</Button>
        </div>
      )}

      <div className="flex flex-col gap-2">
        {items.length === 0 && (
          <p className="text-sm text-muted-foreground">No downloads in queue.</p>
        )}
        {items.map((item) => {
          const prog = progress[item.id];
          const bytesDone = prog?.bytes_done ?? item.bytes_done;
          const total = prog?.total_bytes ?? item.total_bytes;
          const pct = total ? Math.round((bytesDone / total) * 100) : 0;
          const speed = prog?.speed_bps ?? 0;

          return (
            <div key={item.id} className="rounded-lg border border-border bg-card p-4 flex flex-col gap-2">
              <div className="flex items-start justify-between gap-2">
                <div className="flex-1 min-w-0">
                  <div className="font-medium text-sm truncate">{item.filename}</div>
                  <div className="flex items-center gap-2 mt-1 flex-wrap">
                    <Badge variant={statusBadge(item.status)} className="text-[10px] px-1.5 py-0">
                      {item.status}
                    </Badge>
                    {total && (
                      <span className="text-xs text-muted-foreground">
                        {formatBytes(bytesDone)} / {formatBytes(total)}
                      </span>
                    )}
                    {speed > 0 && (
                      <span className="text-xs text-rd-green">{formatBytes(speed)}/s</span>
                    )}
                    {item.scheduled_at && item.status === "scheduled" && (
                      <span className="text-xs text-muted-foreground">
                        Scheduled: {new Date(item.scheduled_at).toLocaleString()}
                      </span>
                    )}
                    <span className="text-xs text-muted-foreground">{item.threads} threads</span>
                  </div>
                  {item.error_msg && (
                    <div className="text-xs text-destructive-foreground mt-1">{item.error_msg}</div>
                  )}
                </div>
                <div className="flex items-center gap-1 shrink-0">
                  {(item.status === "queued" || item.status === "paused") && (
                    <Button variant="ghost" size="icon" onClick={() => start(item.id)} title="Start">
                      <Play size={14} />
                    </Button>
                  )}
                  {item.status === "active" && (
                    <Button variant="ghost" size="icon" onClick={() => pause(item.id)} title="Pause">
                      <Pause size={14} />
                    </Button>
                  )}
                  {item.status !== "completed" && item.status !== "cancelled" && (
                    <>
                      <Button
                        variant="ghost"
                        size="icon"
                        onClick={() => setScheduleId(item.id)}
                        title="Schedule"
                      >
                        <Clock size={14} />
                      </Button>
                      <Button
                        variant="ghost"
                        size="icon"
                        onClick={() => cancel(item.id)}
                        title="Cancel"
                        className="text-muted-foreground hover:text-destructive-foreground"
                      >
                        <X size={14} />
                      </Button>
                    </>
                  )}
                  <Button
                    variant="ghost"
                    size="icon"
                    onClick={() => remove(item.id)}
                    title="Remove"
                    className="text-muted-foreground hover:text-destructive-foreground"
                  >
                    <Trash2 size={14} />
                  </Button>
                </div>
              </div>
              {item.status === "active" && total && total > 0 && (
                <Progress value={pct} />
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
}
