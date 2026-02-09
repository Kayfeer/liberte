import { useState } from "react";
import Modal from "../common/Modal";
import { useMessageStore } from "../../stores/messageStore";

interface Props {
  isOpen: boolean;
  onClose: () => void;
}

export default function CreateChannelModal({ isOpen, onClose }: Props) {
  const [name, setName] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");
  const { createChannel } = useMessageStore();

  const handleCreate = async () => {
    if (!name.trim()) return;
    setLoading(true);
    setError("");
    try {
      await createChannel(name.trim());
      setName("");
      onClose();
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };

  return (
    <Modal isOpen={isOpen} onClose={onClose} title="Créer un canal">
      <div className="space-y-4">
        <div>
          <label className="text-sm text-liberte-muted block mb-1">
            Nom du canal
          </label>
          <input
            type="text"
            value={name}
            onChange={(e) => setName(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && handleCreate()}
            placeholder="ex: general"
            autoFocus
            className="w-full bg-liberte-bg border border-liberte-panel rounded px-3 py-2 text-sm focus:outline-none focus:border-liberte-accent"
          />
        </div>

        {error && (
          <p className="text-sm text-red-400">{error}</p>
        )}

        <button
          onClick={handleCreate}
          disabled={loading || !name.trim()}
          className="btn-primary w-full disabled:opacity-50"
        >
          {loading ? "Création..." : "Créer"}
        </button>
      </div>
    </Modal>
  );
}
