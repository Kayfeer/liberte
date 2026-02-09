import { Wifi, Shield, WifiOff } from "lucide-react";
import { useNetworkStore } from "../../stores/networkStore";

export default function ConnectionBadge() {
  const { connectionMode } = useNetworkStore();

  const config = {
    direct: {
      icon: Wifi,
      label: "Connexion directe",
      color: "text-liberte-success",
      bg: "bg-liberte-success/10",
      border: "border-liberte-success/30",
    },
    relayed: {
      icon: Shield,
      label: "Relayée / Sécurisée",
      color: "text-blue-400",
      bg: "bg-blue-400/10",
      border: "border-blue-400/30",
    },
    disconnected: {
      icon: WifiOff,
      label: "Déconnecté",
      color: "text-liberte-muted",
      bg: "bg-liberte-bg",
      border: "border-liberte-border",
    },
  };

  const { icon: Icon, label, color, bg, border } = config[connectionMode];

  return (
    <div
      className={`flex items-center gap-1.5 px-2 py-1 rounded-full text-xs border ${bg} ${border} ${color}`}
    >
      <Icon className="w-3 h-3" />
      <span>{label}</span>
    </div>
  );
}
