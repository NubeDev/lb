// shadcn-style `cn` helper (clsx + tailwind-merge). Same idiom as the main ui.

import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}
