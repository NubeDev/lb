/**
 * Shell entry. Polyfills load FIRST — the sdk's SSE client needs streaming fetch, which RN's stock
 * networking lacks (see src/polyfills.ts).
 */

import './src/polyfills';
import './src/theme/unistyles'; // register themes before any StyleSheet.create runs
import { AppRegistry } from 'react-native';
import App from './src/App';
import { name as appName } from './app.json';

AppRegistry.registerComponent(appName, () => App);
