import { useEffect, useState } from "react";
import { useNetworkStore } from "@/stores/networkStore";
import { getSettings, getServerInfo, updateSettings } from "@/lib/tauri";

export default function NetworkSettings({ onServerChanged }: { onServerChanged?: () => void }) {
  const network = useNetworkStore();
  const [address, setAddress] = useState("");
  const [serverUrl, setServerUrl] = useState("");
  const [notice, setNotice] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  useEffect(() => {
    let cancelled = false;
    void getSettings().then((settings) => {
      if (!cancelled) setServerUrl(String(settings.serverUrl || ""));
    }).catch((cause) => { if (!cancelled) setError(String(cause)); });
    void network.refreshNetwork();
    return () => { cancelled = true; };
  }, []);

  const connect = async () => {
    setError(null); setNotice(null);
    try {
      await network.connectPeer(address);
      setNotice("Connexion établie.");
      setAddress("");
    } catch (cause) { setError(String(cause)); }
  };
  const saveServer = async () => {
    setSaving(true); setError(null); setNotice(null);
    try {
      const url = serverUrl.trim().replace(/\/$/, "");
      if (url && !/^https?:\/\//i.test(url)) throw new Error("Saisissez une URL HTTP ou HTTPS.");
      const info = url ? await getServerInfo(url) : null;
      const settings = await getSettings();
      await updateSettings({ ...settings, serverUrl: url });
      setServerUrl(url);
      onServerChanged?.();
      setNotice(info ? `Serveur enregistré : ${info.name} (${info.version}).` : "Serveur retiré.");
    } catch (cause) { setError(String(cause)); }
    finally { setSaving(false); }
  };

  const addresses = [...new Set([...network.externalAddresses, ...network.listenAddresses])]
    .map((value) => value.includes("/p2p/") || !network.localPeerId ? value : `${value}/p2p/${network.localPeerId}`);
  return <section className="panel p-4 space-y-4" aria-label="Réseau">
    <h3 className="font-medium">Connexions et serveur</h3>
    <p className="text-sm text-liberte-muted">Les pairs du réseau local sont découverts automatiquement. Pour une connexion distante, échangez une adresse accessible ou utilisez votre relais.</p>
    <label className="block text-xs">Adresse d'un pair ou d'un relais
      <input value={address} onChange={(event) => setAddress(event.target.value)} className="input-field mt-1"
        placeholder="/ip4/192.168.1.10/udp/4001/quic-v1/p2p/..." />
    </label>
    <button onClick={() => void connect()} disabled={!address.trim() || network.loading} className="btn-primary text-sm">
      {network.loading ? "Connexion..." : "Se connecter"}
    </button>
    <div>
      <p className="text-xs text-liberte-muted mb-1">Vos adresses à partager</p>
      {addresses.length ? addresses.map((value) => <div className="flex gap-2 items-center mb-1" key={value}>
        <code className="text-xs break-all flex-1">{value}</code>
        <button className="btn-secondary text-xs" onClick={() => void navigator.clipboard.writeText(value).then(() => setNotice("Adresse copiée.")).catch((cause) => setError(String(cause)))}>Copier</button>
      </div>) : <p className="text-xs text-liberte-muted">Aucune adresse disponible.</p>}
      <p className="text-xs text-liberte-muted mt-2">Une adresse privée fonctionne sur le réseau local. Depuis Internet, une adresse publique accessible ou un relais est nécessaire.</p>
    </div>
    <label className="block text-xs">URL du serveur auto-hébergé
      <input type="url" value={serverUrl} onChange={(event) => setServerUrl(event.target.value)} className="input-field mt-1" placeholder="https://votre-serveur.example" />
    </label>
    <button disabled={saving} onClick={() => void saveServer()} className="btn-secondary text-sm">{saving ? "Vérification..." : "Vérifier et enregistrer le serveur"}</button>
    {(error || network.error) && <p role="alert" className="text-sm text-red-300">{error || network.error}</p>}
    {notice && <p role="status" className="text-sm text-liberte-success">{notice}</p>}
  </section>;
}
