// Web swap for @react-navigation/native-stack. The native stack needs react-native-screens (no
// web support); @react-navigation/stack is the JS-driven stack that renders in the browser. It
// exposes the same createXStackNavigator API + the screen-props type, so App.tsx is unchanged.
// The vite alias points '@react-navigation/native-stack' here for the web build only.

export { createStackNavigator as createNativeStackNavigator } from '@react-navigation/stack';
export type { StackScreenProps as NativeStackScreenProps } from '@react-navigation/stack';
