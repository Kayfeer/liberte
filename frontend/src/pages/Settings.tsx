import { useIdentityStore } from "../stores/identityStore";
import { useNavigationStore } from "../stores/navigationStore";
import { useMediaDevices } from "../hooks/useMediaDevices";
import {
  Shield,
  Copy,
  Check,
  Server,
  ArrowLeft,
  Mic,
  Volume2,
  Video,
  Sparkles,
  Construction,
  AudioLines,
  RefreshCw,
  HardDriveDownload,
  FolderOpen,
  Cloud,
  Download,
} from "lucide-react";
import { useState, useRef, useEffect, useCallback } from "react";
import { useBackupStore } from "../stores/backupStore";

/* ─── Reusable sub-components ─── */

function DeviceSelect({
  label,
  icon: Icon,
  devices,
  value,
  onChange,
}: {
  label: string;
  icon: React.ComponentType<{ className?: string }>;
  devices: { deviceId: string; label: string }[];
  value: string;
  onChange: (id: string) => void;
}) {
  return (
    <div className="space-y-1.5">
      <label className="text-xs text-liberte-muted flex items-center gap-1.5">
        <Icon className="w-3.5 h-3.5" />
        {label}
      </label>
      <select
        value={value}
        onChange={(e) => onChange(e.target.value)}
        className="input-field text-sm"
      >
        {devices.length === 0 && (
          <option value="">Aucun périphérique détecté</option>
        )}
        {devices.map((d) => (
          <option key={d.deviceId} value={d.deviceId}>
            {d.label}
          </option>
        ))}
      </select>
    </div>
  );
}

function VolumeSlider({
  label,
  value,
  onChange,
}: {
  label: string;
  value: number;
  onChange: (v: number) => void;
}) {
  return (
    <div className="space-y-1.5">
      <div className="flex items-center justify-between">
        <label className="text-xs text-liberte-muted">{label}</label>
        <span className="text-xs font-mono text-liberte-muted">{value}%</span>
      </div>
      <input
        type="range"
        min={0}
        max={200}
        value={value}
        onChange={(e) => onChange(Number(e.target.value))}
        className="w-full h-1.5 bg-liberte-border rounded-full appearance-none cursor-pointer
                   accent-liberte-accent"
      />
    </div>
  );
}

function Toggle({
  label,
  description,
  checked,
  onChange,
  badge,
}: {
  label: string;
  description?: string;
  checked: boolean;
  onChange: (v: boolean) => void;
  badge?: string;
}) {
  return (
    <label className="flex items-start gap-3 cursor-pointer group">
      <input
        type="checkbox"
        className="sr-only"
        checked={checked}
        onChange={(e) => onChange(e.target.checked)}
      />
      <div className="pt-0.5">
        <div
          className={`w-9 h-5 rounded-full transition-colors duration-200 flex items-center px-0.5 ${
            checked ? "bg-liberte-accent" : "bg-liberte-border"
          }`}
        >
          <div
            className={`w-4 h-4 bg-white rounded-full shadow transition-transform duration-200 ${
              checked ? "translate-x-4" : "translate-x-0"
            }`}
          />
        </div>
      </div>
      <div className="flex-1">
        <div className="flex items-center gap-2">
          <span className="text-sm font-medium group-hover:text-liberte-accent transition-colors">
            {label}
          </span>
          {badge && (
            <span className="text-[10px] bg-liberte-accent/20 text-liberte-accent px-1.5 py-0.5 rounded-full font-medium">
              {badge}
            </span>
          )}
        </div>
        {description && (
          <p className="text-xs text-liberte-muted mt-0.5">{description}</p>
        )}
      </div>
    </label>
  );
}

/* ─── Main Settings page ─── */

