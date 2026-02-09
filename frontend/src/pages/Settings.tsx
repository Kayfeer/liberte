import { useIdentityStore } from "../stores/identityStore";
import { Shield, Copy, Check, Server, Wifi } from "lucide-react";
import { useState } from "react";
import { getServerInfo, updateSettings, getSettings } from "../lib/tauri";
import type { ServerInfo } from "../lib/types";

export default function Settings() {
  const { identity } = useIdentityStore();
  const [copied, setCopied] = useState(false);
  const [serverUrl, setServerUrl] = useState("");
  const [serverInfo, setServerInfo] = useState<ServerInfo | null>(null);
  const [serverError, setServerError] = useState("");
  const [serverLoading, setServerLoading] = useState(false);

  const copyPubkey = () => {
    if (identity) {
      navigator.clipboard.writeText(identity.publicKey);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    }
  };

  const testServer = async () => {
    if (!serverUrl.trim()) return;
    setServerLoading(true);
    setServerError("");
    setServerInfo(null);
    try {
      const info = await getServerInfo(serverUrl.trim());
      setServerInfo(info);
    } catch (e) {
      setServerError(String(e));
    } finally {
      setServerLoading(false);
    }
  };

  const saveServer = async () => {
    try {
      const current = await getSettings();
      await updateSettings({ ...current, serverUrl: serverUrl.trim() });
    } catch (e) {
      setServerError(String(e));
    }
  };

  return (
    <div className="p-6 max-w-2xl mx-auto space-y-6">
      <h2 className="text-xl font-bold">Paramètres</h2>

      {/* Server / Self-hosted */}
      <div className="panel p-4 space-y-4">
        <h3 className="font-medium flex items-center gap-2">
          <Server className="w-4 h-4 text-liberte-accent" />
          Serveur relais
        </h3>
        <p className="text-sm text-liberte-muted">
          Connectez-vous à un serveur Liberté géré ou auto-hébergé.
          Laissez vide pour le mode P2P pur.
        </p>

        <div className="flex gap-2">
          <input
            type="text"
            placeholder="https://liberte.example.com"
            value={serverUrl}
            onChange={(e) => setServerUrl(e.target.value)}
            className="flex-1 bg-liberte-bg border border-liberte-panel rounded px-3 py-2 text-sm focus:outline-none focus:border-liberte-accent"
          />
          <button
            onClick={testServer}
            disabled={serverLoading || !serverUrl.trim()}
            className="btn-primary text-sm disabled:opacity-50"
          >
            {serverLoading ? "..." : "Tester"}
          </button>
        </div>

        {serverError && (
          <p className="text-sm text-liberte-accent">{serverError}</p>
        )}

        {serverInfo && (
          <div className="bg-liberte-bg rounded p-3 space-y-2 text-sm">
            <div className="flex items-center gap-2">
              <Wifi className="w-4 h-4 text-liberte-success" />
              <span className="font-medium">{serverInfo.name}</span>
              <span className="text-liberte-muted">v{serverInfo.version}</span>
            </div>
            <div className="grid grid-cols-2 gap-1 text-xs text-liberte-muted">
              <span>Premium requis:</span>
              <span>{serverInfo.premiumRequired ? "Oui" : "Non"}</span>
              <span>Inscriptions:</span>
              <span>{serverInfo.registrationOpen ? "Ouvertes" : "Fermées"}</span>
              <span>Max peers:</span>
              <span>{serverInfo.maxPeers === 0 ? "Illimité" : serverInfo.maxPeers}</span>
            </div>
            <button onClick={saveServer} className="btn-primary text-sm mt-2">
              Enregistrer ce serveur
            </button>
          </div>
        )}
      </div>

      {/* Identity */}
      <div className="panel p-4 space-y-4">
        <h3 className="font-medium flex items-center gap-2">
          <Shield className="w-4 h-4 text-liberte-accent" />
          Identité
        </h3>

        {identity && (
          <div className="space-y-2">
            <div>
              <label className="text-xs text-liberte-muted">
                Clé publique
              </label>
              <div className="flex items-center gap-2 mt-1">
                <code className="flex-1 text-xs bg-liberte-bg p-2 rounded font-mono break-all">
                  {identity.publicKey}
                </code>
                <button
                  onClick={copyPubkey}
                  className="p-2 hover:bg-liberte-panel rounded transition-colors"
                >
                  {copied ? (
                    <Check className="w-4 h-4 text-liberte-success" />
                  ) : (
                    <Copy className="w-4 h-4 text-liberte-muted" />
                  )}
                </button>
              </div>
            </div>

            <div>
              <label className="text-xs text-liberte-muted">ID court</label>
              <p className="text-sm font-mono mt-1">{identity.shortId}</p>
            </div>
          </div>
        )}
      </div>

      <div className="panel p-4 space-y-4">
        <h3 className="font-medium">Audio / Vidéo</h3>
        <p className="text-sm text-liberte-muted">
          Configuration des périphériques audio et vidéo.
        </p>
      </div>

      <div className="panel p-4 space-y-4">
        <h3 className="font-medium">Premium</h3>
        <p className="text-sm text-liberte-muted">
          0,99€/mois — Accès au relais SFU et stockage persistant chiffré.
        </p>
        <button className="btn-primary">Activer Premium</button>
      </div>
    </div>
  );
}
