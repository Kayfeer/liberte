import { useEffect, useState } from "react";
import { activatePremium, checkPremium } from "@/lib/tauri";
import type { PremiumStatus } from "@/lib/types";

export default function PremiumSettings() {
  const [status, setStatus] = useState<PremiumStatus | null>(null);
  const [token, setToken] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  useEffect(() => {
    let disposed = false;
    void checkPremium().then((result) => { if (!disposed) setStatus(result); })
      .catch((cause) => { if (!disposed) setError(String(cause)); });
    return () => { disposed = true; };
  }, []);
  const activate = async () => {
    setBusy(true); setError(null);
    try { setStatus(await activatePremium(token.trim())); setToken(""); }
    catch (cause) { setError(String(cause)); }
    finally { setBusy(false); }
  };
  return <section className="panel p-4 space-y-3">
    <h3 className="font-medium">Accès au serveur</h3>
    <p className="text-sm">{status === null ? "Vérification de l’accès au serveur..." : status.isPremium ? `Accès actif${status.validUntil ? " jusqu'au " + new Date(status.validUntil).toLocaleDateString("fr-FR") : ""}.` : "Aucun abonnement actif."}</p>
    <p className="text-xs text-liberte-muted">Si votre serveur exige un abonnement, collez le jeton signé fourni par son administrateur. La messagerie directe reste utilisable sans abonnement.</p>
    <label className="block text-xs">Jeton d'accès signé
      <textarea value={token} onChange={(event) => setToken(event.target.value)} rows={3} className="input-field mt-1 font-mono text-xs" autoComplete="off" />
    </label>
    <button disabled={!token.trim() || busy} onClick={() => void activate()} className="btn-secondary text-sm">{busy ? "Vérification..." : "Vérifier et activer"}</button>
    {error && <p role="alert" className="text-sm text-red-300">{error}</p>}
  </section>;
}
