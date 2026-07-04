// react-native-screens has no web build; the JS stack (@react-navigation/stack) works without it.
// Provide the no-op enableScreens + a passthrough so react-navigation's optional import is happy.
export function enableScreens() {}
export function enableFreeze() {}
export const screensEnabled = () => false;
export const ScreenContainer = ({ children }: any) => children ?? null;
export const Screen = ({ children }: any) => children ?? null;
export default {};
