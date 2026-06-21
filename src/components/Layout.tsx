import { NavLink, Outlet } from "react-router-dom";
import { invoke } from "@tauri-apps/api/core";
import {
  LayoutDashboard,
  Download,
  Magnet,
  Play,
  ArrowDownToLine,
  Settings,
  LogOut,
} from "lucide-react";
import { cn } from "@/lib/utils";

const NAV = [
  { to: "/", icon: LayoutDashboard, label: "Dashboard", end: true },
  { to: "/downloader", icon: Download, label: "Downloader" },
  { to: "/torrents", icon: Magnet, label: "Torrents" },
  { to: "/streaming", icon: Play, label: "Streaming" },
  { to: "/downloads", icon: ArrowDownToLine, label: "Downloads" },
  { to: "/settings", icon: Settings, label: "Settings" },
];

interface Props {
  onLogout: () => void;
}

export default function Layout({ onLogout }: Props) {
  async function handleLogout() {
    await invoke("clear_token").catch(() => {});
    onLogout();
  }

  return (
    <div className="flex h-screen bg-background text-foreground overflow-hidden">
      <aside className="flex flex-col w-16 border-r border-border bg-card shrink-0">
        <div className="flex items-center justify-center h-14 border-b border-border">
          <span className="text-rd-green font-bold text-lg">RD</span>
        </div>
        <nav className="flex flex-col flex-1 gap-1 p-2 pt-3">
          {NAV.map(({ to, icon: Icon, label, end }) => (
            <NavLink
              key={to}
              to={to}
              end={end}
              title={label}
              className={({ isActive }) =>
                cn(
                  "flex flex-col items-center justify-center gap-1 rounded-md p-2 text-[10px] transition-colors",
                  isActive
                    ? "bg-secondary text-foreground"
                    : "text-muted-foreground hover:bg-secondary/50 hover:text-foreground"
                )
              }
            >
              <Icon size={18} />
              <span>{label.slice(0, 5)}</span>
            </NavLink>
          ))}
        </nav>
        <div className="p-2 pb-4">
          <button
            onClick={handleLogout}
            title="Logout"
            className="flex flex-col items-center justify-center gap-1 w-full rounded-md p-2 text-[10px] text-muted-foreground hover:bg-secondary/50 hover:text-foreground transition-colors"
          >
            <LogOut size={18} />
            <span>Logout</span>
          </button>
        </div>
      </aside>
      <main className="flex-1 overflow-y-auto p-6">
        <Outlet />
      </main>
    </div>
  );
}
