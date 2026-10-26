import { useState, useEffect, useCallback, useRef } from "react";
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

  const requestVersion = useRef(0);
  const expiresAt = useRef(0);
  const copiedTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  const generate = useCallback(async () => {
    const version = ++requestVersion.current;
    const startedAt = Date.now();
    setLoading(true);
    setCode("");
    setError("");
    setCopied(false);
    expiresAt.current = 0;
    try {
      const inviteCode = await generateInvite(channelId, channelName, channelKeyHex);
      if (requestVersion.current !== version) return;
      expiresAt.current = startedAt + INVITE_DURATION_SECS * 1000;
      setCode(inviteCode);
      setRemaining(Math.max(0, Math.ceil((expiresAt.current - Date.now()) / 1000)));
    } catch (reason) {
      if (requestVersion.current === version) setError(String(reason));
    } finally {
      if (requestVersion.current === version) setLoading(false);
    }
  }, [channelId, channelName, channelKeyHex]);

  useEffect(() => {
    if (isOpen) void generate();
    return () => {
      ++requestVersion.current;
      if (copiedTimer.current) clearTimeout(copiedTimer.current);
    };
  }, [isOpen, generate]);

  useEffect(() => {
    if (!isOpen || !code) return;
    const interval = setInterval(() => {
      const secondsLeft = Math.max(0, Math.ceil((expiresAt.current - Date.now()) / 1000));
      setRemaining(secondsLeft);
      if (secondsLeft === 0) {
        clearInterval(interval);
        void generate();
      }
    }, 1000);
    return () => clearInterval(interval);
  }, [isOpen, code, generate]);

  const copyCode = async () => {
    const version = requestVersion.current;
    try {
      await navigator.clipboard.writeText(code);
      if (requestVersion.current !== version) return;
      setCopied(true);
      setError("");
      if (copiedTimer.current) clearTimeout(copiedTimer.current);
      copiedTimer.current = setTimeout(() => setCopied(false), 2000);
    } catch (reason) {
      if (requestVersion.current === version) setError(`Copie impossible : ${String(reason)}`);
    }
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

        {error && <p role="alert" className="text-sm text-red-400">{error}</p>}

        {code && (
          <div className="relative">
            <textarea
              aria-label="Code d'invitation"
              readOnly
              value={code}
              className="w-full bg-liberte-bg border border-liberte-panel rounded px-3 py-2 text-xs font-mono resize-none h-24 focus:outline-hidden"
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

        {!loading && !code && <button className="btn-primary" onClick={() => void generate()}>Réessayer</button>}

        {loading && !code && (
          <p className="text-sm text-liberte-muted text-center py-4">
            Génération du code...
          </p>
        )}
      </div>
    </Modal>
  );
}

