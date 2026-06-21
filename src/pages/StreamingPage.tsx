import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Loader2, Copy, ExternalLink } from "lucide-react";

interface StreamInfo {
  apple?: Record<string, string>;
  dash?: Record<string, string>;
  liveMP4?: Record<string, string>;
  h264WebM?: Record<string, string>;
}

const FORMAT_LABELS: Record<string, string> = {
  apple: "HLS (Apple)",
  dash: "DASH",
  liveMP4: "MP4",
  h264WebM: "WebM (H264)",
};

export default function StreamingPage() {
  const [id, setId] = useState("");
  const [info, setInfo] = useState<StreamInfo | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");
  const [copied, setCopied] = useState<string | null>(null);

  async function handleFetch() {
    if (!id.trim()) return;
    setLoading(true);
    setError("");
    setInfo(null);
    try {
      const data = await invoke<StreamInfo>("get_stream_transcodes", { id: id.trim() });
      setInfo(data);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }

  async function copyUrl(url: string) {
    await navigator.clipboard.writeText(url);
    setCopied(url);
    setTimeout(() => setCopied(null), 1500);
  }

  const formats = info
    ? (Object.entries(info) as [string, Record<string, string>][]).filter(
        ([, v]) => v && Object.keys(v).length > 0
      )
    : [];

  return (
    <div className="flex flex-col gap-6 max-w-2xl">
      <div>
        <h1 className="text-xl font-semibold">Streaming</h1>
        <p className="text-sm text-muted-foreground mt-1">
          Generate streaming transcodes from a Real-Debrid download ID.
        </p>
      </div>

      <div className="flex gap-2">
        <Input
          placeholder="Real-Debrid download ID"
          value={id}
          onChange={(e) => setId(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && handleFetch()}
        />
        <Button variant="rd" onClick={handleFetch} disabled={loading || !id.trim()}>
          {loading ? <Loader2 size={14} className="animate-spin" /> : null}
          {loading ? "Fetching…" : "Get Streams"}
        </Button>
      </div>

      {error && (
        <p className="text-xs text-destructive-foreground bg-destructive/20 border border-destructive/30 rounded-md px-3 py-2">
          {error}
        </p>
      )}

      {formats.length > 0 && (
        <div className="flex flex-col gap-4">
          {formats.map(([format, qualities]) => (
            <div key={format} className="rounded-lg border border-border bg-card p-4 flex flex-col gap-2">
              <div className="text-sm font-semibold">
                {FORMAT_LABELS[format] ?? format}
              </div>
              <div className="flex flex-col gap-1.5">
                {Object.entries(qualities).map(([quality, url]) => (
                  <div key={quality} className="flex items-center gap-2">
                    <span className="text-xs text-muted-foreground w-16 shrink-0">{quality}</span>
                    <span className="text-xs font-mono text-muted-foreground flex-1 truncate">{url}</span>
                    <Button
                      variant="ghost"
                      size="icon"
                      className="h-7 w-7 shrink-0"
                      onClick={() => copyUrl(url)}
                    >
                      <Copy size={12} className={copied === url ? "text-rd-green" : ""} />
                    </Button>
                    <Button variant="ghost" size="icon" className="h-7 w-7 shrink-0" asChild>
                      <a href={url} target="_blank" rel="noreferrer">
                        <ExternalLink size={12} />
                      </a>
                    </Button>
                  </div>
                ))}
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
