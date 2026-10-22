import { useEffect, useState } from "react";
import { save } from "@tauri-apps/plugin-dialog";
import { Download, File, RefreshCw, X } from "lucide-react";
import { acceptFile, dismissFile, saveReceivedFile, type FileTransferDto } from "@/lib/tauri";
import { useFileStore } from "@/stores/fileStore";
import { useIdentityStore } from "@/stores/identityStore";

const labels: Record<FileTransferDto["status"], string> = {
  offered: "Fichier proposé", receiving: "Réception", received: "Prêt à enregistrer",
  sending: "Envoi", completed: "Transfert terminé", failed: "Échec du transfert",
};

export default function FileTransfers({ channelId }: { channelId: string }) {
  const transfers = useFileStore((state) => state.transfers);
  const loadError = useFileStore((state) => state.errors[channelId]);
  const ownId = useIdentityStore((state) => state.identity?.publicKey);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState<Record<string, boolean>>({});
  const files = Object.values(transfers).filter((file) => file.channelId === channelId);

  useEffect(() => { void useFileStore.getState().load(channelId); }, [channelId]);
  const act = async (file: FileTransferDto, action: "accept" | "save" | "dismiss") => {
    setError(null);
    setBusy((state) => ({ ...state, [file.fileId]: true }));
    try {
      if (action === "accept") await acceptFile(file.fileId);
      else if (action === "dismiss") {
        await dismissFile(file.fileId);
        useFileStore.getState().remove(file.fileId);
      } else {
        const safeName = file.fileName.split(/[/\\]/).pop()?.replace(/[\x00-\x1f]/g, "") || "fichier";
        const path = await save({ defaultPath: safeName, title: "Enregistrer le fichier reçu" });
        if (path) await saveReceivedFile(file.fileId, path);
      }
      await useFileStore.getState().load(channelId);
    } catch (cause) { setError(String(cause)); }
    finally { setBusy((state) => ({ ...state, [file.fileId]: false })); }
  };

  if (!files.length && !error && !loadError) return null;
  return (
    <section aria-label="Fichiers du canal" className="max-h-48 overflow-y-auto mx-4 mb-2 border border-liberte-border rounded p-2 text-xs">
      {(error || loadError) && <p role="alert" className="text-red-300 mb-2">{error || loadError}</p>}
      {files.map((file) => <div key={file.fileId} className="flex items-center gap-2 py-2 border-b last:border-b-0 border-liberte-border">
        <File className="w-4 h-4 shrink-0" />
        <div className="min-w-0 flex-1">
          <p className="truncate font-medium">{file.fileName}</p>
          <p className="text-liberte-muted">{(file.fileSize / 1024).toFixed(1)} Ko · {labels[file.status]}</p>
          {file.error && <p role="alert" className="text-red-300">{file.error}</p>}
          {(file.status === "sending" || file.status === "receiving") &&
            <progress aria-label={labels[file.status]} max={100} value={Math.max(0, Math.min(100, file.progress))} className="w-full" />}
        </div>
        {file.senderId !== ownId && (file.status === "offered" || file.status === "failed") &&
          <button disabled={busy[file.fileId]} onClick={() => void act(file, "accept")} className="btn-secondary flex items-center gap-1">
            <RefreshCw className="w-3 h-3" />{file.status === "failed" ? "Réessayer" : "Recevoir"}
          </button>}
        {file.senderId !== ownId && (file.status === "received" || file.status === "completed") &&
          <button disabled={busy[file.fileId]} onClick={() => void act(file, "save")} className="btn-secondary flex items-center gap-1">
            <Download className="w-3 h-3" />Enregistrer
          </button>}
        <button disabled={busy[file.fileId]} onClick={() => void act(file, "dismiss")}
          aria-label={`Retirer ${file.fileName}`} title="Retirer et libérer la copie temporaire"
          className="p-1 text-liberte-muted hover:text-red-300">
          <X className="w-4 h-4" />
        </button>
      </div>)}
    </section>
  );
}
