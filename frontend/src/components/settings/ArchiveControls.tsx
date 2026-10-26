import { useState } from "react";
import { open, save } from "@tauri-apps/plugin-dialog";
import { readTextFile } from "@tauri-apps/plugin-fs";
import * as tauri from "@/lib/tauri";
import { useIdentityStore } from "@/stores/identityStore";
import { useMessageStore } from "@/stores/messageStore";
import { useBackupStore } from "@/stores/backupStore";
import RemoteBackupControls from "./RemoteBackupControls";
import LocalBackupControls from "./LocalBackupControls";

export function ArchiveExport({ kind }: { kind: "profile" | "backup" }) {
  const [password, setPassword] = useState("");
  const [confirmation, setConfirmation] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const valid = password.length >= 12 && password === confirmation;
  const exportFile = async () => {
    if (!valid) return;
    setBusy(true); setError(null); setNotice(null);
    try {
      const path = await save({ defaultPath: `liberte_${kind}_${new Date().toISOString().slice(0, 10)}.json`, filters: [{ name: "Sauvegarde Liberté", extensions: ["json", "liberte"] }] });
      if (!path) return;
      if (kind === "profile") await tauri.saveProfileToFile(path, password);
      else await tauri.saveBackupToFile(path, password);
      setPassword(""); setConfirmation("");
      setNotice("Export chiffré enregistré. Conservez votre phrase secrète séparément.");
    } catch (cause) { setError(String(cause)); }
    finally { setBusy(false); }
  };
  return <div className="space-y-2">
    <p className="text-sm font-medium">{kind === "profile" ? "Exporter mon identité et mes données" : "Exporter une sauvegarde portable"}</p>
    <p className="text-xs text-liberte-muted">Votre export est protégé par une phrase secrète d'au moins 12 caractères. Elle ne peut pas être récupérée.</p>
    <label className="block text-xs">Phrase secrète
      <input type="password" autoComplete="new-password" value={password} onChange={(event) => setPassword(event.target.value)} className="input-field mt-1" minLength={12} />
    </label>
    <label className="block text-xs">Confirmer la phrase secrète
      <input type="password" autoComplete="new-password" value={confirmation} onChange={(event) => setConfirmation(event.target.value)} className="input-field mt-1" />
    </label>
    {confirmation && confirmation !== password && <p className="text-xs text-red-300">Les phrases secrètes sont différentes.</p>}
    <button disabled={!valid || busy} onClick={() => void exportFile()} className="btn-secondary text-sm">{busy ? "Chiffrement..." : "Exporter vers un fichier"}</button>
    {error && <p role="alert" className="text-sm text-red-300">{error}</p>}
    {notice && <p role="status" className="text-sm text-liberte-success">{notice}</p>}
  </div>;
}

export function ArchiveImport({ kind }: { kind: "profile" | "backup" }) {
  const [password, setPassword] = useState("");
  const [allowLegacy, setAllowLegacy] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const importFile = async () => {
    setBusy(true); setError(null); setNotice(null);
    try {
      const path = await open({ multiple: false, title: kind === "profile" ? "Restaurer mon identité" : "Importer une sauvegarde", filters: [{ name: "Sauvegarde Liberté", extensions: ["json", "liberte"] }] });
      if (!path || typeof path !== "string") return;
      const json = await readTextFile(path);
      if (kind === "profile") {
        await tauri.importProfile(json, password || undefined, allowLegacy);
        await useIdentityStore.getState().loadIdentity();
        const identityError = useIdentityStore.getState().error;
        if (identityError) throw new Error(identityError);
      } else {
        const stats = await tauri.importBackup(json, password || undefined, allowLegacy);
        await useMessageStore.getState().loadChannels();
        const active = useMessageStore.getState().activeChannelId;
        if (active) await useMessageStore.getState().loadMessages(active);
        setNotice(`${stats.channelsImported} canaux et ${stats.messagesImported} messages importés.`);
        const refreshError = useMessageStore.getState().error || (active && useMessageStore.getState().errors[active]);
        if (refreshError) setError(`Les données sont importées, mais l'affichage n'a pas pu être actualisé : ${refreshError}`);
      }
      setPassword(""); setAllowLegacy(false);
    } catch (cause) { setError(String(cause)); }
    finally { setBusy(false); }
  };
  return <div className="space-y-2">
    <p className="text-sm font-medium">{kind === "profile" ? "J'ai déjà une identité" : "Restaurer une sauvegarde"}</p>
    <label className="block text-xs">Phrase secrète de l'export
      <input type="password" autoComplete="current-password" value={password} onChange={(event) => setPassword(event.target.value)} className="input-field mt-1" />
    </label>
    <label className="flex gap-2 text-xs text-liberte-muted items-start">
      <input type="checkbox" checked={allowLegacy} onChange={(event) => setAllowLegacy(event.target.checked)} />
      Autoriser explicitement l'import d'un ancien export non chiffré dont je connais la provenance.
    </label>
    <button disabled={busy} onClick={() => void importFile()} className="btn-secondary text-sm">{busy ? "Importation..." : "Choisir le fichier à importer"}</button>
    {error && <p role="alert" className="text-sm text-red-300">{error}</p>}
    {notice && <p role="status" className="text-sm text-liberte-success">{notice}</p>}
  </div>;
}

export function BackupSettings() {
  const backup = useBackupStore();
  return <section className="panel p-4 space-y-4" aria-label="Sauvegardes">
    <h3 className="font-medium">Sauvegardes</h3>
    <p className="text-xs text-liberte-muted">Les sauvegardes locales automatiques utilisent votre identité. Créez aussi un export protégé par phrase secrète pour changer d'appareil.</p>
    <label className="flex gap-2 text-sm"><input type="checkbox" checked={backup.autoBackupEnabled} onChange={(event) => backup.setAutoBackup(event.target.checked)} />Sauvegarde automatique</label>
    {backup.autoBackupEnabled && <label className="block text-xs">Intervalle en minutes
      <input type="number" min={5} max={120} value={backup.intervalMinutes} onChange={(event) => {
        const value = Number(event.target.value);
        if (Number.isFinite(value) && value >= 5 && value <= 120) backup.setInterval(value);
      }} className="input-field mt-1" />
    </label>}
    <button disabled={backup.isBackingUp} onClick={() => void backup.runAutoBackup()} className="btn-secondary text-sm">{backup.isBackingUp ? "Sauvegarde..." : "Sauvegarder localement maintenant"}</button>
    {backup.lastBackupTime && <p className="text-xs text-liberte-muted">Dernière sauvegarde : {new Date(backup.lastBackupTime).toLocaleString("fr-FR")}</p>}
    {backup.error && <p role="alert" className="text-sm text-red-300">{backup.error}</p>}
    <div className="border-t border-liberte-border pt-4"><LocalBackupControls /></div>
    <div className="border-t border-liberte-border pt-4"><ArchiveExport kind="backup" /></div>
    <div className="border-t border-liberte-border pt-4"><ArchiveImport kind="backup" /></div>
    <div className="border-t border-liberte-border pt-4"><RemoteBackupControls /></div>
  </section>;
}
