import Logo from "@/components/logo";
import { Skeleton } from "@/components/ui/skeleton";
import { getAppDetails, getClamAvVersion } from "@/data/app";
import { INITIAL_VERSION_INFO } from "@/lib/constants/states";
import { IClamAvVersion } from "@/lib/types";
import { IAppDetails } from "@/lib/types/states";
import { HeartHandshake } from "lucide-react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

const STACK = ["Tauri", "Rust", "React", "TypeScript", "Tailwind CSS", "ClamAV"];
const ORIGINAL_URL = "https://github.com/ArsenTech/clamav-gui";
const CONTRIBUTOR_URL = "https://github.com/tonyreeldev";

function ExternalLink({href, children}: {href: string, children: React.ReactNode}){
     return (
          <a
               href={href}
               onClick={e=>{ e.preventDefault(); openUrl(href); }}
               className="font-medium text-foreground underline decoration-primary/40 underline-offset-4 hover:decoration-primary"
          >{children}</a>
     )
}

export default function AboutContent(){
     const [details, setDetails] = useState<IAppDetails>(()=>JSON.parse(localStorage.getItem("versions") as string) || INITIAL_VERSION_INFO);
     const [clamavVersion, setClamavVersion] = useState<IClamAvVersion|null>(()=>JSON.parse(localStorage.getItem("clamav-version") as string) || null);
     const {t} = useTranslation("about")
     useEffect(()=>{
          (async()=>{
               const newDetails = await getAppDetails()
               setDetails(prev=>({...prev,...newDetails}));
               localStorage.setItem("versions",JSON.stringify(newDetails));
               const clamAvVersion = await getClamAvVersion()
               if(clamAvVersion!==null){
                    const version = { engine: clamAvVersion.engine, dbVersion: clamAvVersion.dbVersion }
                    setClamavVersion(version)
                    localStorage.setItem("clamav-version", JSON.stringify(version));
               }
          })();
     },[])
     return (
          <div className="max-w-xl mx-auto py-8 flex flex-col items-center text-center gap-8">
               <div className="flex flex-col items-center gap-2">
                    <Logo size="lg"/>
                    {details.version.trim()!=="" ? (
                         <p className="text-sm text-muted-foreground">{t("version",{version: details.version})}</p>
                    ) : (
                         <Skeleton className="h-4 w-24"/>
                    )}
               </div>
               <section className="rounded-2xl border bg-card p-6 space-y-3">
                    <h2 className="flex items-center justify-center gap-2 text-lg font-semibold">
                         <HeartHandshake className="size-5 text-primary"/>
                         {t("community.title")}
                    </h2>
                    <p className="text-muted-foreground leading-relaxed">{t("community.desc")}</p>
               </section>
               <section className="space-y-3">
                    <h2 className="text-xs font-semibold uppercase tracking-wider text-muted-foreground">{t("built-with")}</h2>
                    <ul className="flex flex-wrap justify-center gap-2">
                         {STACK.map(name=>(
                              <li key={name} className="rounded-full border px-3 py-1 text-sm">{name}</li>
                         ))}
                    </ul>
               </section>
               <section className="space-y-2 text-sm text-muted-foreground">
                    <h2 className="text-xs font-semibold uppercase tracking-wider">{t("credits.title")}</h2>
                    <p>{t("credits.based-on")} <ExternalLink href={ORIGINAL_URL}>ArsenTech</ExternalLink></p>
                    <p>{t("credits.improved-by")} <ExternalLink href={CONTRIBUTOR_URL}>@tonyreeldev</ExternalLink></p>
                    <p className="text-xs leading-relaxed max-w-md mx-auto">{t("credits.improvements")}</p>
                    <p className="text-xs">{t("credits.license")}</p>
               </section>
               {clamavVersion ? (
                    <p className="text-xs text-muted-foreground" title={t("def-version")}>{t("clamav-version",{
                         engine: clamavVersion.engine,
                         dbVersion: clamavVersion.dbVersion
                    })}</p>
               ) : (
                    <Skeleton className="h-3 w-48"/>
               )}
          </div>
     )
}
