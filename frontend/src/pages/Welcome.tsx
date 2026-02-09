import { useState } from "react";
import { Shield, Key, ArrowRight } from "lucide-react";
import { useIdentityStore } from "../stores/identityStore";

export default function Welcome() {
  const { createIdentity, loading, error } = useIdentityStore();
  const [step, setStep] = useState<"intro" | "creating">("intro");

  const handleCreate = async () => {
    setStep("creating");
    await createIdentity();
  };

  return (
    <div className="flex items-center justify-center min-h-screen bg-liberte-bg p-4">
      <div className="max-w-md w-full">
        <div className="text-center mb-8">
          <img src="/logo.png" alt="Liberté" className="w-24 h-24 mx-auto mb-4 rounded-2xl" />
          <h1 className="text-4xl font-bold text-liberte-accent mb-2">
            Liberté
          </h1>
          <p className="text-liberte-muted text-sm">
            Communication souveraine, chiffrée, pair-à-pair
          </p>
        </div>

        <div className="panel p-6 space-y-6">
          <div className="flex items-start gap-4">
            <div className="p-2 bg-liberte-panel rounded-lg">
              <Shield className="w-6 h-6 text-liberte-accent" />
            </div>
            <div>
              <h3 className="font-medium mb-1">Chiffrement de bout en bout</h3>
              <p className="text-sm text-liberte-muted">
                XChaCha20-Poly1305 + Noise Protocol. Vos messages sont
                illisibles pour quiconque sauf vous et vos correspondants.
              </p>
            </div>
          </div>

          <div className="flex items-start gap-4">
            <div className="p-2 bg-liberte-panel rounded-lg">
              <Key className="w-6 h-6 text-liberte-accent" />
            </div>
            <div>
              <h3 className="font-medium mb-1">Identité cryptographique</h3>
              <p className="text-sm text-liberte-muted">
                Pas d'email, pas de numéro de téléphone. Votre identité est une
                clé Ed25519 générée localement.
              </p>
            </div>
          </div>

          {error && (
            <div className="p-3 bg-red-900/20 border border-red-800 rounded-lg text-sm text-red-300">
              {error}
            </div>
          )}

          <button
            onClick={handleCreate}
            disabled={loading}
            className="btn-primary w-full flex items-center justify-center gap-2"
          >
            {loading ? (
              <span>Génération de votre identité...</span>
            ) : (
              <>
                <span>Créer mon identité</span>
                <ArrowRight className="w-4 h-4" />
              </>
            )}
          </button>

          <p className="text-xs text-liberte-muted text-center">
            Votre clé privée ne quitte jamais cet appareil.
            <br />
            Aucune donnée n'est envoyée à un serveur central.
          </p>
        </div>
      </div>
    </div>
  );
}
