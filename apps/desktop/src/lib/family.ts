import { useEffect, useState } from "react";
import { api } from "./api";
import type { FamilyApp } from "./types";

// Asked once per session: which family apps are on this computer (for buttons such as
// "Save to OpenBlocks" that only make sense when the app is installed).
let asked: Promise<FamilyApp[]> | null = null;

export function useFamilyInstalled(id: string): boolean {
  const [on, setOn] = useState(false);
  useEffect(() => {
    asked ??= api.familyStatus().catch(() => []);
    let live = true;
    void asked.then((apps) => live && setOn(apps.some((a) => a.id === id && a.installed)));
    return () => {
      live = false;
    };
  }, [id]);
  return on;
}
