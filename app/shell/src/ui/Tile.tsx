// A quick-access tile from the reference "Quick Access" row: an icon in a chip, a title, a subtitle
// (e.g. "3/4 cctv"), pressable with a springy press-in. `active` promotes it to the mint state. The
// icon is passed in (a lucide-react-native element) so this file owns layout, not iconography.

import React from 'react';
import { Pressable, Text, View } from 'react-native';
import { StyleSheet } from 'react-native-unistyles';

type Props = {
  title: string;
  subtitle?: string;
  icon?: React.ReactNode;
  active?: boolean;
  onPress?: () => void;
};

export function Tile({ title, subtitle, icon, active, onPress }: Props) {
  return (
    <Pressable
      onPress={onPress}
      style={({ pressed }) => [styles.tile(!!active), pressed && styles.pressed]}
    >
      <View style={styles.iconChip(!!active)}>{icon}</View>
      <Text style={styles.title} numberOfLines={1}>
        {title}
      </Text>
      {subtitle ? (
        <Text style={styles.subtitle} numberOfLines={1}>
          {subtitle}
        </Text>
      ) : null}
    </Pressable>
  );
}

const styles = StyleSheet.create((t) => ({
  tile: (active: boolean) => ({
    flex: 1,
    minWidth: 92,
    gap: t.space(2.5),
    padding: t.space(3.5),
    borderRadius: t.radius.md,
    borderWidth: 1,
    borderColor: active ? t.colors.mint : t.colors.line,
    backgroundColor: active ? t.colors.mintDim : t.colors.surface,
  }),
  pressed: { transform: [{ scale: 0.97 }], opacity: 0.9 },
  iconChip: (active: boolean) => ({
    width: 40,
    height: 40,
    borderRadius: t.radius.sm,
    alignItems: 'center',
    justifyContent: 'center',
    backgroundColor: active ? t.colors.mint : t.colors.surfaceHi,
  }),
  title: { color: t.colors.text, fontSize: 15, fontWeight: t.font.weightMed },
  subtitle: { color: t.colors.textDim, fontSize: 12 },
}));
