import React from 'react';
export const SafeAreaProvider = ({ children }) => React.createElement(React.Fragment, null, children);
export const SafeAreaView = ({ children, style }) => React.createElement('div', { style }, children);
export const useSafeAreaInsets = () => ({ top: 0, bottom: 0, left: 0, right: 0 });
export const useSafeAreaFrame = () => ({ x: 0, y: 0, width: 0, height: 0 });
export const SafeAreaInsetsContext = React.createContext({ top: 0, bottom: 0, left: 0, right: 0 });
export const initialWindowMetrics = { insets: { top: 0, bottom: 0, left: 0, right: 0 }, frame: { x: 0, y: 0, width: 0, height: 0 } };
export default {};
