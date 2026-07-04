// Empty stand-in for expo-secure-store on web (unused — keychain.storage.web.ts wins first via .web
// platform resolution). This alias only exists so the Vite web build never tries to resolve the
// native-only expo-secure-store package. The token store's web path uses localStorage instead
// (keychain.storage.web.ts). Mirrors the native SecureStore surface the app imports.
export const AFTER_FIRST_UNLOCK_THIS_DEVICE_ONLY = 'afterFirstUnlockThisDeviceOnly';
export async function getItemAsync(): Promise<string | null> { return null; }
export async function setItemAsync(): Promise<void> {}
export async function deleteItemAsync(): Promise<void> {}
