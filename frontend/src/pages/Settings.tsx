import { useIdentityStore } from "../stores/identityStore";
import { useNavigationStore } from "../stores/navigationStore";
import { Shield, Copy, Check, Server, ArrowLeft } from "lucide-react";
import { useState } from "react";

export default function Settings() {
  const { identity } = useIdentityStore();
  const navigate = useNavigationStore((s) => s.navigate);
  const [copied, setCopied] = useState(false);

  const copyPubkey = () => {
    if (identity) {
      navigator.clipboard.writeText(identity.publicKey);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    }
  };

  return (
    <div className="p-6 max-w-2xl mx-auto space-y-6 overflow-y-auto flex-1">
      <div className="flex items-center gap-3">
        <button
          onClick={() => navigate("home")}
          className="p-1.5 hover:bg-liberte-panel rounded transition-colors"
        >
          <ArrowLeft className="w-5 h-5 text-liberte-muted" />
        </button>
        <h2 className="text-xl font-bold">Paramètres</h2>
      </div>

      {/* Server / Self-hosted */}
      <div className="panel p-4 space-y-4 opacity-60">
        <h3 className="font-medium flex items-center gap-2">
          <Server className="w-4 h-4 text-liberte-accent" />
          Serveur relais
          <span className="text-xs bg-liberte-panel text-liberte-muted px-2 py-0.5 rounded-full">
            Bientôt disponible
          </span>
        </h3>
        <p className="text-sm text-liberte-muted">
          La connexion à un serveur relais auto-hébergé sera disponible dans une prochaine mise à jour.
          En attendant, Liberté fonctionne en mode pair-à-pair pur.
        </p>
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
