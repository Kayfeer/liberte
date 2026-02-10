import { useMessageStore } from "../../stores/messageStore";
import { useMediaStore } from "../../stores/mediaStore";
import MessageList from "../chat/MessageList";
import MessageInput from "../chat/MessageInput";
import TypingIndicator from "../chat/TypingIndicator";
import VoicePanel from "../voice/VoicePanel";

export default function MainPanel() {
  const { activeChannelId } = useMessageStore();
  const { inCall } = useMediaStore();

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
      {inCall && <VoicePanel />}
      <MessageList channelId={activeChannelId} />
      <TypingIndicator channelId={activeChannelId} />
      <MessageInput channelId={activeChannelId} />
    </div>
  );
}
