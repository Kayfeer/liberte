import { useEffect, useRef } from "react";
import { useMessageStore } from "../../stores/messageStore";
import MessageBubble from "./MessageBubble";

interface Props {
  channelId: string;
}

export default function MessageList({ channelId }: Props) {
  const { messages, loading } = useMessageStore();
  const bottomRef = useRef<HTMLDivElement>(null);
  const channelMessages = messages[channelId] || [];

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [channelMessages.length]);

  if (loading) {
    return (
      <div className="flex-1 flex items-center justify-center">
        <p className="text-liberte-muted text-sm">Chargement des messages...</p>
      </div>
    );
  }

  if (channelMessages.length === 0) {
    return (
      <div className="flex-1 flex items-center justify-center">
        <p className="text-liberte-muted text-sm">
          Aucun message. Commencez la conversation !
        </p>
      </div>
    );
  }

  return (
    <div className="flex-1 overflow-y-auto px-4 py-2 space-y-1">
      {channelMessages.map((msg) => (
        <MessageBubble key={msg.id} message={msg} />
      ))}
      <div ref={bottomRef} />
    </div>
  );
}
