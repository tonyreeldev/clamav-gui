import { useSettings } from "@/context/settings";
import { Switch } from "../ui/switch";
import { useRealtimeScan } from "@/context/antivirus/real-time";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import AlertBox from "../popup/alert";

export function RealTimeToggle(){
     const [isOpen, setIsOpen] = useState(false)
     const { setSettings } = useSettings();
     // The provider starts/stops the backend when the setting changes; calling start/stop here too ran it twice.
     const {enabled} = useRealtimeScan()
     const handleToggle = (on: boolean) => {
          if (on) {
               setSettings({ realTime: true });
          } else {
               setIsOpen(true);
          }
     };
     const confirmDisable = () => {
          setIsOpen(false);
          setSettings({ realTime: false });
     };
     const {t} = useTranslation("confirmation")
     return (
          <>
          <Switch
               checked={enabled}
               onCheckedChange={handleToggle}
          />
          <AlertBox
               open={isOpen}
               setOpen={setIsOpen}
               title={t("real-time-scan.title")}
               description={t("real-time-scan.desc")}
               submitText={t("actions.turn-off")}
               submitEvent={confirmDisable}
               cancelText={t("actions.cancel")}
               type="danger"
          />
          </>
     )
}