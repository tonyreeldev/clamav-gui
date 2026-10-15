import { Spinner } from "@/components/ui/spinner";
import Logo from "../logo";
import { useTranslation } from "react-i18next";

export default function SplashScreen(){
     const {t} = useTranslation()
     return (
          <div className="flex justify-center items-center gap-8 flex-col h-dvh bg-background px-4">
               <Logo size="lg"/>
               <div className="flex items-center gap-2 text-muted-foreground">
                    <Spinner className="size-4"/>
                    <p className="text-sm">{t("splash-screen-text")}</p>
               </div>
          </div>
     )
}