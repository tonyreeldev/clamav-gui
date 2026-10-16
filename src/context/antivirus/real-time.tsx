import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState } from "react";
import { useSettings } from "@/context/settings";
import { invoke } from "@tauri-apps/api/core";
import { toast } from "sonner";
import { useTranslation } from "react-i18next";
import { getErrorMessage } from "@/lib/helpers";

type RealtimeContextValue = {
     enabled: boolean;
     start(): Promise<void>;
     stop(): Promise<void>;
};

const RealtimeContext = createContext<RealtimeContextValue | null>(null);

export function RealtimeProvider({ children }: { children: React.ReactNode }) {
     const { settings } = useSettings();
     const {t: messageTxt} = useTranslation("messages");
     const [enabled, setEnabled] = useState(settings.realTime);
     const start = useCallback(async () => {
          await invoke("start_real_time_scan", { behavior: settings.behavior || "balanced" });
          await invoke("update_tray_icon",{state: "enabled"})
     },[settings.behavior]);
     const stop = useCallback(async () => {
          await invoke("stop_real_time_scan");
          await invoke("update_tray_icon",{state: "disabled"})
     },[]);
     const isFirstRun = useRef(true);
     useEffect(() => {
          const firstRun = isFirstRun.current;
          isFirstRun.current = false;
          if(!settings.realTime){
               // Nothing to stop at launch; stopping would only log a bogus "stopped" entry.
               if (!firstRun) stop().catch(console.error);
               setEnabled(false);
               return
          }
          // Only report protection as on when the backend actually started it. Protection is on by default,
          // so a failed start (e.g. during login) is retried instead of leaving the PC unprotected.
          let cancelled = false;
          let retry: ReturnType<typeof setTimeout> | undefined;
          const attempt = (notify: boolean) => {
               start().then(() => { if (!cancelled) setEnabled(true) }).catch(e => {
                    if (cancelled) return;
                    setEnabled(false);
                    invoke("update_tray_icon",{state: "disabled"}).catch(()=>{});
                    if (notify) toast.error(messageTxt("realtime-start-error"), { description: getErrorMessage(e) });
                    retry = setTimeout(() => attempt(false), 30_000);
               });
          };
          attempt(true);
          return () => { cancelled = true; clearTimeout(retry); };
     }, [settings.realTime, settings.behavior]);
     const values: RealtimeContextValue = useMemo(()=>({
          enabled,
          start: async () => {
               await start();
               setEnabled(true);
          },
          stop: async () => {
               await stop();
               setEnabled(false);
          },
     }),[enabled])
     return (
          <RealtimeContext.Provider
               value={values}
          >
               {children}
          </RealtimeContext.Provider>
     );
}

export function useRealtimeScan() {
     const ctx = useContext(RealtimeContext);
     if (!ctx) {
          throw new Error("useRealtimeContext must be used inside RealtimeProvider");
     }
     return ctx;
}