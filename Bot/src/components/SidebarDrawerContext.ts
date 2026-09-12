import { createContext, useContext } from "react";

export const SidebarDrawerContext = createContext<(() => void) | null>(null);

export function useSidebarDrawer() {
  return useContext(SidebarDrawerContext);
}
