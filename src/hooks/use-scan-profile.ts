import { DEFAULT_BACKEND_SETTINGS } from "@/lib/constants/settings";
import { IBackendSettings, ScanProfileValues } from "@/lib/types/settings";
import { useState, useEffect } from "react";
import { useBackendSettings } from "./use-settings";
import { hydrateProfile } from "@/lib/helpers/scan";

// Each change is a read-modify-write of the whole store section; chaining them stops a fast second change from undoing the first.
let writeQueue: Promise<unknown> = Promise.resolve();

export function useScanProfile(profile: keyof IBackendSettings["scanProfiles"]) {
     const { fetchSettingsbySection, setSettingsbySection } = useBackendSettings();
     const [values, setValues] = useState<ScanProfileValues>({});
     const [isLoading, setIsLoading] = useState(true);

     useEffect(() => {
          (async () => {
               const profiles = await fetchSettingsbySection("scanProfiles");
               setValues(profiles?.[profile] ?? {});
               setIsLoading(false);
          })();
     }, [profile]);

     const setValue = async <K extends keyof ScanProfileValues>(
          key: K,
          value: ScanProfileValues[K]
     ) => {
          setValues(prev => ({ ...prev, [key]: value }));
          writeQueue = writeQueue.catch(() => {}).then(async () => {
               const profiles = (await fetchSettingsbySection("scanProfiles")) ?? DEFAULT_BACKEND_SETTINGS.scanProfiles;
               await setSettingsbySection("scanProfiles", profile, {
                    ...profiles[profile],
                    [key]: value,
               });
          });
          await writeQueue;
     };

     return { values: hydrateProfile(values), setValue, isLoading };
}