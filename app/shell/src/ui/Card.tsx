// A raised surface: the base container for everything in the reference set. Hairline border, soft
// radius, no drop shadow (the dark-on-dark look reads depth from the border + surface step, not
// shadow). `active` washes it in mint for the "device is on / room selected" state.

import React from 'react';
import { View, type ViewProps } from 'react-native';
import { StyleSheet } from 'react-native-unistyles';

type Props = ViewProps & { active?: boolean };

export function Card({ active, style, children, ...rest }: Props) {
  return (
    <View style={[styles.card(!!active), style]} {...rest}>
      {children}
    </View>
  );
}

const styles = StyleSheet.create((t) => ({
  card: (active: boolean) => ({
    backgroundColor: active ? t.colors.mintDim : t.colors.surface,
    borderRadius: t.radius.lg,
    borderWidth: 1,
    borderColor: active ? t.colors.mint : t.colors.line,
    padding: t.space(4),
  }),
}));
