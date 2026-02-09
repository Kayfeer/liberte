import { Copy, Check } from "lucide-react";
import { useState } from "react";
import type { IdentityInfo } from "../../lib/types";

interface Props {
  identity: IdentityInfo;
}

export default function ProfileCard({ identity }: Props) {
  const [copied, setCopied] = useState(false);

  const handleCopy = () => {
    navigator.clipboard.writeText(identity.publicKey);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <div className="panel p-4 space-y-3">
      <div className="flex items-center gap-3">
        <div
          className="w-10 h-10 rounded-full flex items-center justify-center text-sm font-bold"
          style={{
            backgroundColor: `hsl(${hashCode(identity.publicKey) % 360}, 60%, 40%)`,
          }}
        >
          {identity.shortId.slice(0, 2).toUpperCase()}
        </div>
        <div>
          <p className="font-medium font-mono text-sm">{identity.shortId}</p>
          <p className="text-xs text-liberte-muted">
            Créé le{" "}
            {new Date(identity.createdAt).toLocaleDateString("fr-FR")}
          </p>
        </div>
      </div>

      <div className="flex items-center gap-2">
        <code className="flex-1 text-xs bg-liberte-bg p-2 rounded font-mono break-all text-liberte-muted">
          {identity.publicKey}
        </code>
        <button
          onClick={handleCopy}
          className="p-2 hover:bg-liberte-panel rounded transition-colors flex-shrink-0"
        >
          {copied ? (
            <Check className="w-4 h-4 text-liberte-success" />
          ) : (
            <Copy className="w-4 h-4 text-liberte-muted" />
          )}
        </button>
      </div>
    </div>
  );
}

function hashCode(str: string): number {
  let hash = 0;
  for (let i = 0; i < str.length; i++) {
    hash = (hash << 5) - hash + str.charCodeAt(i);
    hash |= 0;
  }
  return Math.abs(hash);
}
