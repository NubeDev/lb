// Empty stand-in for react-native-keychain on web (unused — keychain.storage.web.ts wins first).
export async function getGenericPassword() { return false; }
export async function setGenericPassword() { return false; }
export async function resetGenericPassword() { return true; }
