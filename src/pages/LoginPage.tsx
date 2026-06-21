import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Loader2 } from "lucide-react";

interface Props {
  onLogin: () => void;
}

export default function LoginPage({ onLogin }: Props) {
  const [token, setToken] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");

  async function handleConnect() {
    if (!token.trim()) return;
    setLoading(true);
    setError("");
    try {
      await invoke("save_token", { token: token.trim() });
      await invoke("get_user");
      onLogin();
    } catch (e) {
      await invoke("clear_token").catch(() => {});
      setError("Invalid token or connection failed. Check your API token and try again.");
    } finally {
      setLoading(false);
    }
  }

  return (
    <div className="min-h-screen bg-background flex items-center justify-center">
      <div className="w-full max-w-sm flex flex-col items-center gap-8">
        <div className="flex flex-col items-center gap-4">
          <div className="flex items-center gap-2">
            <div className="w-12 h-12 rounded-xl bg-rd-green flex items-center justify-center">
              <span className="text-black font-black text-xl">RD</span>
            </div>
          </div>
          <div className="text-center">
            <h1 className="text-2xl font-bold text-foreground">RDTool</h1>
            <p className="mt-2 text-sm text-muted-foreground leading-relaxed max-w-xs">
              Desktop client for Real-Debrid. Convert restricted links and magnet
              files into fast direct downloads with a built-in queue and scheduler.
            </p>
          </div>
        </div>

        <div className="w-full flex flex-col gap-3">
          <div className="flex flex-col gap-1.5">
            <label className="text-sm font-medium text-foreground">API Token</label>
            <Input
              type="password"
              placeholder="Paste your Real-Debrid API token"
              value={token}
              onChange={(e) => setToken(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && handleConnect()}
              autoFocus
            />
            <a
              href="https://real-debrid.com/apitoken"
              target="_blank"
              rel="noreferrer"
              className="text-xs text-rd-green hover:underline self-end"
            >
              Get your token →
            </a>
          </div>

          {error && (
            <p className="text-xs text-destructive-foreground bg-destructive/20 border border-destructive/30 rounded-md px-3 py-2">
              {error}
            </p>
          )}

          <Button
            variant="rd"
            className="w-full"
            onClick={handleConnect}
            disabled={loading || !token.trim()}
          >
            {loading ? <Loader2 size={16} className="animate-spin" /> : null}
            {loading ? "Connecting..." : "Connect"}
          </Button>
        </div>
      </div>
    </div>
  );
}