function CameraPreview({ deviceId }: { deviceId: string }) {
  const videoRef = useRef<HTMLVideoElement>(null);
  const streamRef = useRef<MediaStream | null>(null);

  const startPreview = useCallback(async () => {
    // Stop any existing stream
    if (streamRef.current) {
      streamRef.current.getTracks().forEach((t) => t.stop());
      streamRef.current = null;
    }

    if (!deviceId) return;

    try {
      const stream = await navigator.mediaDevices.getUserMedia({
        video: { deviceId: { exact: deviceId }, width: 640, height: 360 },
        audio: false,
      });
      streamRef.current = stream;
      if (videoRef.current) {
        videoRef.current.srcObject = stream;
      }
    } catch {
      // Camera unavailable — silently ignore
    }
  }, [deviceId]);

  useEffect(() => {
    startPreview();
    return () => {
      if (streamRef.current) {
        streamRef.current.getTracks().forEach((t) => t.stop());
        streamRef.current = null;
      }
    };
  }, [startPreview]);

  return (
    <div className="relative rounded-lg overflow-hidden bg-liberte-bg border border-liberte-border aspect-video">
      <video
        ref={videoRef}
        autoPlay
        playsInline
        muted
        className="w-full h-full object-cover mirror"
      />
      {!deviceId && (
        <div className="absolute inset-0 flex items-center justify-center text-liberte-muted text-sm">
          Aucune caméra sélectionnée
        </div>
      )}
    </div>
  );
}

