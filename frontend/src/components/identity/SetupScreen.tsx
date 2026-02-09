import { Shield, Key } from "lucide-react";

interface Props {
  onCreateIdentity: () => void;
  loading: boolean;
}

export default function SetupScreen({ onCreateIdentity, loading }: Props) {
  return (
    <div className="text-center space-y-6">
      <div className="flex justify-center gap-4">
        <div className="p-3 bg-liberte-panel rounded-xl">
          <Shield className="w-8 h-8 text-liberte-accent" />
        </div>
        <div className="p-3 bg-liberte-panel rounded-xl">
          <Key className="w-8 h-8 text-liberte-accent" />
        </div>
      </div>

      <div>
        <h2 className="text-xl font-bold mb-2">Créer votre identité</h2>
        <p className="text-sm text-liberte-muted">
          Une paire de clés Ed25519 sera générée localement. Votre clé publique
          deviendra votre identifiant unique sur le réseau.
        </p>
      </div>

      <button
        onClick={onCreateIdentity}
        disabled={loading}
        className="btn-primary w-full"
      >
        {loading ? "Génération en cours..." : "Générer ma clé"}
      </button>
    </div>
  );
}
