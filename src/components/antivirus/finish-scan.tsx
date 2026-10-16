import { useNavigate } from "react-router";
import { lazy, Suspense, useEffect } from "react";
import { exit } from "@tauri-apps/plugin-process";
import ScanFinishedLoader from "@/loaders/scan/scan-finish/no-threats";
import ScanFinishedTableLoader from "@/loaders/scan/scan-finish/with-threats";
import { useScan } from "@/context/antivirus/scan";

const ScanFinishedContent = lazy(()=>import("@/contents/scan-finish/no-threats"))
const ScanFinishedTable = lazy(()=>import("@/contents/scan-finish/with-threats"))

interface Props{
     isStartup: boolean,
}
export default function ScanFinishResult({isStartup}: Props){
     const {scanState, markSeen} = useScan()
     const navigate = useNavigate();
     useEffect(() => markSeen(), []);
     const handlePrimaryAction = async () => {
          if (isStartup) {
               await exit(0);
          } else {
               navigate("/");
          }
     };
     const {errMsg, threats} = scanState;
     const hasErrors = !!errMsg && errMsg.trim() !== "";
     const hasThreats = threats.length > 0;
     // Exit code 2 (some files unreadable) can come with real detections: those must stay actionable.
     return (hasErrors || !hasThreats) ? (
          <Suspense fallback={<ScanFinishedLoader isStartup={isStartup} hasErr={hasErrors}/>}>
               <ScanFinishedContent
                    isStartup={isStartup}
                    handlePrimaryAction={handlePrimaryAction}
               />
          </Suspense>
     )  : (
          <Suspense fallback={<ScanFinishedTableLoader rows={threats.slice(0,10).length}/>}>
               <ScanFinishedTable
                    isStartup={isStartup}
                    handlePrimaryAction={handlePrimaryAction}
               />
          </Suspense>
     )
}