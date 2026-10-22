import { useMessageStore } from "../../stores/messageStore";
import MessageList from "../chat/MessageList";
import MessageInput from "../chat/MessageInput";
import TypingIndicator from "../chat/TypingIndicator";
import FileTransfers from "../chat/FileTransfers";

export default function MainPanel() {
  const { activeChannelId } = useMessageStore();

  if (!activeChannelId) {
    return (
      <div className="flex-1 flex items-center justify-center">
        <div className="text-center">
          <h2 className="text-xl font-bold text-liberte-accent mb-2">
            Liberté
          </h2>
          <p className="text-liberte-muted text-sm">
            Sélectionnez un canal pour commencer
          </p>
        </div>
      </div>
    );
  }

  return (
    <div className="flex-1 flex flex-col min-h-0">
      <MessageList channelId={activeChannelId} />
      <FileTransfers channelId={activeChannelId} />
      <TypingIndicator channelId={activeChannelId} />
      <MessageInput channelId={activeChannelId} />
    </div>
  );
}
