import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { readFile } from "@tauri-apps/plugin-fs";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Progress } from "@/components/ui/progress";
import { Badge } from "@/components/ui/badge";
import { Loader2, Magnet, Upload, Trash2, Plus } from "lucide-react";

interface Torrent {
  id: string;
  filename: string;
  bytes: number;
  links: string[];
  status: string;
  progress: number;
  seeders?: number;
  speed?: number;
  added: string;
}

function formatBytes(n: number): string {
  if (n < 1024 ** 2) return `${(n / 1024).toFixed(1)} KB`;
  if (n < 1024 ** 3) return `${(n / 1024 ** 2).toFixed(1)} MB`;
  return `${(n / 1024 ** 3).toFixed(2)} GB`;
}

function statusColor(s: string): "default" | "success" | "secondary" | "destructive" {
  if (s === "downloaded") return "success";
  if (s === "downloading") return "default";
  if (s === "error") return "destructive";
  return "secondary";
}

export default function TorrentsPage() {
  const [torrents, setTorrents] = useState<Torrent[]>([]);
  const [magnet, setMagnet] = useState("");
  const [loading, setLoading] = useState(false);
  const [refreshing, setRefreshing] = useState(false);
  const pollRef = useRef<ReturnType<typeof setInterval> | null>(null);

  async function refresh() {
    setRefreshing(true);
    const data = await invoke<Torrent[]>("get_torrents").catch(() => []);
    setTorrents(data);
    setRefreshing(false);
  }

  useEffect(() => {
    refresh();
    pollRef.current = setInterval(refresh, 5000);
    return () => { if (pollRef.current) clearInterval(pollRef.current); };
  }, []);

  async function handleAddMagnet() {
    if (!magnet.trim()) return;
    setLoading(true);
    try {
      const result = await invoke<{ id: string }>("add_magnet", { magnet: magnet.trim() });
      await invoke("select_torrent_files", { id: result.id, fileIds: [] });
      setMagnet("");
      await refresh();
    } catch (e) {
      console.error(e);
    } finally {
      setLoading(false);
    }
  }

  async function handleAddFile() {
    const path = await open({ filters: [{ name: "Torrent", extensions: ["torrent"] }] });
    if (!path || typeof path !== "string") return;
    setLoading(true);
    try {
      const bytes = Array.from(await readFile(path));
      const filename = path.split("/").pop() ?? "file.torrent";
      const result = await invoke<{ id: string }>("add_torrent_file", { bytes, filename });
      await invoke("select_torrent_files", { id: result.id, fileIds: [] });
      await refresh();
    } catch (e) {
      console.error(e);
    } finally {
      setLoading(false);
    }
  }

  async function handleDelete(id: string) {
    await invoke("delete_torrent", { id }).catch(console.error);
    setTorrents((t) => t.filter((x) => x.id !== id));
  }

  async function addLinksToQueue(t: Torrent) {
    for (const link of t.links) {
      await invoke("enqueue_download", {
        url: link,
        filename: t.filename,
        opts: { threads: null, scheduled_at: null, priority: 0 },
      }).catch(console.error);
    }
  }

  return (
    <div className="flex flex-col gap-6 max-w-3xl">
      <div>
        <h1 className="text-xl font-semibold">Torrents</h1>
        <p className="text-sm text-muted-foreground mt-1">
          Add magnet links or .torrent files to Real-Debrid.
        </p>
      </div>

      <div className="flex gap-2">
        <Input
          placeholder="magnet:?xt=urn:btih:..."
          value={magnet}
          onChange={(e) => setMagnet(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && handleAddMagnet()}
          className="font-mono text-xs"
        />
        <Button variant="rd" onClick={handleAddMagnet} disabled={loading || !magnet.trim()}>
          {loading ? <Loader2 size={14} className="animate-spin" /> : <Magnet size={14} />}
          Add Magnet
        </Button>
        <Button variant="outline" onClick={handleAddFile} disabled={loading}>
          <Upload size={14} />
          Add File
        </Button>
      </div>

      <div className="flex flex-col gap-2">
        <div className="flex items-center justify-between">
          <span className="text-sm font-medium">{torrents.length} torrent{torrents.length !== 1 ? "s" : ""}</span>
          {refreshing && <Loader2 size={14} className="animate-spin text-muted-foreground" />}
        </div>

        {torrents.map((t) => (
          <div key={t.id} className="rounded-lg border border-border bg-card p-4 flex flex-col gap-2">
            <div className="flex items-start justify-between gap-2">
              <div className="flex-1 min-w-0">
                <div className="font-medium text-sm truncate">{t.filename}</div>
                <div className="flex items-center gap-2 mt-1">
                  <Badge variant={statusColor(t.status)} className="text-[10px] px-1.5 py-0">
                    {t.status}
                  </Badge>
                  <span className="text-xs text-muted-foreground">{formatBytes(t.bytes)}</span>
                  {t.seeders !== undefined && (
                    <span className="text-xs text-muted-foreground">{t.seeders} seeders</span>
                  )}
                  {t.speed !== undefined && t.speed > 0 && (
                    <span className="text-xs text-muted-foreground">
                      {formatBytes(t.speed)}/s
                    </span>
                  )}
                </div>
              </div>
              <div className="flex items-center gap-1 shrink-0">
                {t.status === "downloaded" && t.links.length > 0 && (
                  <Button variant="outline" size="sm" onClick={() => addLinksToQueue(t)}>
                    <Plus size={12} />
                    Queue
                  </Button>
                )}
                <Button
                  variant="ghost"
                  size="icon"
                  onClick={() => handleDelete(t.id)}
                  className="text-muted-foreground hover:text-destructive-foreground"
                >
                  <Trash2 size={14} />
                </Button>
              </div>
            </div>
            {t.status === "downloading" && (
              <Progress value={t.progress} />
            )}
            <div className="text-xs text-muted-foreground">
              {t.links.length} link{t.links.length !== 1 ? "s" : ""} available
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