export default function Settings() {
  const { identity } = useIdentityStore();
  const navigate = useNavigationStore((s) => s.navigate);
  const media = useMediaDevices();
  const backup = useBackupStore();
  const [copied, setCopied] = useState(false);

  const copyPubkey = () => {
    if (identity) {
      navigator.clipboard.writeText(identity.publicKey);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    }
  };

  return (
    <div className="p-6 max-w-2xl mx-auto space-y-6 overflow-y-auto flex-1">
      {/* ── Header ── */}
      <div className="flex items-center gap-3">
        <button
          onClick={() => navigate("home")}
          className="p-1.5 hover:bg-liberte-panel rounded transition-colors"
        >
          <ArrowLeft className="w-5 h-5 text-liberte-muted" />
        </button>
        <h2 className="text-xl font-bold">Paramètres</h2>
      </div>

      {/* ── Server / Self-hosted ── */}
      <div className="panel p-4 space-y-4 opacity-60">
        <h3 className="font-medium flex items-center gap-2">
          <Server className="w-4 h-4 text-liberte-accent" />
          Serveur relais
          <span className="text-xs bg-liberte-panel text-liberte-muted px-2 py-0.5 rounded-full">
            Bientôt disponible
          </span>
        </h3>
        <p className="text-sm text-liberte-muted">
          La connexion à un serveur relais auto-hébergé sera disponible dans une
          prochaine mise à jour. En attendant, Liberté fonctionne en mode
          pair-à-pair pur.
        </p>
      </div>

      {/* ── Identity ── */}
      <div className="panel p-4 space-y-4">
        <h3 className="font-medium flex items-center gap-2">
          <Shield className="w-4 h-4 text-liberte-accent" />
          Identité
        </h3>

        {identity && (
          <div className="space-y-2">
            <div>
              <label className="text-xs text-liberte-muted">
                Clé publique
              </label>
              <div className="flex items-center gap-2 mt-1">
                <code className="flex-1 text-xs bg-liberte-bg p-2 rounded font-mono break-all">
                  {identity.publicKey}
                </code>
                <button
                  onClick={copyPubkey}
                  className="p-2 hover:bg-liberte-panel rounded transition-colors"
                >
                  {copied ? (
                    <Check className="w-4 h-4 text-liberte-success" />
                  ) : (
                    <Copy className="w-4 h-4 text-liberte-muted" />
                  )}
                </button>
              </div>
            </div>

            <div>
              <label className="text-xs text-liberte-muted">ID court</label>
              <p className="text-sm font-mono mt-1">{identity.shortId}</p>
            </div>
          </div>
        )}
      </div>

      {/* ── Audio ── */}
      <div className="panel p-4 space-y-5">
        <div className="flex items-center justify-between">
          <h3 className="font-medium flex items-center gap-2">
            <Mic className="w-4 h-4 text-liberte-accent" />
            Audio
          </h3>
          <button
            onClick={media.refresh}
            className="p-1.5 hover:bg-liberte-panel rounded transition-colors"
            title="Rafraîchir les périphériques"
          >
            <RefreshCw className="w-3.5 h-3.5 text-liberte-muted" />
          </button>
        </div>

        {media.error && (
          <p className="text-xs text-liberte-accent bg-liberte-accent/10 p-2 rounded">
            ⚠ {media.error}
          </p>
        )}

        {/* Input device */}
        <DeviceSelect
          label="Périphérique d'entrée (micro)"
          icon={Mic}
          devices={media.audioInputs}
          value={media.selectedAudioInput}
          onChange={(id) => media.update({ selectedAudioInput: id })}
        />

        <VolumeSlider
          label="Volume d'entrée"
          value={media.inputVolume}
          onChange={(v) => media.update({ inputVolume: v })}
        />

        {/* Output device */}
        <DeviceSelect
          label="Périphérique de sortie (haut-parleurs / casque)"
          icon={Volume2}
          devices={media.audioOutputs}
          value={media.selectedAudioOutput}
          onChange={(id) => media.update({ selectedAudioOutput: id })}
        />

        <VolumeSlider
          label="Volume de sortie"
          value={media.outputVolume}
          onChange={(v) => media.update({ outputVolume: v })}
        />

        {/* Separator */}
        <div className="border-t border-liberte-border" />

        {/* Voice processing */}
        <h4 className="text-sm font-medium flex items-center gap-2">
          <AudioLines className="w-4 h-4 text-liberte-accent" />
          Traitement vocal
        </h4>

        <div className="space-y-4">
          <Toggle
            label="Suppression du bruit"
            description="Filtre les bruits de fond (clavier, ventilateur, environnement)"
            checked={media.noiseSuppression}
            onChange={(v) => media.update({ noiseSuppression: v })}
          />

          <Toggle
            label="Annulation d'écho"
            description="Empêche les retours audio entre le micro et les haut-parleurs"
            checked={media.echoCancellation}
            onChange={(v) => media.update({ echoCancellation: v })}
          />

          <Toggle
            label="Contrôle automatique du gain"
            description="Ajuste automatiquement le volume de votre micro pour un niveau constant"
            checked={media.autoGainControl}
            onChange={(v) => media.update({ autoGainControl: v })}
          />

          <Toggle
            label="Isolation vocale"
            description="Isole votre voix et supprime tous les sons environnants — idéal en environnement bruyant"
            checked={media.voiceIsolation}
            onChange={(v) => media.update({ voiceIsolation: v })}
            badge="Expérimental"
          />
        </div>
      </div>

      {/* ── Video ── */}
      <div className="panel p-4 space-y-5">
        <h3 className="font-medium flex items-center gap-2">
          <Video className="w-4 h-4 text-liberte-accent" />
          Vidéo
        </h3>

        <DeviceSelect
          label="Caméra"
          icon={Video}
          devices={media.videoInputs}
          value={media.selectedVideoInput}
          onChange={(id) => media.update({ selectedVideoInput: id })}
        />

        {media.videoInputs.length > 0 && (
          <CameraPreview deviceId={media.selectedVideoInput} />
        )}

        {media.videoInputs.length === 0 && !media.loading && (
          <p className="text-xs text-liberte-muted">
            Aucune caméra détectée. Branchez une webcam et cliquez sur
            rafraîchir.
          </p>
        )}
      </div>

      {/* ── Sauvegarde ── */}
      <div className="panel p-4 space-y-5">
        <h3 className="font-medium flex items-center gap-2">
          <HardDriveDownload className="w-4 h-4 text-liberte-accent" />
          Sauvegarde
        </h3>

        <Toggle
          label="Sauvegarde automatique"
          description={`Exporte automatiquement vos données toutes les ${backup.intervalMinutes} minutes`}
          checked={backup.autoBackupEnabled}
          onChange={(v) => backup.setAutoBackup(v)}
        />

        {backup.autoBackupEnabled && (
          <div className="space-y-1.5">
            <div className="flex items-center justify-between">
              <label className="text-xs text-liberte-muted">Fréquence</label>
              <span className="text-xs font-mono text-liberte-muted">
                {backup.intervalMinutes} min
              </span>
            </div>
            <input
              type="range"
              min={5}
              max={120}
              step={5}
              value={backup.intervalMinutes}
              onChange={(e) => backup.setInterval(Number(e.target.value))}
              className="w-full h-1.5 bg-liberte-border rounded-full appearance-none cursor-pointer accent-liberte-accent"
            />
          </div>
        )}

        {backup.lastBackupTime && (
          <p className="text-xs text-liberte-muted">
            Dernière sauvegarde :{" "}
            {new Date(backup.lastBackupTime).toLocaleString()}
          </p>
        )}

        {backup.error && (
          <p className="text-xs text-red-400 bg-red-400/10 p-2 rounded">
            ⚠ {backup.error}
          </p>
        )}

        <div className="flex flex-wrap gap-2">
          <button
            onClick={() => backup.runAutoBackup()}
            disabled={backup.isBackingUp}
            className="btn-secondary text-xs flex items-center gap-1.5"
          >
            <Download className="w-3.5 h-3.5" />
            {backup.isBackingUp ? "Sauvegarde..." : "Sauvegarder maintenant"}
          </button>

          <button
            onClick={async () => {
              const { save } = await import("@tauri-apps/plugin-dialog");
              const path = await save({
                defaultPath: `liberte_backup_${new Date().toISOString().slice(0, 10)}.json`,
                filters: [{ name: "JSON", extensions: ["json"] }],
              });
              if (path) await backup.saveToFile(path);
            }}
            disabled={backup.isBackingUp}
            className="btn-secondary text-xs flex items-center gap-1.5"
          >
            <FolderOpen className="w-3.5 h-3.5" />
            Exporter vers un fichier
          </button>

          <button
            onClick={async () => {
              const { open: openFile } = await import(
                "@tauri-apps/plugin-dialog"
              );
              const path = await openFile({
                multiple: false,
                filters: [{ name: "JSON", extensions: ["json"] }],
                title: "Importer une sauvegarde",
              });
              if (path) {
                const { readTextFile } = await import(
                  "@tauri-apps/plugin-fs"
                );
                const json = await readTextFile(path);
                const stats = await backup.importFromFile(json);
                alert(
                  `Importé : ${stats.channelsImported} channels, ${stats.messagesImported} messages, ${stats.keysImported} clés`
                );
              }
            }}
            disabled={backup.isRestoring}
            className="btn-secondary text-xs flex items-center gap-1.5"
          >
            <Cloud className="w-3.5 h-3.5" />
            {backup.isRestoring ? "Importation..." : "Importer"}
          </button>
        </div>
      </div>

      {/* ── Premium (en cours de confection) ── */}
      <div className="panel p-4 space-y-4 relative overflow-hidden">
        {/* Decorative gradient band */}
        <div className="absolute inset-x-0 top-0 h-1 bg-gradient-to-r from-liberte-accent via-yellow-500 to-liberte-accent" />

        <h3 className="font-medium flex items-center gap-2 pt-1">
          <Sparkles className="w-4 h-4 text-yellow-500" />
          Premium
          <span className="text-xs bg-yellow-500/20 text-yellow-400 px-2 py-0.5 rounded-full flex items-center gap-1">
            <Construction className="w-3 h-3" />
            En cours de confection
          </span>
        </h3>

        <p className="text-sm text-liberte-muted">
          L'abonnement Premium est en cours de développement. Il offrira :
        </p>

        <ul className="text-sm text-liberte-muted space-y-1.5 list-none">
          <li className="flex items-center gap-2">
            <span className="w-1.5 h-1.5 rounded-full bg-yellow-500/60" />
            Relais SFU pour les appels de groupe à faible latence
          </li>
          <li className="flex items-center gap-2">
            <span className="w-1.5 h-1.5 rounded-full bg-yellow-500/60" />
            Stockage persistant chiffré de bout en bout
          </li>
          <li className="flex items-center gap-2">
            <span className="w-1.5 h-1.5 rounded-full bg-yellow-500/60" />
            Transfert de fichiers volumineux via le serveur
          </li>
          <li className="flex items-center gap-2">
            <span className="w-1.5 h-1.5 rounded-full bg-yellow-500/60" />
            Badge Premium sur votre profil
          </li>
        </ul>

        <div className="flex items-center gap-3 pt-1">
          <button
            disabled
            className="btn-primary opacity-50 cursor-not-allowed"
          >
            Bientôt disponible — 0,99 €/mois
          </button>
        </div>
      </div>
    </div>
  );
}
