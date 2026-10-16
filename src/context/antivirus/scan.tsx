import { GET_INITIAL_SCAN_STATE, INITIAL_FINISH_SCAN_STATE } from "@/lib/constants/states";
import { SetDataFunction, SetStateType } from "@/lib/types";
import { ScanProfile, ScanType } from "@/lib/types/enums";
import { IFinishScanState, IScanPageState } from "@/lib/types/states";
import { ScanProfileValues } from "@/lib/types/settings";
import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, UnlistenFn } from "@tauri-apps/api/event";
import { toast } from "sonner";
import { useTranslation } from "react-i18next";
import { useSettings } from "@/context/settings";
import { useBackendSettings } from "@/hooks/use-settings";
import { hydrateProfile, mapScanSettingsToArgs, validateScanSettings } from "@/lib/helpers/scan";
import { getErrorMessage } from "@/lib/helpers";

interface ScanContextValues {
     finishScanState: IFinishScanState,
     setFinishScanState: SetStateType<IFinishScanState>,
     updateFinishScanState: SetDataFunction<IFinishScanState>
     scanState: IScanPageState,
     setScanState: SetStateType<IScanPageState>
     updateScanState: SetDataFunction<IScanPageState>,
     startScan: (type: ScanType, paths: string[], route: string) => void,
     reset: () => void,
     markSeen: () => void,
     /** A scan is running, or finished with results the user hasn't looked at yet. */
     hasSession: boolean,
     /** Route of the current scan, to bring the user back to it. */
     activeRoute: string,
     startedAt: number | null,
}

const ACTIVE = ["starting", "running", "reconnecting"];
const IDLE_STATE = GET_INITIAL_SCAN_STATE(ScanType.None, []);

const ScanContext = createContext<ScanContextValues | null>(null)

