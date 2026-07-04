export function enableScreens() {}
export function enableFreeze() {}
export const screensEnabled = () => false;
const Passthrough = ({ children }) => children ?? null;
export const ScreenContainer = Passthrough;
export const Screen = Passthrough;
export const ScreenStack = Passthrough;
export const NativeScreen = Passthrough;
export default {};
