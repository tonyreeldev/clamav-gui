import { AppLayout } from "@/components/layout";
import useWindowTitle from "@/hooks/use-window-title";
import { AboutContentLoader } from "@/loaders/about";
import { lazy, Suspense } from "react";
import { useTranslation } from "react-i18next";

const AboutContent = lazy(()=>import("@/contents/about"));

export default function AboutPage(){
     const {t} = useTranslation("about")
     useWindowTitle(t("title"))
     return (
          <AppLayout>
               <Suspense fallback={<AboutContentLoader/>}>
                    <AboutContent/>
               </Suspense>
          </AppLayout>
     )
}
