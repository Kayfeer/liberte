import { useEffect, useState } from "react";
import { useIdentityStore } from "./stores/identityStore";
import { useThemeStore } from "./stores/themeStore";
import Welcome from "./pages/Welcome";
import Home from "./pages/Home";

export default function App() {
  const { identity, loadIdentity, loading } = useIdentityStore();
  const [initialized, setInitialized] = useState(false);

  // Initialize theme CSS variables on mount
  useThemeStore();

  useEffect(() => {
    loadIdentity().finally(() => setInitialized(true));
  }, [loadIdentity]);

  if (!initialized || loading) {
    return (
      <div className="flex items-center justify-center h-screen bg-liberte-bg">
        <div className="text-center">
          <h1 className="text-3xl font-bold text-liberte-accent mb-2">
            Liberté
          </h1>
          <p className="text-liberte-muted">Chargement...</p>
        </div>
      </div>
    );
  }

  if (!identity) {
    return <Welcome />;
  }

  return <Home />;
}
