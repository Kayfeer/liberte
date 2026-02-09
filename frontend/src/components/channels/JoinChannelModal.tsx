import { useState } from "react";
import Modal from "../common/Modal";
import { acceptInvite } from "../../lib/tauri";
import { useMessageStore } from "../../stores/messageStore";

interface Props {
  isOpen: boolean;
  onClose: () => void;
}

export default function JoinChannelModal({ isOpen, onClose }: Props) {
  const [code, setCode] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");
  const { loadChannels, setActiveChannel, setChannelKey } = useMessageStore();

  const handleJoin = async () => {
    if (!code.trim()) return;
    setLoading(true);
    setError("");
    try {
      const result = await acceptInvite(code.trim());
      setChannelKey(result.id, result.channelKeyHex);
      await loadChannels();
      setActiveChannel(result.id);
      setCode("");
      onClose();
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };

  return (
    <Modal isOpen={isOpen} onClose={onClose} title="Rejoindre un canal">
      <div className="space-y-4">
        <div>
          <label className="text-sm text-liberte-muted block mb-1">
            Code d'invitation
          </label>
          <textarea
            value={code}
            onChange={(e) => setCode(e.target.value)}
            placeholder="Collez le code d'invitation ici..."
            autoFocus
            className="w-full bg-liberte-bg border border-liberte-panel rounded px-3 py-2 text-xs font-mono resize-none h-24 focus:outline-none focus:border-liberte-accent"
          />
        </div>

        {error && (
          <p className="text-sm text-red-400">{error}</p>
        )}

        <button
          onClick={handleJoin}
          disabled={loading || !code.trim()}
          className="btn-primary w-full disabled:opacity-50"
        >
          {loading ? "Connexion..." : "Rejoindre"}
        </button>
      </div>
    </Modal>
  );
}
