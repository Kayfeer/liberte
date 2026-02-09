import { useEffect, useState } from "react";
import { check } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { Download, X, RefreshCw } from "lucide-react";

type UpdateStatus = "idle" | "checking" | "available" | "downloading" | "ready" | "error";

export default function UpdateBanner() {
  const [status, setStatus] = useState<UpdateStatus>("idle");
  const [version, setVersion] = useState("");
  const [progress, setProgress] = useState(0);
  const [error, setError] = useState("");
  const [dismissed, setDismissed] = useState(false);

  useEffect(() => {
    checkForUpdate();
  }, []);

  const checkForUpdate = async () => {
    setStatus("checking");
    try {
      const update = await check();
      if (update) {
        setVersion(update.version);
        setStatus("available");
      } else {
        setStatus("idle");
      }
    } catch (e) {
      // Silently ignore check errors (offline, no releases, etc.)
      console.debug("Update check:", e);
      setStatus("idle");
    }
  };

  const downloadAndInstall = async () => {
    setStatus("downloading");
    try {
      const update = await check();
      if (!update) return;

      let totalBytes = 0;
      let downloadedBytes = 0;

      await update.downloadAndInstall((event) => {
        if (event.event === "Started" && event.data.contentLength) {
          totalBytes = event.data.contentLength;
        } else if (event.event === "Progress") {
          downloadedBytes += event.data.chunkLength;
          if (totalBytes > 0) {
            setProgress(Math.round((downloadedBytes / totalBytes) * 100));
          }
        } else if (event.event === "Finished") {
          setStatus("ready");
        }
      });

      setStatus("ready");
    } catch (e) {
      setError(String(e));
      setStatus("error");
    }
  };

  const handleRelaunch = async () => {
    await relaunch();
  };

  if (dismissed || status === "idle" || status === "checking") return null;

  return (
    <div className="bg-liberte-accent/10 border-b border-liberte-accent/30 px-4 py-2 flex items-center gap-3 text-sm">
      {status === "available" && (
        <>
          <Download className="w-4 h-4 text-liberte-accent flex-shrink-0" />
          <span className="flex-1">
            Mise à jour <strong>v{version}</strong> disponible
          </span>
          <button
            onClick={downloadAndInstall}
            className="btn-primary text-xs py-1 px-3"
          >
            Installer
          </button>
          <button
            onClick={() => setDismissed(true)}
            className="p-1 hover:bg-liberte-panel rounded"
          >
            <X className="w-3.5 h-3.5 text-liberte-muted" />
          </button>
        </>
      )}

      {status === "downloading" && (
        <>
          <RefreshCw className="w-4 h-4 text-liberte-accent animate-spin flex-shrink-0" />
          <span className="flex-1">
            Téléchargement de la mise à jour... {progress}%
          </span>
          <div className="w-32 h-1.5 bg-liberte-border rounded-full overflow-hidden">
            <div
              className="h-full bg-liberte-accent transition-all duration-300"
              style={{ width: `${progress}%` }}
            />
          </div>
        </>
      )}

      {status === "ready" && (
        <>
          <Download className="w-4 h-4 text-green-400 flex-shrink-0" />
          <span className="flex-1">
            Mise à jour prête — redémarrage nécessaire
          </span>
          <button
            onClick={handleRelaunch}
            className="btn-primary text-xs py-1 px-3"
          >
            Redémarrer
          </button>
        </>
      )}

      {status === "error" && (
        <>
          <span className="flex-1 text-red-400">
            Erreur de mise à jour : {error}
          </span>
          <button
            onClick={() => setDismissed(true)}
            className="p-1 hover:bg-liberte-panel rounded"
          >
            <X className="w-3.5 h-3.5 text-liberte-muted" />
          </button>
        </>
      )}
    </div>
  );
}
