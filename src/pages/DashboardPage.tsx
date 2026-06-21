import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Badge } from "@/components/ui/badge";
import { Loader2, User, Star, Clock } from "lucide-react";

interface RdUser {
  id: number;
  username: string;
  email: string;
  points: number;
  account_type: string;
  premium: number;
  expiration?: string;
}

function formatExpiry(iso?: string): string {
  if (!iso) return "N/A";
  return new Date(iso).toLocaleDateString(undefined, {
    year: "numeric",
    month: "long",
    day: "numeric",
  });
}

function daysLeft(iso?: string): number | null {
  if (!iso) return null;
  const diff = new Date(iso).getTime() - Date.now();
  return Math.max(0, Math.floor(diff / 86400000));
}

export default function DashboardPage() {
  const [user, setUser] = useState<RdUser | null>(null);
  const [error, setError] = useState("");

  useEffect(() => {
    invoke<RdUser>("get_user")
      .then(setUser)
      .catch((e: unknown) => setError(String(e)));
  }, []);

  if (error) {
    return (
      <div className="text-destructive-foreground text-sm">
        Failed to load account: {error}
      </div>
    );
  }

  if (!user) {
    return (
      <div className="flex items-center gap-2 text-muted-foreground">
        <Loader2 size={16} className="animate-spin" />
        Loading account…
      </div>
    );
  }

  const days = daysLeft(user.expiration);
  const isPremium = user.account_type === "premium";

  return (
    <div className="flex flex-col gap-6 max-w-2xl">
      <div>
        <h1 className="text-xl font-semibold">Dashboard</h1>
        <p className="text-sm text-muted-foreground mt-1">Account overview</p>
      </div>

      <div className="grid grid-cols-2 gap-4">
        <div className="rounded-lg border border-border bg-card p-4 flex flex-col gap-2">
          <div className="flex items-center gap-2 text-muted-foreground text-xs font-medium uppercase tracking-wider">
            <User size={14} />
            Account
          </div>
          <div className="text-lg font-semibold">{user.username}</div>
          <div className="text-sm text-muted-foreground">{user.email}</div>
          <Badge variant={isPremium ? "success" : "secondary"} className="w-fit mt-1">
            {isPremium ? "Premium" : user.account_type}
          </Badge>
        </div>

        <div className="rounded-lg border border-border bg-card p-4 flex flex-col gap-2">
          <div className="flex items-center gap-2 text-muted-foreground text-xs font-medium uppercase tracking-wider">
            <Star size={14} />
            Points
          </div>
          <div className="text-3xl font-bold text-rd-green">
            {user.points.toLocaleString()}
          </div>
          <div className="text-xs text-muted-foreground">Fidelity points</div>
        </div>

        <div className="col-span-2 rounded-lg border border-border bg-card p-4 flex flex-col gap-2">
          <div className="flex items-center gap-2 text-muted-foreground text-xs font-medium uppercase tracking-wider">
            <Clock size={14} />
            Premium Expiry
          </div>
          <div className="text-lg font-semibold">{formatExpiry(user.expiration)}</div>
          {days !== null && (
            <div className={`text-sm ${days < 7 ? "text-destructive-foreground" : "text-muted-foreground"}`}>
              {days === 0 ? "Expires today" : `${days} day${days === 1 ? "" : "s"} remaining`}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