// Lives above the router so a scan keeps its progress and results while the user browses other sections.
export default function ScanProvider({children}: {children: React.ReactNode}){
     const [finishScanState, setFinishScanState] = useState<IFinishScanState>(INITIAL_FINISH_SCAN_STATE)
     const updateFinishScanState: SetDataFunction<IFinishScanState> = useCallback(overrides => setFinishScanState(prev=>({ ...prev, ...overrides})),[]);
     const [scanState, setScanState] = useState<IScanPageState>(IDLE_STATE);
     const updateScanState: SetDataFunction<IScanPageState> = useCallback(overrides => setScanState(prev=>({ ...prev, ...overrides })),[])
     const [activeRoute, setActiveRoute] = useState("/scan/main");
     const [startedAt, setStartedAt] = useState<number | null>(null);
     const [seen, setSeen] = useState(true);
     const startTimeRef = useRef<number | null>(null);
     const {settings} = useSettings();
     const maxLogLinesRef = useRef(settings.maxLogLines);
     maxLogLinesRef.current = settings.maxLogLines;
     const {getSettingsBySection} = useBackendSettings();
     const {t} = useTranslation("scan")
     const {t: messageTxt} = useTranslation("messages")
     const elapsed = () => startTimeRef.current ? Math.floor((Date.now() - startTimeRef.current)/1000) : 0;

     useEffect(()=>{
          const unsubs: Promise<UnlistenFn>[] = [
               listen<string>("clamscan:log",e=>{
                    setScanState(prev => {
                         if (!ACTIVE.includes(prev.status)) return prev;
                         let next = { ...prev };
                         next.logs = [...prev.logs.slice(-(maxLogLinesRef.current - 1)), e.payload]
                         // "<path>: <signature> FOUND" — the path may contain spaces; the greedy path stops at the last ": ".
                         const found = e.payload.match(/^(.*): (.+) FOUND$/);
                         if(found) {
                              next.threats = [
                                   ...prev.threats,
                                   {
                                        id: String(prev.threats.length+1),
                                        displayName: found[2],
                                        filePath: found[1],
                                        status: "detected",
                                        detectedAt: new Date()
                                   }
                              ]
                         }
                         if (e.payload.includes(": OK") || e.payload.includes(" FOUND")) {
                              const idx = e.payload.lastIndexOf(": ");
                              next.currLocation = idx !== -1 ? e.payload.slice(0, idx) : prev.currLocation;
                              next.scannedFiles = prev.scannedFiles+1
                         }
                         next.status = prev.status === "starting" ? "running" : prev.status;
                         return next;
                    });
               }),
               listen<number>("clamscan:finished",(e)=>{
                    setScanState(prev=>{
                         if (!ACTIVE.includes(prev.status)) return prev;
                         return {
                              ...prev,
                              status: "finished",
                              duration: elapsed(),
                              exitCode: e.payload,
                              errMsg: undefined
                         }
                    })
                    startTimeRef.current = null;
                    localStorage.setItem("last-scanned",Date.now().toString())
               }),
               listen<number>("clamscan:total", e => updateScanState({ totalFiles: e.payload })),
               listen<string>("clamscan:error", e => {
                    setScanState(prev=>{
                         if (!ACTIVE.includes(prev.status)) return prev;
                         return {
                              ...prev,
                              status: "error",
                              errMsg: e.payload,
                              duration: elapsed(),
                              exitCode: -1
                         }
                    })
                    startTimeRef.current = null;
               })
          ];
          return () => {
               Promise.all(unsubs).then(fns=>fns.forEach(fn=>fn()));
          }
     },[]); // registered once: re-subscribing mid-scan could miss the "finished" event

     // The webview may have reloaded while the backend kept scanning.
     useEffect(() => {
          invoke<boolean>("get_scan_status").then(isRunning => {
               if (isRunning) setScanState(prev => ACTIVE.includes(prev.status) ? prev : {...prev, status: "reconnecting", isReconnected: true});
          }).catch(() => toast.warning(messageTxt("scan-status-error")));
     }, []);

     const isDone = scanState.status === "finished" || scanState.status === "error"
     // Keyed on isDone alone so it fires once per scan, wherever the user is in the app.
     useEffect(()=>{
          if(!isDone || !settings.notifOnScanFinish) return;
          invoke("notify",{
               title: t("notification.scan-finish.title"),
               body: !scanState.errMsg ? t("notification.scan-finish.desc",{count: scanState.threats.length}) : t("notification.scan-finish.with-err"),
               kind: scanState.errMsg || scanState.threats.length>0 ? "alert" : "success"
          })
     },[isDone])

     const startScan = useCallback(async (type: ScanType, paths: string[], route: string) => {
          const isMainOrFull = type === "main" || type === "full";
          startTimeRef.current = Date.now();
          setStartedAt(startTimeRef.current);
          setActiveRoute(route);
          setSeen(false);
          setFinishScanState(INITIAL_FINISH_SCAN_STATE);
          setScanState({...GET_INITIAL_SCAN_STATE(type, isMainOrFull ? [] : paths), status: "running"});
          if (settings.notifOnScanStart) {
               invoke("notify",{
                    title: t("notification.scan-start.title"),
                    body: t("notification.scan-start.desc", { scanName: type !== ScanType.None ? t(`scan-type.${type}.name`) : t("scan-type.fallback") }),
                    kind: "info"
               });
          }
          try{
               let scanOptions: ScanProfileValues | null = null;
               const scanProfile: ScanProfile | null =
                    type==="main" ? ScanProfile.Main :
                    type==="custom" ? ScanProfile.Custom :
                    type==="file" ? ScanProfile.File : null;
               if(scanProfile){
                    const availableOptions = await getSettingsBySection("scanProfiles",scanProfile);
                    if(availableOptions) scanOptions = hydrateProfile(availableOptions,scanProfile==="file");
               }
               const command = `start_${isMainOrFull ? type : "custom"}_scan`;
               const payload = {
                    ...(!isMainOrFull ? { paths } : {}),
                    args: scanOptions ? mapScanSettingsToArgs(validateScanSettings(scanOptions)): null
               };
               // Right after "Stop" the killed scan may still be releasing the scanner for a moment.
               for (let attempt = 0; ; attempt++) {
                    try {
                         await invoke(command, payload);
                         break;
                    } catch (e) {
                         if (getErrorMessage(e) !== "SCAN_ALREADY_RUNNING" || attempt >= 6) throw e;
                         await new Promise(r => setTimeout(r, 500));
                    }
               }
          } catch (e){
               const errMsg = getErrorMessage(e);
               if (errMsg === "SCAN_ALREADY_RUNNING") {
                    // Another scan (e.g. from the tray) owns the scanner: follow it instead of failing.
                    setScanState(prev => ({...prev, status: "reconnecting", isReconnected: true}));
                    return;
               }
               toast.error(messageTxt("scan-start-error"));
               setScanState(prev=>{
                    if (!ACTIVE.includes(prev.status)) return prev;
                    return { ...prev, status: "error", errMsg, duration: elapsed(), exitCode: -1 }
               })
               startTimeRef.current = null;
          }
     },[settings.notifOnScanStart, t]);

     const reset = useCallback(() => {
          startTimeRef.current = null;
          setStartedAt(null);
          setSeen(true);
          setScanState(IDLE_STATE);
     },[]);
     const markSeen = useCallback(() => setSeen(true),[]);
     const hasSession = ACTIVE.includes(scanState.status) || (isDone && !seen);

     const values: ScanContextValues = useMemo(()=>({
          finishScanState, setFinishScanState, updateFinishScanState,
          scanState, setScanState, updateScanState,
          startScan, reset, markSeen, hasSession, activeRoute, startedAt
     }),[finishScanState, scanState, startScan, hasSession, activeRoute, startedAt])
     return (
          <ScanContext.Provider value={values}>
               {children}
          </ScanContext.Provider>
     )
}
export function useScan(){
     const ctx = useContext(ScanContext);
     if (!ctx) {
          throw new Error("useScan must be used inside ScanProvider");
     }
     return ctx;
}
