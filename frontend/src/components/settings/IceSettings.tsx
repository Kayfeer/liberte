import { useEffect, useState } from "react";

export default function IceSettings({ servers, onSave }: { servers: RTCIceServer[]; onSave: (servers: RTCIceServer[]) => void }) {
  const [value, setValue] = useState(JSON.stringify(servers, null, 2));
  const [error, setError] = useState<string | null>(null);
  const [saved, setSaved] = useState(false);
  useEffect(() => { setValue(JSON.stringify(servers, null, 2)); }, [servers]);
  const save = () => {
    try {
      const parsed: unknown = JSON.parse(value);
      if (!Array.isArray(parsed)) throw new Error("La configuration doit être un tableau JSON.");
      for (const server of parsed) {
        if (!server || typeof server !== "object") throw new Error("Serveur ICE invalide.");
        const urls = Array.isArray(server.urls) ? server.urls : [server.urls];
        if (!urls.length || urls.some((url: unknown) => typeof url !== "string" || !/^(stun|stuns|turn|turns):[^\s]+$/.test(url))) {
          throw new Error("Chaque serveur doit fournir urls avec une adresse stun:, stuns:, turn: ou turns:.");
        }
        if (server.username !== undefined && typeof server.username !== "string") throw new Error("username doit être du texte.");
        if (server.credential !== undefined && typeof server.credential !== "string") throw new Error("credential doit être du texte.");
      }
      onSave(parsed as RTCIceServer[]);
      setError(null); setSaved(true);
    } catch (cause) { setError(String(cause)); setSaved(false); }
  };
  return <section className="panel p-4 space-y-3">
    <h3 className="font-medium">Appels entre réseaux différents</h3>
    <p className="text-xs text-liberte-muted">Configurez vos serveurs STUN/TURN si les participants sont derrière un routeur. Aucun fournisseur tiers n'est ajouté par défaut. Une modification s'applique aux nouvelles connexions.</p>
    <label className="block text-xs">Serveurs ICE (JSON)
      <textarea rows={5} value={value} onChange={(event) => { setValue(event.target.value); setSaved(false); }} className="input-field mt-1 font-mono text-xs"
        placeholder={'[{"urls":"turn:relay.example:3478","username":"utilisateur","credential":"secret"}]'} spellCheck={false} />
    </label>
    <p className="text-xs text-liberte-muted">Les identifiants TURN sont conservés sur cet appareil avec vos préférences.</p>
    <button onClick={save} className="btn-secondary text-sm">Enregistrer la configuration</button>
    {error && <p role="alert" className="text-sm text-red-300">{error}</p>}
    {saved && <p role="status" className="text-sm text-liberte-success">Configuration enregistrée.</p>}
  </section>;
}
