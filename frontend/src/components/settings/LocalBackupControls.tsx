import { useEffect, useState } from "react";
import { useBackupStore } from "@/stores/backupStore";
import { useMessageStore } from "@/stores/messageStore";
import { restoreLocalBackup, type BackupFileInfo } from "@/lib/tauri";

export default function LocalBackupControls() {
  const { backups, lastBackupPath, lastBackupTime, isRestoring, loadBackupList, error: backupError } = useBackupStore();
  const [selected, setSelected] = useState<BackupFileInfo | null>(null);
  const [restoring, setRestoring] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  useEffect(() => { void loadBackupList(); }, [loadBackupList, lastBackupTime]);

  const restore = async () => {
    if (!selected || restoring || isRestoring) return;
    setRestoring(true); setError(null); setNotice(null);
    try {
      const stats = await restoreLocalBackup(selected.fileName);
      setSelected(null);
      await useMessageStore.getState().loadChannels();
      let refreshError = useMessageStore.getState().error;
      const active = useMessageStore.getState().activeChannelId;
      if (!refreshError && active) {
        await useMessageStore.getState().loadMessages(active);
        refreshError = useMessageStore.getState().errors[active] || null;
      }
      setNotice(`${stats.channelsImported} canaux et ${stats.messagesImported} messages restaurés.`);
      if (refreshError) setError(`Les données sont restaurées, mais l'affichage n'a pas pu être actualisé : ${refreshError}`);
    } catch (cause) { setError(String(cause)); }
    finally { setRestoring(false); }
  };

  return <div className="space-y-3">
    <h4 className="text-sm font-medium">Sauvegardes locales disponibles</h4>
    {lastBackupPath && <p className="text-xs text-liberte-muted break-all">Dernier fichier créé : <code>{lastBackupPath}</code></p>}
    <button className="btn-secondary text-sm" disabled={restoring} onClick={() => void loadBackupList()}>Actualiser la liste</button>
    {!backupError && backups.length === 0 && <p className="text-xs text-liberte-muted">Aucune sauvegarde locale disponible.</p>}
    <ul className="space-y-3">
      {backups.map((file) => <li key={file.filePath} className="rounded border border-liberte-border p-3 space-y-2">
        <p className="text-xs font-medium break-all">{file.fileName}</p>
        <p className="text-xs text-liberte-muted">{new Date(file.modified).toLocaleString("fr-FR")} · {Math.ceil(file.sizeBytes / 1024)} Ko</p>
        <code className="block text-xs text-liberte-muted break-all">{file.filePath}</code>
        <button className="btn-secondary text-xs" disabled={restoring || isRestoring} onClick={() => { setSelected(file); setError(null); setNotice(null); }}>Restaurer cette sauvegarde</button>
      </li>)}
    </ul>
    {selected && <div className="rounded border border-liberte-border p-3 space-y-2" role="group" aria-label="Confirmer la restauration locale">
      <p className="text-sm break-all">Restaurer {selected.fileName} avec votre identité actuelle ? Les données importées seront ajoutées aux données locales.</p>
      <div className="flex gap-2">
        <button className="btn-primary text-sm" disabled={restoring || isRestoring} onClick={() => void restore()}>{restoring ? "Restauration..." : "Confirmer la restauration"}</button>
        <button className="btn-secondary text-sm" disabled={restoring} onClick={() => setSelected(null)}>Annuler</button>
      </div>
    </div>}
    {error && <p role="alert" className="text-sm text-red-300">{error}</p>}
    {notice && <p role="status" className="text-sm text-liberte-success">{notice}</p>}
  </div>;
}
