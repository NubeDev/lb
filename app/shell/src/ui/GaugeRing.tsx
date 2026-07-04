// The dotted-ring gauge from the reference energy screen: a full ring of dots, the leading arc lit
// mint to show `value/max`, big number + unit centered. Pure react-native-svg (works native + web
// via RN-Web's svg support) — no animation dep. Dot count and geometry are props-free constants so
// the shape stays consistent wherever it's used.

import React from 'react';
import { View, Text } from 'react-native';
import Svg, { Circle } from 'react-native-svg';
import { StyleSheet, useUnistyles } from 'react-native-unistyles';

type Props = { value: number; max: number; label: string; unit?: string; size?: number };

const DOTS = 60;
const GAP_DEG = 90; // open gap at the bottom, like the reference

export function GaugeRing({ value, max, label, unit, size = 220 }: Props) {
  const { theme } = useUnistyles();
  const r = size / 2 - 10;
  const cx = size / 2;
  const cy = size / 2;
  const frac = Math.max(0, Math.min(1, value / max));
  const sweep = 360 - GAP_DEG;
  const start = 90 + GAP_DEG / 2; // start after the bottom gap, going clockwise

  const dots = Array.from({ length: DOTS }, (_, i) => {
    const t = i / (DOTS - 1);
    const angle = ((start + t * sweep) * Math.PI) / 180;
    return {
      x: cx + r * Math.cos(angle),
      y: cy + r * Math.sin(angle),
      lit: t <= frac,
    };
  });

  return (
    <View style={[styles.wrap, { width: size, height: size }]}>
      <Svg width={size} height={size}>
        {dots.map((d, i) => (
          <Circle
            key={i}
            cx={d.x}
            cy={d.y}
            r={d.lit ? 3.5 : 2}
            fill={d.lit ? theme.colors.mint : theme.colors.textFaint}
          />
        ))}
      </Svg>
      <View style={styles.center}>
        <Text style={styles.value}>{label}</Text>
        {unit ? <Text style={styles.unit}>{unit}</Text> : null}
      </View>
    </View>
  );
}

const styles = StyleSheet.create((t) => ({
  wrap: { alignItems: 'center', justifyContent: 'center' },
  center: { position: 'absolute', alignItems: 'center' },
  value: { color: t.colors.text, fontSize: 44, fontWeight: t.font.weightBold, letterSpacing: -1 },
  unit: { color: t.colors.textDim, fontSize: 14, marginTop: 2 },
}));
