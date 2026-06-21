import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Loader2, Copy, Play } from "lucide-react";
import Hls from "hls.js";

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

function VideoPlayer({ src }: { src: string }) {
  const videoRef = useRef<HTMLVideoElement>(null);
  const hlsRef = useRef<Hls | null>(null);

  useEffect(() => {
    const video = videoRef.current;
    if (!video) return;

    if (hlsRef.current) {
      hlsRef.current.destroy();
      hlsRef.current = null;
    }

    const isHls = src.includes(".m3u8") || src.includes("/hls/");

    if (isHls && Hls.isSupported()) {
      const hls = new Hls();
      hlsRef.current = hls;
      hls.loadSource(src);
      hls.attachMedia(video);
    } else {
      video.src = src;
    }

    return () => {
      hlsRef.current?.destroy();
      hlsRef.current = null;
    };
  }, [src]);

  return (
    <div className="rounded-lg overflow-hidden bg-black border border-border">
      <video
        ref={videoRef}
        controls
        autoPlay
        className="w-full max-h-[420px]"
      />
    </div>
  );
}

export default function StreamingPage() {
  const [id, setId] = useState("");
  const [info, setInfo] = useState<StreamInfo | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");
  const [copied, setCopied] = useState<string | null>(null);
  const [playerUrl, setPlayerUrl] = useState<string | null>(null);

  async function handleFetch() {
    if (!id.trim()) return;
    setLoading(true);
    setError("");
    setInfo(null);
    setPlayerUrl(null);
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
    <div className="flex flex-col gap-6 max-w-3xl">
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
          {loading ? "Fetching..." : "Get Streams"}
        </Button>
      </div>

      {error && (
        <p className="text-xs text-destructive-foreground bg-destructive/20 border border-destructive/30 rounded-md px-3 py-2">
          {error}
        </p>
      )}

      {playerUrl && (
        <div className="flex flex-col gap-2">
          <div className="flex items-center justify-between">
            <span className="text-sm font-medium">Player</span>
            <button
              onClick={() => setPlayerUrl(null)}
              className="text-xs text-muted-foreground hover:text-foreground"
            >
              close
            </button>
          </div>
          <VideoPlayer src={playerUrl} />
        </div>
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
                      title="Play in player"
                      onClick={() => setPlayerUrl(url)}
                    >
                      <Play size={12} className={playerUrl === url ? "text-rd-green" : ""} />
                    </Button>
                    <Button
                      variant="ghost"
                      size="icon"
                      className="h-7 w-7 shrink-0"
                      onClick={() => copyUrl(url)}
                    >
                      <Copy size={12} className={copied === url ? "text-rd-green" : ""} />
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
