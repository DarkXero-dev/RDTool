import { useEffect, useState } from "react";
import { check } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { Button } from "@/components/ui/button";
import { Download, X } from "lucide-react";

type Phase = "idle" | "available" | "downloading" | "ready" | "error";

export default function UpdateBanner() {
  const [phase, setPhase] = useState<Phase>("idle");
  const [version, setVersion] = useState("");
  const [progress, setProgress] = useState(0);
  const [dismissed, setDismissed] = useState(false);

  useEffect(() => {
    let cancelled = false;
    check()
      .then((update) => {
        if (cancelled || !update) return;
        setVersion(update.version);
        setPhase("available");
      })
      .catch(() => {});
    return () => { cancelled = true; };
  }, []);

  async function install() {
    setPhase("downloading");
    setProgress(0);
    try {
      const update = await check();
      if (!update) return;
      let downloaded = 0;
      let total = 0;
      await update.downloadAndInstall((event) => {
        if (event.event === "Started") {
          total = event.data.contentLength ?? 0;
        } else if (event.event === "Progress") {
          downloaded += event.data.chunkLength;
          if (total > 0) setProgress(Math.round((downloaded / total) * 100));
        } else if (event.event === "Finished") {
          setPhase("ready");
        }
      });
    } catch {
      setPhase("error");
    }
  }

  if (dismissed || phase === "idle") return null;

  return (
    <div className="flex items-center gap-3 px-4 py-2 bg-rd-green/10 border-b border-rd-green/30 text-sm shrink-0">
      <Download size={14} className="text-rd-green shrink-0" />

      {phase === "available" && (
        <>
          <span className="flex-1 text-foreground">
            Update <span className="font-semibold text-rd-green">v{version}</span> available
          </span>
          <Button size="sm" variant="rd" className="h-7 text-xs px-3" onClick={install}>
            Update Now
          </Button>
          <button onClick={() => setDismissed(true)} className="text-muted-foreground hover:text-foreground">
            <X size={14} />
          </button>
        </>
      )}

      {phase === "downloading" && (
        <span className="flex-1 text-muted-foreground">
          Downloading update{progress > 0 ? ` ${progress}%` : "..."}
        </span>
      )}

      {phase === "ready" && (
        <>
          <span className="flex-1 text-foreground">Update downloaded. Restart to apply.</span>
          <Button size="sm" variant="rd" className="h-7 text-xs px-3" onClick={relaunch}>
            Restart
          </Button>
        </>
      )}

      {phase === "error" && (
        <>
          <span className="flex-1 text-destructive-foreground">Update failed. Try again later.</span>
          <button onClick={() => setDismissed(true)} className="text-muted-foreground hover:text-foreground">
            <X size={14} />
          </button>
        </>
      )}
    </div>
  );
}
