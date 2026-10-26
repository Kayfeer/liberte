import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useMessageStore } from "@/stores/messageStore";
import type { ImportStats } from "@/lib/tauri";

type RemoteBackupInfo = { sizeBytes: number; syncedAt: string };

export default function RemoteBackupControls() {
  const [busy, setBusy] = useState<"upload" | "restore" | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const run = async (operation: "upload" | "restore") => {
    if (busy) return;
    setBusy(operation); setError(null); setNotice(null);
    try {
      if (operation === "upload") {
        const result = await invoke<RemoteBackupInfo>("upload_remote_backup");
        setNotice(`Sauvegarde chiffrée synchronisée (${Math.ceil(result.sizeBytes / 1024)} Ko), le ${new Date(result.syncedAt).toLocaleString("fr-FR")}.`);
      } else {
        const result = await invoke<ImportStats>("restore_remote_backup");
        await useMessageStore.getState().loadChannels();
        const channel = useMessageStore.getState().activeChannelId;
        if (channel) await useMessageStore.getState().loadMessages(channel);
        setNotice(`${result.channelsImported} canaux et ${result.messagesImported} messages restaurés depuis le serveur.`);
        const refreshError = useMessageStore.getState().error || (channel && useMessageStore.getState().errors[channel]);
        if (refreshError) setError(`Les données sont restaurées, mais l'affichage n'a pas pu être actualisé : ${refreshError}`);
      }
    } catch (cause) { setError(String(cause)); }
    finally { setBusy(null); }
  };
  return <section aria-label="Sauvegarde distante" aria-busy={busy !== null} className="border-t border-liberte-border pt-4 space-y-2">
    <h4 className="text-sm font-medium">Sauvegarde sur mon serveur</h4>
    <p className="text-xs text-liberte-muted">Votre historique est chiffré avec votre identité avant l'envoi au serveur configuré. La restauration nécessite la même identité : conservez aussi votre export de profil portable. Les fichiers joints ne sont pas inclus.</p>
    <p className="text-xs text-liberte-muted">La synchronisation remplace la sauvegarde distante précédente. La restauration fusionne les données sans écraser votre historique local. Le serveur peut demander un accès premium.</p>
    <div className="flex flex-wrap gap-2">
      <button className="btn-secondary text-sm" disabled={busy !== null} onClick={() => void run("upload")}>{busy === "upload" ? "Synchronisation..." : "Synchroniser maintenant"}</button>
      <button className="btn-secondary text-sm" disabled={busy !== null} onClick={() => void run("restore")}>{busy === "restore" ? "Restauration..." : "Restaurer depuis le serveur"}</button>
    </div>
    {error && <p role="alert" className="text-sm text-red-300">{error}</p>}
    {notice && <p role="status" className="text-sm text-liberte-success">{notice}</p>}
  </section>;
}
