import { parseClamVersion } from "@/lib/helpers";
import { IAppDetails } from "@/lib/types/states";
import { getVersion, getTauriVersion, getName, getIdentifier } from "@tauri-apps/api/app";
import { invoke } from "@tauri-apps/api/core";
import { cache } from "react";

export const getAppDetails = cache(async(): Promise<IAppDetails> => {
     const [appName, version, tauri, identifier] = await Promise.all([
          getName(),
          getVersion(),
          getTauriVersion(),
          getIdentifier()
     ]);
     return {name: appName, version, tauri, identifier}
})

export const getClamAvVersion = cache(async() => {
     try{
          const clamAVraw = await invoke<string>("get_clamav_version");
          const parsed = parseClamVersion(clamAVraw);
          return parsed
     } catch {
          return null
     }
})

export const checkDefinitionStatus = cache(async() => {
     try{
          const parsed = await getClamAvVersion();
          return parsed ? parsed.isOutdated : false
     } catch {
          return false
     }
})

export const checkAvailability = cache(async() =>await invoke<boolean>("check_availability"))