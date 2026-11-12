import { useState } from "react";

import {
  discoverPlugins,
  previewPluginInstall,
  startPluginInstall,
  type PluginDiscoveryResult,
  type PluginInstallConsent,
} from "../../lib/api";

export interface PluginJobRef {
  id: string;
  title: string;
}

/** The `gh:owner/repo` source taken from a discovery row's install command. */
export const sourceFromCommand = (command: string) => command.replace(/^.*\binstall\s+/, "").trim();

/** GitHub discovery and the preview, consent and install flow. */
export function usePluginMarketplace(startJob: (job: PluginJobRef) => void) {
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<PluginDiscoveryResult[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [discovering, setDiscovering] = useState(false);
  const [previewingSource, setPreviewingSource] = useState<string | null>(null);
  const [consent, setConsent] = useState<PluginInstallConsent | null>(null);
  const [installBusy, setInstallBusy] = useState(false);
  const [installError, setInstallError] = useState<string | null>(null);

  const discover = async () => {
    setDiscovering(true);
    setError(null);
    const res = await discoverPlugins(query);
    if (res.kind === "ok") {
      setResults(res.results);
    } else {
      setResults(null);
      setError(res.message);
    }
    setDiscovering(false);
  };

  const install = async (source: string) => {
    setPreviewingSource(source);
    setError(null);
    setInstallError(null);
    try {
      const res = await previewPluginInstall(source);
      if (res.kind === "ok") setConsent(res.consent);
      else setError(res.message);
    } finally {
      setPreviewingSource(null);
    }
  };

  const approveInstall = async () => {
    if (!consent) return;
    setInstallBusy(true);
    setInstallError(null);
    try {
      const res = await startPluginInstall(consent.source, consent.fingerprint);
      if (res.kind === "ok") {
        setConsent(null);
        startJob({ id: res.jobId, title: `Installing ${consent.id}` });
      } else {
        setInstallError(res.message);
      }
    } finally {
      setInstallBusy(false);
    }
  };

  return {
    query,
    setQuery,
    results,
    error,
    discovering,
    previewingSource,
    consent,
    installBusy,
    installError,
    discover,
    install,
    approveInstall,
    closeConsent: () => setConsent(null),
  };
}

export type PluginMarketplaceState = ReturnType<typeof usePluginMarketplace>;
