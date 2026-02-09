import { useState, useEffect, useCallback } from "react";
import { Copy, Check, RefreshCw } from "lucide-react";
import Modal from "../common/Modal";
import { generateInvite } from "../../lib/tauri";

interface Props {
  isOpen: boolean;
  onClose: () => void;
  channelId: string;
  channelName: string;
  channelKeyHex: string;
}

const INVITE_DURATION_SECS = 300; // 5 minutes

export default function InviteModal({
  isOpen,
  onClose,
  channelId,
  channelName,
  channelKeyHex,
}: Props) {
  const [code, setCode] = useState("");
  const [copied, setCopied] = useState(false);
  const [remaining, setRemaining] = useState(INVITE_DURATION_SECS);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");

  const generate = useCallback(async () => {
    setLoading(true);
    setError("");
    setCopied(false);
    try {
      const inviteCode = await generateInvite(channelId, channelName, channelKeyHex);
      setCode(inviteCode);
      setRemaining(INVITE_DURATION_SECS);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, [channelId, channelName, channelKeyHex]);

  useEffect(() => {
    if (isOpen) generate();
  }, [isOpen, generate]);

  useEffect(() => {
    if (!isOpen || !code) return;

    const interval = setInterval(() => {
      setRemaining((prev) => {
        if (prev <= 1) {
          generate();
          return INVITE_DURATION_SECS;
        }
        return prev - 1;
      });
    }, 1000);

    return () => clearInterval(interval);
  }, [isOpen, code, generate]);

  const copyCode = () => {
    navigator.clipboard.writeText(code);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  const minutes = Math.floor(remaining / 60);
  const seconds = remaining % 60;

  return (
    <Modal isOpen={isOpen} onClose={onClose} title={`Invitation - #${channelName}`}>
      <div className="space-y-4">
        <p className="text-sm text-liberte-muted">
          Partagez ce code d'invitation. Il expire dans{" "}
          <span className="font-mono text-liberte-accent">
            {minutes}:{seconds.toString().padStart(2, "0")}
          </span>
        </p>

        {error && <p className="text-sm text-red-400">{error}</p>}

        {code && (
          <div className="relative">
            <textarea
              readOnly
              value={code}
              className="w-full bg-liberte-bg border border-liberte-panel rounded px-3 py-2 text-xs font-mono resize-none h-24 focus:outline-none"
            />
            <div className="flex gap-2 mt-2">
              <button
                onClick={copyCode}
                className="btn-primary flex-1 flex items-center justify-center gap-2 text-sm"
              >
                {copied ? (
                  <>
                    <Check className="w-4 h-4" />
                    Copié
                  </>
                ) : (
                  <>
                    <Copy className="w-4 h-4" />
                    Copier le code
                  </>
                )}
              </button>
              <button
                onClick={generate}
                disabled={loading}
                className="p-2 hover:bg-liberte-panel rounded transition-colors"
                title="Régénérer"
              >
                <RefreshCw className={`w-4 h-4 text-liberte-muted ${loading ? "animate-spin" : ""}`} />
              </button>
            </div>
          </div>
        )}

        {loading && !code && (
          <p className="text-sm text-liberte-muted text-center py-4">
            Génération du code...
          </p>
        )}
      </div>
    </Modal>
  );
}
