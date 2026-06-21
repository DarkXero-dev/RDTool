import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
import { Badge } from "@/components/ui/badge";
import { Loader2, Copy, Download, FileText, Plus } from "lucide-react";
import { cn } from "@/lib/utils";

interface UnrestrictedLink {
  id: string;
  filename: string;
  filesize: number;
  link: string;
  host: string;
  download: string;
  streamable?: number;
}

function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 ** 2) return `${(n / 1024).toFixed(1)} KB`;
  if (n < 1024 ** 3) return `${(n / 1024 ** 2).toFixed(1)} MB`;
  return `${(n / 1024 ** 3).toFixed(2)} GB`;
}

export default function DownloaderPage() {
  const [raw, setRaw] = useState("");
  const [results, setResults] = useState<UnrestrictedLink[]>([]);
  const [loading, setLoading] = useState(false);
  const [copied, setCopied] = useState<string | null>(null);

  async function handleUnrestrict() {
    const links = raw
      .split(/\n/)
      .map((l) => l.trim())
      .filter(Boolean);
    if (!links.length) return;
    setLoading(true);
    try {
      const data = await invoke<UnrestrictedLink[]>("unrestrict_links", { links });
      setResults(data);
    } catch (e) {
      console.error(e);
    } finally {
      setLoading(false);
    }
  }

  async function copyLink(url: string) {
    await navigator.clipboard.writeText(url);
    setCopied(url);
    setTimeout(() => setCopied(null), 1500);
  }

  async function exportTxt() {
    const urls = results.map((r) => r.download);
    const path = await save({ filters: [{ name: "Text", extensions: ["txt"] }] });
    if (!path) return;
    await invoke("export_links_to_txt", { links: urls, path });
  }

  async function addToQueue(r: UnrestrictedLink) {
    await invoke("enqueue_download", {
      url: r.download,
      filename: r.filename,
      opts: { threads: null, scheduled_at: null, priority: 0 },
    });
  }

  return (
    <div className="flex flex-col gap-6 max-w-3xl">
      <div>
        <h1 className="text-xl font-semibold">Unrestricted Downloader</h1>
        <p className="text-sm text-muted-foreground mt-1">
          Paste one link per line, get direct download URLs.
        </p>
      </div>

      <div className="flex flex-col gap-3">
        <Textarea
          placeholder="https://rapidgator.net/file/...&#10;https://mega.nz/file/..."
          className="min-h-[120px] font-mono text-xs"
          value={raw}
          onChange={(e) => setRaw(e.target.value)}
        />
        <Button variant="rd" onClick={handleUnrestrict} disabled={loading || !raw.trim()}>
          {loading ? <Loader2 size={14} className="animate-spin" /> : null}
          {loading ? "Unrestricting…" : "Unrestrict"}
        </Button>
      </div>

      {results.length > 0 && (
        <div className="flex flex-col gap-3">
          <div className="flex items-center justify-between">
            <span className="text-sm font-medium">
              {results.length} link{results.length > 1 ? "s" : ""} generated
            </span>
            <Button variant="outline" size="sm" onClick={exportTxt}>
              <FileText size={14} />
              Export .txt
            </Button>
          </div>

          <div className="flex flex-col gap-2">
            {results.map((r) => (
              <div
                key={r.id}
                className="rounded-lg border border-border bg-card p-3 flex items-start gap-3"
              >
                <div className="flex-1 min-w-0">
                  <div className="font-medium text-sm truncate">{r.filename}</div>
                  <div className="flex items-center gap-2 mt-1">
                    <span className="text-xs text-muted-foreground">{r.host}</span>
                    <span className="text-xs text-muted-foreground">{formatBytes(r.filesize)}</span>
                    {r.streamable === 1 && (
                      <Badge variant="secondary" className="text-[10px] px-1.5 py-0">
                        Streamable
                      </Badge>
                    )}
                  </div>
                  <div className="text-xs text-muted-foreground font-mono mt-1 truncate">
                    {r.download}
                  </div>
                </div>
                <div className="flex items-center gap-1 shrink-0">
                  <Button
                    variant="ghost"
                    size="icon"
                    title="Copy link"
                    onClick={() => copyLink(r.download)}
                    className={cn(copied === r.download && "text-rd-green")}
                  >
                    <Copy size={14} />
                  </Button>
                  <Button
                    variant="ghost"
                    size="icon"
                    title="Add to download queue"
                    onClick={() => addToQueue(r)}
                  >
                    <Plus size={14} />
                  </Button>
                  <Button variant="ghost" size="icon" title="Open in browser" asChild>
                    <a href={r.download} target="_blank" rel="noreferrer">
                      <Download size={14} />
                    </a>
                  </Button>
                </div>
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
