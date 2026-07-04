// React Navigation theme derived from our Unistyles tokens, so the stack chrome (screen background,
// header bar, back-button tint, borders) matches the app instead of the library's default light.
// Kept out of App.tsx to hold the one-responsibility line; App just imports navTheme.

import { DarkTheme, type Theme } from '@react-navigation/native';
import { darkTheme } from './tokens';

export const navTheme: Theme = {
  ...DarkTheme,
  colors: {
    ...DarkTheme.colors,
    primary: darkTheme.colors.mint,
    background: darkTheme.colors.ink,
    card: darkTheme.colors.ink,
    text: darkTheme.colors.text,
    border: darkTheme.colors.line,
    notification: darkTheme.colors.mint,
  },
};
