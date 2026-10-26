import { useEffect, useLayoutEffect, useRef } from "react";
import { useMessageStore } from "../../stores/messageStore";
import MessageBubble from "./MessageBubble";

interface Props { channelId: string }

export default function MessageList({ channelId }: Props) {
  const messages = useMessageStore((state) => state.messages[channelId]);
  const loading = useMessageStore((state) => state.loadingByChannel[channelId]);
  const loadingMore = useMessageStore((state) => state.loadingMore[channelId]);
  const hasMore = useMessageStore((state) => state.hasMore[channelId]);
  const error = useMessageStore((state) => state.errors[channelId]);
  const focusedMessageId = useMessageStore((state) => state.focusedMessageId);
  const viewport = useRef<HTMLDivElement>(null);
  const bottomRef = useRef<HTMLDivElement>(null);
  const previousHeight = useRef<number | null>(null);
  const newestId = messages?.[messages.length - 1]?.id;

  useEffect(() => {
    if (!focusedMessageId && previousHeight.current === null) bottomRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [newestId, channelId]);

  useLayoutEffect(() => {
    if (!loadingMore && previousHeight.current !== null && viewport.current) {
      viewport.current.scrollTop += viewport.current.scrollHeight - previousHeight.current;
      previousHeight.current = null;
    }
  }, [loadingMore, messages]);

  useEffect(() => {
    if (focusedMessageId) document.getElementById("message-" + focusedMessageId)?.scrollIntoView({ behavior: "smooth", block: "center" });
  }, [focusedMessageId, channelId]);

  const loadOlder = () => {
    previousHeight.current = viewport.current?.scrollHeight ?? null;
    void useMessageStore.getState().loadMoreMessages(channelId);
  };

  return (
    <div ref={viewport} className="flex-1 overflow-y-auto px-4 py-2 space-y-1" aria-label="Historique des messages">
      {error && <div role="alert" className="text-red-300 text-sm p-2">
        {error}
        <button onClick={() => void useMessageStore.getState().loadMessages(channelId)} className="ml-2 underline">Réessayer</button>
      </div>}
      {hasMore && <button onClick={loadOlder} disabled={loadingMore} className="btn-secondary block mx-auto text-xs mb-3">
        {loadingMore ? "Chargement..." : "Charger les messages précédents"}
      </button>}
      {!messages?.length && <p className="text-liberte-muted text-sm text-center py-6">
        {loading ? "Chargement des messages..." : error ? "Historique indisponible." : "Aucun message. Commencez la conversation !"}
      </p>}
      {messages?.map((message) => <div key={message.id} id={"message-" + message.id}
        className={focusedMessageId === message.id ? "rounded border border-liberte-accent/50 bg-liberte-accent/10" : undefined}>
        <MessageBubble message={message}
          onReactionChange={() => void useMessageStore.getState().refreshReactions(channelId, message.id)} />
      </div>)}
      <div ref={bottomRef} />
    </div>
  );
}
