import { lazy, Suspense, useEffect, useState, useTransition } from "react";
import TopNav from "@/components/top-nav";
import { cn } from "@/lib/utils";
import {invoke} from "@tauri-apps/api/core"
import SplashScreen from "./splash-screen";
import { ClamAVState } from "@/lib/types/enums";
import { Toaster } from "../ui/sonner";
import { useNavigate } from "react-router";
import { useStartupScan } from "@/context/antivirus/startup-scan";
import { useSettings } from "@/context/settings";
import { isPermissionGranted, requestPermission, } from '@tauri-apps/plugin-notification';
import { listen, UnlistenFn } from "@tauri-apps/api/event";
import { useLocale } from "@/i18n/locale";
import { checkAvailability } from "@/data/app";

const NoClamAVPage = lazy(()=>import("./no-clamav"));

interface Props{
     children: React.ReactNode,
     className?: string
}
export function AppLayout({children, className}: Props){
     const cached = localStorage.getItem("clamav") as ClamAVState | null;
     const [status, setStatus] = useState<ClamAVState>(cached || ClamAVState.Checking);
     const navigate = useNavigate();
     const [isLoading, startTransition] = useTransition();
     const {setSettings, settings} = useSettings()
     const startupScan = useStartupScan()
     const {locale: lang} = useLocale()
     const handleCheck = () => {
          startTransition(async() => {
               try{
                    const isAvailable = await checkAvailability()
                    const next: ClamAVState = isAvailable ? ClamAVState.Available : ClamAVState.Missing;
                    setStatus(next);
                    localStorage.setItem("clamav",next);
               } catch {
                    setStatus(ClamAVState.Missing);
                    localStorage.setItem("clamav","missing");
               }
          })
     }
     useEffect(() => {
          handleCheck();
          (async()=>{
               let permissionGranted = await isPermissionGranted();
               if(!permissionGranted){
                    const permission = await requestPermission();
                    permissionGranted = permission === 'granted';
               }
               setSettings({notifPermitted: permissionGranted});
               await invoke("set_language",{ lang });
               await invoke("rebuild_tray")
          })()
          const listeners: Promise<UnlistenFn>[] = [
               listen<string>("systray:move",e=>navigate(encodeURI(e.payload)))
          ]
          return () => {
               Promise.all(listeners).then(fns=>fns.forEach(fn=>fn()))
          }
     }, []);
     useEffect(() => {
          if (!startupScan) return;
          if (startupScan.isStartup && startupScan.scanType)
               navigate(`/scan/${startupScan.scanType}`, { replace: true });
     }, [startupScan]);
     return (
          <main className={cn(settings.reduceMotion && "reduce-motion")}>
               {status==="checking" ? (
                    <SplashScreen/>
               ) : status==="missing" ? (
                    <Suspense fallback={<SplashScreen/>}>
                         <NoClamAVPage
                              isPending={isLoading}
                              handleCheck={handleCheck}
                         />
                    </Suspense>
               ) : startupScan?.isStartup ? (
                    <main className="py-2 w-full">
                         <div className={cn("w-full",className)}>
                              {children}
                         </div>
                    </main>
               ) : (
                    <div className="min-h-dvh flex flex-col">
                         <TopNav/>
                         <div className={cn("w-full max-w-6xl mx-auto px-2 sm:px-4 py-4 flex-1",className)}>
                              {children}
                         </div>
                    </div>
               )}
               <Toaster
                    richColors
                    position="top-right"
               />
          </main>
     )
}