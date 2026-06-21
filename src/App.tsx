import { useEffect, useState } from "react";
import { HashRouter, Navigate, Route, Routes } from "react-router-dom";
import { invoke } from "@tauri-apps/api/core";

import Layout from "@/components/Layout";
import LoginPage from "@/pages/LoginPage";
import DashboardPage from "@/pages/DashboardPage";
import DownloaderPage from "@/pages/DownloaderPage";
import TorrentsPage from "@/pages/TorrentsPage";
import StreamingPage from "@/pages/StreamingPage";
import DownloadsPage from "@/pages/DownloadsPage";
import SettingsPage from "@/pages/SettingsPage";

function App() {
  const [authed, setAuthed] = useState<boolean | null>(null);

  useEffect(() => {
    invoke("load_token")
      .then(() => setAuthed(true))
      .catch(() => setAuthed(false));
  }, []);

  if (authed === null) return null;

  return (
    <HashRouter>
      <Routes>
        <Route
          path="/login"
          element={
            authed ? (
              <Navigate to="/" replace />
            ) : (
              <LoginPage onLogin={() => setAuthed(true)} />
            )
          }
        />
        {authed ? (
          <Route element={<Layout onLogout={() => setAuthed(false)} />}>
            <Route path="/" element={<DashboardPage />} />
            <Route path="/downloader" element={<DownloaderPage />} />
            <Route path="/torrents" element={<TorrentsPage />} />
            <Route path="/streaming" element={<StreamingPage />} />
            <Route path="/downloads" element={<DownloadsPage />} />
            <Route path="/settings" element={<SettingsPage />} />
          </Route>
        ) : (
          <Route path="*" element={<Navigate to="/login" replace />} />
        )}
      </Routes>
    </HashRouter>
  );
}

export default App;
