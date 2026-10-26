import { useEffect, useRef, useState } from "react";
import { Search, X } from "lucide-react";
import { searchMessages } from "@/lib/tauri";
import type { Message } from "@/lib/types";
import { useMessageStore } from "@/stores/messageStore";

export default function MessageSearch({ channelId, onClose }: { channelId: string | null; onClose: () => void }) {
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<Message[]>([]);
  const [searching, setSearching] = useState(false);
  const [opening, setOpening] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const channels = useMessageStore((state) => state.channels);
  const mounted = useRef(false);

  useEffect(() => {
    mounted.current = true;
    return () => { mounted.current = false; };
  }, []);

  useEffect(() => {
    let disposed = false;
    setResults([]);
    setError(null);
    const valid = query.trim().length >= 2;
    setSearching(valid);
    if (!valid) return;
    const timer = setTimeout(() => {
      void searchMessages(query.trim(), channelId ?? undefined).then((messages) => {
        if (!disposed) setResults(messages);
      }).catch((reason) => {
        if (!disposed) setError(String(reason));
      }).finally(() => { if (!disposed) setSearching(false); });
    }, 300);
    return () => { disposed = true; clearTimeout(timer); };
  }, [query, channelId]);

  const openMessage = async (message: Message) => {
    if (opening) return;
    setOpening(true);
    setError(null);
    try {
      const found = await useMessageStore.getState().revealMessage(message.channelId, message.id);
      if (found && mounted.current) onClose();
    } catch (reason) { if (mounted.current) setError(String(reason)); }
    finally { if (mounted.current) setOpening(false); }
  };

  return (
    <div className="bg-liberte-surface border-b border-liberte-border px-4 py-2 space-y-2">
      <div className="flex items-center gap-2">
        <Search className="w-4 h-4 text-liberte-muted shrink-0" />
        <input autoFocus aria-label="Rechercher dans les messages" value={query}
          onChange={(event) => setQuery(event.target.value)} disabled={opening}
          placeholder="Rechercher dans les messages…"
          className="flex-1 bg-transparent text-sm focus:outline-hidden placeholder-liberte-muted" />
        <button onClick={onClose} aria-label="Fermer la recherche" className="p-1 hover:bg-liberte-panel rounded">
          <X className="w-3.5 h-3.5 text-liberte-muted" />
        </button>
      </div>
      {(searching || opening) && <p role="status" className="text-xs text-liberte-muted">{opening ? "Chargement du message…" : "Recherche…"}</p>}
      {error && <p role="alert" className="text-xs text-red-300">{error}</p>}
      {results.length > 0 && <div className="max-h-60 overflow-y-auto space-y-1">
        {results.map((message) => <button key={message.id} disabled={opening} onClick={() => void openMessage(message)}
          className="block w-full text-left text-xs p-2 bg-liberte-bg rounded hover:bg-liberte-panel transition-colors">
          <span className="flex items-center gap-2 text-liberte-muted mb-0.5">
            <span className="font-mono">{message.senderId.slice(0, 8)}…</span>
            <span className="text-liberte-accent">#{channels.find((channel) => channel.id === message.channelId)?.name || message.channelId}</span>
            <span className="ml-auto">{new Date(message.timestamp).toLocaleString()}</span>
          </span>
          <span className="block text-liberte-text truncate">{message.content}</span>
        </button>)}
      </div>}
      {query.trim().length >= 2 && !searching && !error && !opening && results.length === 0 &&
        <p className="text-xs text-liberte-muted py-1">Aucun résultat</p>}
    </div>
  );
}
