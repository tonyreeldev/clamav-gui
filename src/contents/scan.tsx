import { AppLayout } from "@/components/layout";
import ScanFinishResult from "@/components/antivirus/finish-scan";
import LogText from "@/components/log";
import { ScanType } from "@/lib/types/enums";
import { lazy, Suspense, useEffect, useRef } from "react";
import { useStartupScan } from "@/context/antivirus/startup-scan";
import ScanLoader from "@/loaders/scan/index";
import { ScrollArea } from "@/components/ui/scroll-area";
import { useTranslation } from "react-i18next";
import { useScan } from "@/context/antivirus/scan";
import { useLocation } from "react-router";

const ScanProcess = lazy(()=>import("@/components/antivirus/scan-process"))

interface Props{
     type?: ScanType,
     path: string[]
}
export default function ScanContent({type, path}: Props){
     const {scanState, startScan, reset, hasSession} = useScan()
     const {isStartup} = useStartupScan();
     const location = useLocation();
     const handledRef = useRef(false); // StrictMode runs mount effects twice
     // Until the start effect runs, don't flash the previous scan's results.
     const isDone = (scanState.status === "finished" || scanState.status === "error") && !(!handledRef.current && !hasSession && type)
     // Opening a scan route starts that scan, unless one is already in progress or its results are unseen:
     // then this page just shows it, so leaving and coming back never restarts or loses a scan.
     useEffect(() => {
          if (handledRef.current) return;
          handledRef.current = true;
          if (hasSession || !type) return;
          startScan(type, path, location.pathname + location.search);
     }, []);
     const {logs, scanType} = scanState;
     const {t} = useTranslation("scan")
     const {t: logTxt} = useTranslation()
     return (
          <AppLayout className={isDone ? "flex justify-center items-center gap-4 flex-col p-4" : "grid gris-cols-1 md:grid-cols-2 gap-10 p-4"}>
               {isDone ? (
                    <>
                         <h1 className="text-2xl md:text-3xl font-medium border-b pb-2 w-fit">{t("scan-complete")}</h1>
                         <ScanFinishResult isStartup={isStartup} />
                    </>
               ) : (
                    <>
                         <div className="space-y-4">
                              <h1 className="text-2xl md:text-3xl font-medium border-b pb-2 w-fit">{t("title")}</h1>
                              <Suspense fallback={<ScanLoader type={scanType}/>}>
                                   <ScanProcess
                                        handleReset={reset}
                                        isStartup={isStartup}
                                   />
                              </Suspense>
                         </div>
                         <ScrollArea className="max-h-[800px]">
                              <div className="space-y-3 px-3 text-lg">
                                   <h2 className="text-2xl md:text-3xl font-medium border-b pb-2 w-fit">{logTxt("log.title")}</h2>
                                   <LogText logs={logs}/>
                              </div>
                         </ScrollArea>
                    </>
               )}
          </AppLayout>
     )
}
